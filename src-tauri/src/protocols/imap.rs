use std::{collections::HashMap, time::Duration};

mod encoding;
mod parse;
mod policy;
mod session;

pub use encoding::decode_modified_utf7;
use encoding::{mailbox_leaf_display_name, mailbox_role};
use parse::{message_flag_state, parse_message_in_background, MessageParseInput};
use policy::{should_download_body, sync_policy_cutoff};
#[cfg(test)]
use session::conditional_store_query;
use session::{
    append_message_session, apply_operation_session, fetch_message_session, fetch_remote_messages,
    replace_draft_session, wait_for_change_session,
};

use super::native_tls_connector;
use crate::core::{
    CommandError, CommandResult, ConnectionSecurity, ImapAccountConfig, ImapSyncProvider,
    InboxWatchOutcome, MailSyncSink, MailboxRole, RemoteMailbox, RemoteMessage, RemoteMessageState,
    RemoteOperation, RemoteOperationOutcome, StoredMailbox, SyncNotice, SyncObserver,
};
use async_imap::{
    types::{Flag, NameAttribute},
    Session,
};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use rustls::pki_types::ServerName;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

const FETCH_BATCH_SIZE: usize = 100;

struct FetchedMessageSummary {
    uid: u32,
    received_at: i64,
    unread: bool,
    flagged: bool,
    header: Vec<u8>,
    size: u64,
    modseq: Option<u64>,
}

struct FolderSyncContext<'a> {
    uid_validity: u32,
    mailbox: &'a StoredMailbox,
    mailbox_name: &'a str,
    download_all_bodies: bool,
}

struct FolderDescriptor {
    name: String,
    display_name: String,
    progress_name: String,
    delimiter: Option<String>,
    role: MailboxRole,
    selectable: bool,
}

#[derive(Default)]
pub struct AsyncImapProvider;

trait ImapTransport: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send {}

impl<T> ImapTransport for T where T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send {}

type BoxedImapTransport = Box<dyn ImapTransport>;

#[async_trait]
impl ImapSyncProvider for AsyncImapProvider {
    async fn synchronize(
        &self,
        account: &ImapAccountConfig,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()> {
        sync_session(connect_session(account).await?, account, sink, observer).await
    }

    async fn fetch_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessage> {
        fetch_message_session(
            connect_session(account).await?,
            mailbox_name,
            uid,
            expected_uid_validity,
        )
        .await
    }

    async fn apply_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteOperation,
    ) -> CommandResult<RemoteOperationOutcome> {
        apply_operation_session(connect_session(account).await?, operation).await
    }

    async fn append_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        flags: &str,
        raw: &[u8],
    ) -> CommandResult<()> {
        append_message_session(connect_session(account).await?, mailbox_name, flags, raw).await
    }

    async fn replace_draft(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        draft_id: &str,
        raw: &[u8],
    ) -> CommandResult<RemoteOperationOutcome> {
        replace_draft_session(connect_session(account).await?, mailbox_name, draft_id, raw).await
    }

    async fn wait_for_inbox_change(
        &self,
        account: &ImapAccountConfig,
        timeout: Duration,
    ) -> CommandResult<InboxWatchOutcome> {
        wait_for_change_session(connect_session(account).await?, timeout).await
    }
}

async fn connect_session(
    account: &ImapAccountConfig,
) -> CommandResult<Session<BoxedImapTransport>> {
    let stream = TcpStream::connect((account.host.as_str(), account.port))
        .await
        .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
    let transport: BoxedImapTransport = match account.security {
        ConnectionSecurity::None => Box::new(stream),
        ConnectionSecurity::Tls => Box::new(connect_tls(&account.host, stream).await?),
        ConnectionSecurity::StartTls => {
            let mut client = async_imap::Client::new(stream);
            read_greeting(&mut client).await?;
            client
                .run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
            Box::new(connect_tls(&account.host, client.into_inner()).await?)
        }
    };
    let mut client = async_imap::Client::new(transport);
    if account.security != ConnectionSecurity::StartTls {
        read_greeting(&mut client).await?;
    }
    login(client, account).await
}

async fn read_greeting<T>(client: &mut async_imap::Client<T>) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    client
        .read_response()
        .await
        .map_err(|_| CommandError::new("sync.imap_greeting_failed"))?
        .ok_or_else(|| CommandError::new("sync.imap_greeting_failed"))?;
    Ok(())
}

async fn login<T>(
    client: async_imap::Client<T>,
    account: &ImapAccountConfig,
) -> CommandResult<Session<T>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    client
        .login(&account.username, &account.password)
        .await
        .map_err(|_| CommandError::new("sync.imap_authentication_failed"))
}

async fn sync_session<T>(
    mut session: Session<T>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let folders = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|_| CommandError::retryable("sync.folder_list_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.folder_list_failed"))?;
    let capabilities = session
        .capabilities()
        .await
        .map_err(|_| CommandError::retryable("sync.imap_capability_failed"))?;
    let condstore = capabilities.has_str("CONDSTORE");
    let folder_total = folders.len() as u64;

    for (folder_index, folder) in folders.into_iter().enumerate() {
        let name = folder.name().to_owned();
        let display_name = decode_modified_utf7(&name);
        let delimiter = folder.delimiter().map(str::to_owned);
        let progress_name =
            mailbox_leaf_display_name(&display_name, delimiter.as_deref()).to_owned();
        observer.notify(SyncNotice::Folders {
            completed: folder_index as u64,
            total: folder_total,
            mailbox_name: Some(progress_name.clone()),
        });
        sync_folder(
            &mut session,
            account,
            sink,
            observer,
            condstore,
            FolderDescriptor {
                role: mailbox_role(&display_name, folder.attributes()),
                selectable: !folder.attributes().contains(&NameAttribute::NoSelect),
                delimiter,
                name,
                display_name,
                progress_name,
            },
        )
        .await?;
    }
    observer.notify(SyncNotice::Folders {
        completed: folder_total,
        total: folder_total,
        mailbox_name: None,
    });
    let _ = session.logout().await;
    Ok(())
}

async fn sync_folder<T>(
    session: &mut Session<T>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    condstore: bool,
    folder: FolderDescriptor,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if !folder.selectable {
        let mailbox = sink
            .upsert_mailbox(
                &account.account_slot_id,
                &RemoteMailbox {
                    name: folder.name,
                    display_name: folder.display_name,
                    delimiter: folder.delimiter,
                    role: folder.role,
                    selectable: false,
                    uid_validity: 0,
                    uid_next: 0,
                    total_count: 0,
                    unread_count: 0,
                    highest_modseq: None,
                },
            )
            .await?;
        notify_mailbox(observer, mailbox.id);
        return Ok(());
    }

    let selected = if condstore {
        session.select_condstore(&folder.name).await
    } else {
        session.examine(&folder.name).await
    }
    .map_err(|_| CommandError::retryable("sync.mailbox_open_failed"))?;
    let uid_validity = selected.uid_validity.unwrap_or_default();
    if uid_validity == 0 {
        return Err(CommandError::new("sync.uid_not_supported"));
    }
    let unseen = session
        .uid_search("UNSEEN")
        .await
        .map_err(|_| CommandError::retryable("sync.mailbox_search_failed"))?;
    let highest_modseq = selected.highest_modseq;
    let download_all_bodies =
        account.download_non_inbox_bodies && folder.role != crate::core::MailboxRole::Inbox;
    let mailbox_name = folder.progress_name;
    let mailbox = sink
        .upsert_mailbox(
            &account.account_slot_id,
            &RemoteMailbox {
                name: folder.name,
                display_name: folder.display_name,
                delimiter: folder.delimiter,
                role: folder.role,
                selectable: true,
                uid_validity,
                uid_next: selected.uid_next.unwrap_or_default(),
                total_count: selected.exists,
                unread_count: unseen.len() as u32,
                highest_modseq,
            },
        )
        .await?;
    notify_mailbox(observer, mailbox.id.clone());

    let context = FolderSyncContext {
        uid_validity,
        mailbox: &mailbox,
        mailbox_name: &mailbox_name,
        download_all_bodies,
    };

    let highest_uid =
        fetch_summaries(session, account, sink, observer, condstore, &context).await?;
    backfill_bodies(session, account, sink, observer, &context).await?;
    reconcile_flags(
        session,
        sink,
        condstore,
        uid_validity,
        highest_modseq,
        &mailbox,
    )
    .await?;
    sink.complete_mailbox(&mailbox.id, highest_uid).await?;
    notify_mailbox(observer, mailbox.id);
    Ok(())
}

async fn fetch_summaries<T>(
    session: &mut Session<T>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    condstore: bool,
    context: &FolderSyncContext<'_>,
) -> CommandResult<u32>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut uids = session
        .uid_search("ALL")
        .await
        .map_err(|_| CommandError::retryable("sync.mailbox_search_failed"))?
        .into_iter()
        .filter(|uid| *uid > context.mailbox.last_uid)
        .collect::<Vec<_>>();
    uids.sort_unstable();
    let total = uids.len() as u64;
    let mut completed = 0_u64;
    let mut highest_uid = context.mailbox.last_uid;

    for batch in uids.chunks(FETCH_BATCH_SIZE) {
        let summaries = fetch_summary_batch(session, batch, condstore).await?;
        let body_uids = summaries
            .iter()
            .filter(|summary| {
                context.download_all_bodies
                    || should_download_body(account.sync_policy.clone(), summary.received_at)
            })
            .map(|summary| summary.uid)
            .collect::<Vec<_>>();
        let mut raw_by_uid = fetch_raw_batch(session, &body_uids).await?;

        for summary in summaries {
            let mut message = parse_message_in_background(MessageParseInput {
                uid: summary.uid,
                uid_validity: context.uid_validity,
                size: summary.size,
                received_at: summary.received_at,
                unread: summary.unread,
                flagged: summary.flagged,
                header: summary.header,
                raw: raw_by_uid.remove(&summary.uid),
            })
            .await?;
            message.modseq = summary.modseq;
            sink.upsert_message(&account.account_slot_id, &context.mailbox.id, &message)
                .await?;
            highest_uid = highest_uid.max(summary.uid);
            completed += 1;
            observer.notify(SyncNotice::Summaries {
                completed,
                total,
                mailbox_name: context.mailbox_name.to_owned(),
            });
            notify_mailbox(observer, context.mailbox.id.clone());
        }
    }
    Ok(highest_uid)
}

async fn fetch_summary_batch<T>(
    session: &mut Session<T>,
    uids: &[u32],
    condstore: bool,
) -> CommandResult<Vec<FetchedMessageSummary>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let query = if condstore {
        "(UID FLAGS MODSEQ INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
    } else {
        "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
    };
    Ok(session
        .uid_fetch(format_uid_set(uids), query)
        .await
        .map_err(|_| CommandError::retryable("sync.message_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_fetch_failed"))?
        .into_iter()
        .filter_map(|summary| {
            let uid = summary.uid?;
            let received_at = summary
                .internal_date()
                .map(|value| value.timestamp())
                .unwrap_or_default();
            let (unread, flagged) = message_flag_state(summary.flags());
            Some(FetchedMessageSummary {
                uid,
                received_at,
                unread,
                flagged,
                header: summary.header().unwrap_or_default().to_vec(),
                size: summary.size.unwrap_or_default() as u64,
                modseq: summary.modseq,
            })
        })
        .collect())
}

async fn backfill_bodies<T>(
    session: &mut Session<T>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let received_after = if context.download_all_bodies {
        None
    } else {
        sync_policy_cutoff(account.sync_policy.clone())
    };
    let pending = sink
        .pending_body_locations(&context.mailbox.id, received_after)
        .await?
        .into_iter()
        .filter(|location| location.uid_validity == context.uid_validity)
        .collect::<Vec<_>>();
    let total = pending.len() as u64;
    let mut completed = 0_u64;

    for batch in pending.chunks(FETCH_BATCH_SIZE) {
        let uids = batch
            .iter()
            .map(|location| location.uid)
            .collect::<Vec<_>>();
        for message in fetch_remote_messages(session, &uids, context.uid_validity).await? {
            sink.upsert_message(&account.account_slot_id, &context.mailbox.id, &message)
                .await?;
            completed += 1;
            observer.notify(SyncNotice::Bodies {
                completed,
                total,
                mailbox_name: context.mailbox_name.to_owned(),
            });
            notify_mailbox(observer, context.mailbox.id.clone());
        }
    }
    Ok(())
}

async fn reconcile_flags<T>(
    session: &mut Session<T>,
    sink: &(dyn MailSyncSink + Send + Sync),
    condstore: bool,
    uid_validity: u32,
    highest_modseq: Option<u64>,
    mailbox: &StoredMailbox,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let query = if condstore {
        "(UID FLAGS MODSEQ)"
    } else {
        "(UID FLAGS)"
    };
    let states = session
        .uid_fetch("1:*", query)
        .await
        .map_err(|_| CommandError::retryable("sync.flags_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.flags_fetch_failed"))?
        .into_iter()
        .filter_map(|item| {
            let uid = item.uid?;
            let flags = item.flags().collect::<Vec<_>>();
            Some(RemoteMessageState {
                uid,
                unread: !flags.iter().any(|flag| matches!(flag, Flag::Seen)),
                flagged: flags.iter().any(|flag| matches!(flag, Flag::Flagged)),
                modseq: item.modseq,
            })
        })
        .collect::<Vec<_>>();
    sink.reconcile_mailbox(&mailbox.id, uid_validity, highest_modseq, &states)
        .await
}

fn notify_mailbox(observer: &(dyn SyncObserver + Send + Sync), mailbox_id: String) {
    observer.notify(SyncNotice::MailboxChanged {
        mailbox_id,
        revision: 0,
    });
}

async fn fetch_raw_batch<T>(
    session: &mut Session<T>,
    uids: &[u32],
) -> CommandResult<HashMap<u32, Vec<u8>>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if uids.is_empty() {
        return Ok(HashMap::new());
    }
    let messages = session
        .uid_fetch(format_uid_set(uids), "(UID BODY.PEEK[])")
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?;
    Ok(messages
        .into_iter()
        .filter_map(|message| Some((message.uid?, message.body()?.to_vec())))
        .collect())
}

pub(super) fn format_uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

async fn connect_tls(
    host: &str,
    stream: TcpStream,
) -> CommandResult<tokio_rustls::client::TlsStream<TcpStream>> {
    let server_name = ServerName::try_from(host.to_owned())
        .map_err(|_| CommandError::new("sync.server_name_invalid"))?;
    native_tls_connector("sync.system_certificates_unavailable")?
        .connect(server_name, stream)
        .await
        .map_err(|_| CommandError::retryable("sync.imap_tls_failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::imap::parse::parse_message;
    use mail_parser::MessageParser;

    #[test]
    fn conditional_store_preserves_delta_semantics() {
        assert_eq!(
            conditional_store_query(Some(42), "+FLAGS.SILENT", "\\Seen"),
            "(UNCHANGEDSINCE 42) +FLAGS.SILENT (\\Seen)"
        );
        assert_eq!(
            conditional_store_query(None, "-FLAGS.SILENT", "\\Flagged"),
            "-FLAGS.SILENT (\\Flagged)"
        );
    }

    #[test]
    fn formats_a_batch_as_one_uid_set() {
        assert_eq!(format_uid_set(&[3, 7, 9]), "3,7,9");
        assert_eq!(format_uid_set(&[]), "");
    }

    #[test]
    fn parses_and_sanitizes_html_message() {
        let raw = b"From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: Hello\r\nMessage-ID: <1@example.com>\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p onclick=\"bad()\">Hello<script>bad()</script></p>";
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw,
            Some(raw.to_vec()),
        )
        .unwrap();
        assert_eq!(message.subject, "Hello");
        assert!(!message.unread);
        assert!(!message.safe_html.unwrap().contains("<script"));
    }

    #[test]
    fn embeds_referenced_cid_images_without_listing_them_as_attachments() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Inline image\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>Logo <img src=\"cid:logo@example.test\"></p>\r\n",
            "--nextmail\r\n",
            "Content-Type: image/png; name=logo.png\r\n",
            "Content-Disposition: attachment; filename=logo.png\r\n",
            "Content-ID: <logo@example.test>\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\n",
            "aW1hZ2U=\r\n",
            "--nextmail--\r\n"
        );
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();

        assert!(message
            .safe_html
            .expect("safe HTML")
            .contains("data:image/png;base64,aW1hZ2U="));
        assert!(message.attachments.is_empty());
    }

    #[test]
    fn decodes_gb2312_encoded_words_and_message_bodies() {
        let raw = b"From: =?GB2312?B?xOO6ww==?= <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: =?GB2312?B?xOO6ww==?=\r\n\
Content-Type: text/plain; charset=gb2312\r\n\
Content-Transfer-Encoding: base64\r\n\r\n\
xOO6ww==";
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw,
            Some(raw.to_vec()),
        )
        .unwrap();

        assert_eq!(message.subject, "你好");
        assert_eq!(message.from[0].name.as_deref(), Some("你好"));
        assert_eq!(message.plain_text.as_deref(), Some("你好"));
    }

    #[test]
    fn decodes_rfc2047_b_q_aliases_and_address_phrases() {
        let cases = [
            ("=?UTF-8?B?5L2g5aW9?=", "你好"),
            ("=?utf-8?q?Hello_=E4=B8=96=E7=95=8C?=", "Hello 世界"),
            ("=?ISO-8859-1?Q?caf=E9?=", "café"),
            ("=?windows-1252?Q?=80uro?=", "€uro"),
            ("=?UTF-7?B?K1plVm5MSXFlLQ==?=", "日本語"),
        ];

        for (encoded, expected) in cases {
            let raw =
                format!("From: {encoded} <alice@example.com>\r\nSubject: {encoded}\r\n\r\nbody");
            let message = parse_message(
                1,
                1,
                raw.len() as u64,
                1,
                [Flag::Seen].into_iter(),
                raw.as_bytes(),
                Some(raw.as_bytes().to_vec()),
            )
            .unwrap();
            assert_eq!(message.subject, expected);
            assert_eq!(message.from[0].name.as_deref(), Some(expected));
        }
    }

    #[test]
    fn decodes_adjacent_folded_rfc2047_words_and_mixed_ascii() {
        let raw = concat!(
            "From: =?UTF-8?B?5L2g5aW9?=\r\n",
            " =?UTF-8?Q?_=E4=B8=96=E7=95=8C?= <alice@example.com>\r\n",
            "Subject: Status =?UTF-8?B?5L2g5aW9?=\r\n",
            " =?UTF-8?Q?_=E4=B8=96=E7=95=8C?= ready\r\n\r\n",
            "body"
        );
        let directly_parsed = MessageParser::default().parse(raw.as_bytes()).unwrap();
        assert_eq!(directly_parsed.subject(), Some("Status 你好 世界 ready"));
        assert_eq!(
            directly_parsed
                .from()
                .and_then(|address| address.first())
                .and_then(|address| address.name.as_deref()),
            Some("你好 世界")
        );
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();

        assert_eq!(message.subject, "Status 你好 世界 ready");
        assert_eq!(message.from[0].name.as_deref(), Some("你好 世界"));
    }

    #[test]
    fn malformed_rfc2047_words_fail_safely_without_losing_following_headers() {
        let raw = "From: Alice <alice@example.com>\r\n\
Subject: prefix =?X-UNKNOWN?Q?abc=FF?= suffix\r\n\
Message-ID: <safe@example.com>\r\n\r\nbody";
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();

        assert!(message.subject.starts_with("prefix "));
        assert!(message.subject.ends_with(" suffix"));
        assert_eq!(message.message_id.as_deref(), Some("safe@example.com"));
    }
}

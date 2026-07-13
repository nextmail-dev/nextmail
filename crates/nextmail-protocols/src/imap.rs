use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_imap::{
    extensions::idle::IdleResponse,
    types::{Flag, NameAttribute},
    Session,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::TryStreamExt;
use mail_parser::{Address, Message, MessageParser, MimeHeaders};
use nextmail_core::{
    CommandError, CommandResult, ConnectionSecurity, ImapAccountConfig, ImapSyncProvider,
    InboxWatchOutcome, MailSyncSink, MailboxRole, MessageAddress, RemoteAttachment, RemoteMailbox,
    RemoteMessage, RemoteMessageState, RemoteOperation, RemoteOperationKind,
    RemoteOperationOutcome, SyncNotice, SyncObserver, SyncPolicy,
};
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::TlsConnector;

use crate::sanitize_mail_html;

const FETCH_BATCH_SIZE: usize = 100;

#[derive(Default)]
pub struct AsyncImapProvider;

#[async_trait]
impl ImapSyncProvider for AsyncImapProvider {
    async fn synchronize(
        &self,
        account: &ImapAccountConfig,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                let session = login(client, account).await?;
                sync_session(session, account, sink, observer).await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                let session = login(client, account).await?;
                sync_session(session, account, sink, observer).await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                let session = login(client, account).await?;
                sync_session(session, account, sink, observer).await
            }
        }
    }

    async fn fetch_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessage> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                let session = login(client, account).await?;
                fetch_message_session(session, mailbox_name, uid, expected_uid_validity).await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                let session = login(client, account).await?;
                fetch_message_session(session, mailbox_name, uid, expected_uid_validity).await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                let session = login(client, account).await?;
                fetch_message_session(session, mailbox_name, uid, expected_uid_validity).await
            }
        }
    }

    async fn apply_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteOperation,
    ) -> CommandResult<RemoteOperationOutcome> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                apply_operation_session(login(client, account).await?, operation).await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                apply_operation_session(login(client, account).await?, operation).await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                apply_operation_session(login(client, account).await?, operation).await
            }
        }
    }

    async fn append_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        flags: &str,
        raw: &[u8],
    ) -> CommandResult<()> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                append_message_session(login(client, account).await?, mailbox_name, flags, raw)
                    .await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                append_message_session(login(client, account).await?, mailbox_name, flags, raw)
                    .await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                append_message_session(login(client, account).await?, mailbox_name, flags, raw)
                    .await
            }
        }
    }

    async fn replace_draft(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        draft_id: &str,
        raw: &[u8],
    ) -> CommandResult<RemoteOperationOutcome> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                replace_draft_session(login(client, account).await?, mailbox_name, draft_id, raw)
                    .await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                replace_draft_session(login(client, account).await?, mailbox_name, draft_id, raw)
                    .await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                replace_draft_session(login(client, account).await?, mailbox_name, draft_id, raw)
                    .await
            }
        }
    }

    async fn wait_for_inbox_change(
        &self,
        account: &ImapAccountConfig,
        timeout: Duration,
    ) -> CommandResult<InboxWatchOutcome> {
        let stream = TcpStream::connect((account.host.as_str(), account.port))
            .await
            .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?;
        match account.security {
            ConnectionSecurity::None => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                wait_for_change_session(login(client, account).await?, timeout).await
            }
            ConnectionSecurity::Tls => {
                let tls = connect_tls(&account.host, stream).await?;
                let mut client = async_imap::Client::new(tls);
                read_greeting(&mut client).await?;
                wait_for_change_session(login(client, account).await?, timeout).await
            }
            ConnectionSecurity::StartTls => {
                let mut client = async_imap::Client::new(stream);
                read_greeting(&mut client).await?;
                client
                    .run_command_and_check_ok("STARTTLS", None)
                    .await
                    .map_err(|_| CommandError::new("sync.imap_starttls_failed"))?;
                let tls = connect_tls(&account.host, client.into_inner()).await?;
                let client = async_imap::Client::new(tls);
                wait_for_change_session(login(client, account).await?, timeout).await
            }
        }
    }
}

async fn apply_operation_session<T>(
    mut session: Session<T>,
    operation: &RemoteOperation,
) -> CommandResult<RemoteOperationOutcome>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let capabilities = session
        .capabilities()
        .await
        .map_err(|_| CommandError::retryable("operation.capability_failed"))?;
    let selected = if capabilities.has_str("CONDSTORE") {
        session.select_condstore(&operation.source_mailbox).await
    } else {
        session.select(&operation.source_mailbox).await
    }
    .map_err(|_| CommandError::retryable("operation.mailbox_open_failed"))?;
    if selected.uid_validity.unwrap_or_default() != operation.uid_validity {
        return Err(CommandError::new("sync.uid_validity_changed"));
    }
    let uid = operation.uid.to_string();
    let source_contains_uid = session
        .uid_search(format!("UID {uid}"))
        .await
        .map_err(|_| CommandError::retryable("operation.message_check_failed"))?
        .contains(&operation.uid);
    if !source_contains_uid {
        let _ = session.logout().await;
        return if matches!(
            operation.kind,
            RemoteOperationKind::Move | RemoteOperationKind::Delete
        ) {
            Ok(RemoteOperationOutcome::default())
        } else {
            Err(CommandError::new("operation.message_missing"))
        };
    }
    let mut cleanup_pending = false;
    match operation.kind {
        RemoteOperationKind::SetRead(value) => {
            let action = if value { "+FLAGS" } else { "-FLAGS" };
            store_flag_delta(
                &mut session,
                &uid,
                operation.base_modseq,
                capabilities.has_str("CONDSTORE"),
                action,
                "\\Seen",
            )
            .await?;
        }
        RemoteOperationKind::SetFlagged(value) => {
            let action = if value { "+FLAGS" } else { "-FLAGS" };
            store_flag_delta(
                &mut session,
                &uid,
                operation.base_modseq,
                capabilities.has_str("CONDSTORE"),
                action,
                "\\Flagged",
            )
            .await?;
        }
        RemoteOperationKind::Copy => {
            let destination = operation
                .destination_mailbox
                .as_deref()
                .ok_or_else(|| CommandError::new("operation.destination_required"))?;
            session
                .uid_copy(&uid, destination)
                .await
                .map_err(|_| CommandError::retryable("operation.copy_failed"))?;
        }
        RemoteOperationKind::Move => {
            let destination = operation
                .destination_mailbox
                .as_deref()
                .ok_or_else(|| CommandError::new("operation.destination_required"))?;
            if capabilities.has_str("MOVE") {
                session
                    .uid_mv(&uid, destination)
                    .await
                    .map_err(|_| CommandError::retryable("operation.move_failed"))?;
            } else {
                session
                    .uid_copy(&uid, destination)
                    .await
                    .map_err(|_| CommandError::retryable("operation.copy_failed"))?;
                mark_deleted(&mut session, &uid).await?;
                if capabilities.has_str("UIDPLUS") {
                    session
                        .uid_expunge(&uid)
                        .await
                        .map_err(|_| CommandError::retryable("operation.expunge_failed"))?
                        .try_collect::<Vec<_>>()
                        .await
                        .map_err(|_| CommandError::retryable("operation.expunge_failed"))?;
                } else {
                    cleanup_pending = true;
                }
            }
        }
        RemoteOperationKind::Delete => {
            mark_deleted(&mut session, &uid).await?;
            if capabilities.has_str("UIDPLUS") {
                session
                    .uid_expunge(&uid)
                    .await
                    .map_err(|_| CommandError::retryable("operation.expunge_failed"))?
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(|_| CommandError::retryable("operation.expunge_failed"))?;
            } else {
                cleanup_pending = true;
            }
        }
    }
    let _ = session.logout().await;
    Ok(RemoteOperationOutcome { cleanup_pending })
}

async fn store_flag_delta<T>(
    session: &mut Session<T>,
    uid: &str,
    base_modseq: Option<u64>,
    condstore: bool,
    action: &str,
    flag: &str,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let query = conditional_store_query(base_modseq.filter(|_| condstore), action, flag);
    let updates = session
        .uid_store(uid, query)
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?;
    if updates.iter().any(|update| update.uid.is_some()) {
        return Ok(());
    }
    if !condstore {
        return Err(CommandError::retryable("operation.store_failed"));
    }
    let latest = session
        .uid_fetch(uid, "(UID FLAGS MODSEQ)")
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?;
    let latest_modseq = latest
        .iter()
        .find_map(|message| message.modseq)
        .ok_or_else(|| CommandError::new("operation.message_missing"))?;
    let retry_query = conditional_store_query(Some(latest_modseq), action, flag);
    let retry_updates = session
        .uid_store(uid, retry_query)
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?;
    if retry_updates.iter().any(|update| update.uid.is_some()) {
        Ok(())
    } else {
        Err(CommandError::retryable("operation.flag_conflict"))
    }
}

fn conditional_store_query(base_modseq: Option<u64>, action: &str, flag: &str) -> String {
    match base_modseq {
        Some(modseq) => format!("(UNCHANGEDSINCE {modseq}) {action} ({flag})"),
        None => format!("{action} ({flag})"),
    }
}

async fn mark_deleted<T>(session: &mut Session<T>, uid: &str) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    session
        .uid_store(uid, "+FLAGS.SILENT (\\Deleted)")
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("operation.store_failed"))?;
    Ok(())
}

async fn append_message_session<T>(
    mut session: Session<T>,
    mailbox_name: &str,
    flags: &str,
    raw: &[u8],
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if let Some(message_id) = MessageParser::default()
        .parse(raw)
        .and_then(|message| message.message_id().map(str::to_owned))
        .filter(|value| {
            !value.is_empty()
                && !value
                    .chars()
                    .any(|character| matches!(character, '"' | '\\' | '\r' | '\n'))
        })
    {
        session
            .select(mailbox_name)
            .await
            .map_err(|_| CommandError::retryable("operation.mailbox_open_failed"))?;
        let existing = session
            .uid_search(format!("HEADER Message-ID \"{message_id}\""))
            .await
            .map_err(|_| CommandError::retryable("operation.sent_search_failed"))?;
        if !existing.is_empty() {
            let _ = session.logout().await;
            return Ok(());
        }
    }
    session
        .append(mailbox_name, Some(flags), None, raw)
        .await
        .map_err(|_| CommandError::retryable("operation.append_failed"))?;
    let _ = session.logout().await;
    Ok(())
}

async fn replace_draft_session<T>(
    mut session: Session<T>,
    mailbox_name: &str,
    draft_id: &str,
    raw: &[u8],
) -> CommandResult<RemoteOperationOutcome>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if !draft_id
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || value == '-')
    {
        return Err(CommandError::new("draft.id_invalid"));
    }
    let capabilities = session
        .capabilities()
        .await
        .map_err(|_| CommandError::retryable("operation.capability_failed"))?;
    session
        .select(mailbox_name)
        .await
        .map_err(|_| CommandError::retryable("operation.mailbox_open_failed"))?;
    let mut old_uids = session
        .uid_search(format!("HEADER X-NextMail-Draft-ID \"{draft_id}\""))
        .await
        .map_err(|_| CommandError::retryable("operation.draft_search_failed"))?
        .into_iter()
        .collect::<Vec<_>>();
    old_uids.sort_unstable();
    session
        .append(mailbox_name, Some("(\\Draft)"), None, raw)
        .await
        .map_err(|_| CommandError::retryable("operation.append_failed"))?;
    let mut cleanup_pending = false;
    if !old_uids.is_empty() {
        let uid_set = old_uids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        mark_deleted(&mut session, &uid_set).await?;
        if capabilities.has_str("UIDPLUS") {
            session
                .uid_expunge(&uid_set)
                .await
                .map_err(|_| CommandError::retryable("operation.expunge_failed"))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|_| CommandError::retryable("operation.expunge_failed"))?;
        } else {
            cleanup_pending = true;
        }
    }
    let _ = session.logout().await;
    Ok(RemoteOperationOutcome { cleanup_pending })
}

async fn wait_for_change_session<T>(
    mut session: Session<T>,
    timeout: Duration,
) -> CommandResult<InboxWatchOutcome>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let capabilities = session
        .capabilities()
        .await
        .map_err(|_| CommandError::retryable("sync.imap_capability_failed"))?;
    if !capabilities.has_str("IDLE") {
        let _ = session.logout().await;
        return Ok(InboxWatchOutcome::Unsupported);
    }
    session
        .examine("INBOX")
        .await
        .map_err(|_| CommandError::retryable("sync.mailbox_open_failed"))?;
    let mut handle = session.idle();
    handle
        .init()
        .await
        .map_err(|_| CommandError::retryable("sync.idle_failed"))?;
    let outcome = {
        let (wait, _) = handle.wait_with_timeout(timeout);
        wait.await
            .map_err(|_| CommandError::retryable("sync.idle_failed"))?
    };
    let mut session = handle
        .done()
        .await
        .map_err(|_| CommandError::retryable("sync.idle_failed"))?;
    let _ = session.logout().await;
    Ok(match outcome {
        IdleResponse::NewData(_) => InboxWatchOutcome::Changed,
        IdleResponse::Timeout | IdleResponse::ManualInterrupt => InboxWatchOutcome::Timeout,
    })
}

async fn fetch_message_session<T>(
    mut session: Session<T>,
    mailbox_name: &str,
    uid: u32,
    expected_uid_validity: u32,
) -> CommandResult<RemoteMessage>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let selected = session
        .examine(mailbox_name)
        .await
        .map_err(|_| CommandError::retryable("sync.mailbox_open_failed"))?;
    let uid_validity = selected.uid_validity.unwrap_or_default();
    if uid_validity == 0 || uid_validity != expected_uid_validity {
        return Err(CommandError::new("sync.uid_validity_changed"));
    }
    let message = fetch_remote_message(&mut session, uid, uid_validity).await?;
    let _ = session.logout().await;
    Ok(message)
}

async fn fetch_remote_message<T>(
    session: &mut Session<T>,
    uid: u32,
    uid_validity: u32,
) -> CommandResult<RemoteMessage>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut messages = session
        .uid_fetch(
            uid.to_string(),
            "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])",
        )
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?;
    let fetched = messages
        .pop()
        .filter(|message| message.uid == Some(uid))
        .ok_or_else(|| CommandError::new("sync.message_not_found"))?;
    let raw = fetched
        .body()
        .map(ToOwned::to_owned)
        .ok_or_else(|| CommandError::new("sync.message_body_missing"))?;
    let received_at = fetched
        .internal_date()
        .map(|value| value.timestamp())
        .unwrap_or_default();
    parse_message(
        uid,
        uid_validity,
        fetched.size.unwrap_or(raw.len() as u32) as u64,
        received_at,
        fetched.flags(),
        &[],
        Some(raw),
    )
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
        observer.notify(SyncNotice::Folders {
            completed: folder_index as u64,
            total: folder_total,
        });
        let selectable = !folder.attributes().contains(&NameAttribute::NoSelect);
        let name = folder.name().to_owned();
        let display_name = decode_modified_utf7(&name);
        let role = mailbox_role(&display_name, folder.attributes());
        if !selectable {
            sink.upsert_mailbox(
                &account.account_slot_id,
                &RemoteMailbox {
                    name,
                    display_name,
                    delimiter: folder.delimiter().map(str::to_owned),
                    role,
                    selectable: false,
                    uid_validity: 0,
                    uid_next: 0,
                    total_count: 0,
                    unread_count: 0,
                    highest_modseq: None,
                },
            )
            .await?;
            continue;
        }

        let selected = if condstore {
            session.select_condstore(&name).await
        } else {
            session.examine(&name).await
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
        let mailbox = sink
            .upsert_mailbox(
                &account.account_slot_id,
                &RemoteMailbox {
                    name: name.clone(),
                    display_name,
                    delimiter: folder.delimiter().map(str::to_owned),
                    role,
                    selectable: true,
                    uid_validity,
                    uid_next: selected.uid_next.unwrap_or_default(),
                    total_count: selected.exists,
                    unread_count: unseen.len() as u32,
                    highest_modseq: selected.highest_modseq,
                },
            )
            .await?;
        let mut uids = session
            .uid_search("ALL")
            .await
            .map_err(|_| CommandError::retryable("sync.mailbox_search_failed"))?
            .into_iter()
            .filter(|uid| *uid > mailbox.last_uid)
            .collect::<Vec<_>>();
        uids.sort_unstable();
        let total = uids.len() as u64;
        let mut completed = 0u64;
        let mut highest_uid = mailbox.last_uid;

        for batch in uids.chunks(FETCH_BATCH_SIZE) {
            let uid_set = batch
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let summary_query = if condstore {
                "(UID FLAGS MODSEQ INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
            } else {
                "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
            };
            let summaries = session
                .uid_fetch(&uid_set, summary_query)
                .await
                .map_err(|_| CommandError::retryable("sync.message_fetch_failed"))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|_| CommandError::retryable("sync.message_fetch_failed"))?;

            for summary in summaries {
                let Some(uid) = summary.uid else { continue };
                let received_at = summary
                    .internal_date()
                    .map(|value| value.timestamp())
                    .unwrap_or_default();
                let raw = if should_download_body(account.sync_policy.clone(), received_at) {
                    fetch_raw(&mut session, uid).await?
                } else {
                    None
                };
                let header = summary.header().unwrap_or_default();
                let mut message = parse_message(
                    uid,
                    uid_validity,
                    summary.size.unwrap_or_default() as u64,
                    received_at,
                    summary.flags(),
                    header,
                    raw,
                )?;
                message.modseq = summary.modseq;
                sink.upsert_message(&account.account_slot_id, &mailbox.id, &message)
                    .await?;
                highest_uid = highest_uid.max(uid);
                completed += 1;
                observer.notify(SyncNotice::Summaries { completed, total });
            }
        }

        let pending_bodies = sink
            .pending_body_locations(&mailbox.id, sync_policy_cutoff(account.sync_policy.clone()))
            .await?;
        let body_total = pending_bodies.len() as u64;
        for (index, location) in pending_bodies.into_iter().enumerate() {
            if location.uid_validity != uid_validity {
                continue;
            }
            let message = fetch_remote_message(&mut session, location.uid, uid_validity).await?;
            sink.upsert_message(&account.account_slot_id, &mailbox.id, &message)
                .await?;
            observer.notify(SyncNotice::Bodies {
                completed: index as u64 + 1,
                total: body_total,
            });
        }
        let flags_query = if condstore {
            "(UID FLAGS MODSEQ)"
        } else {
            "(UID FLAGS)"
        };
        let states = session
            .uid_fetch("1:*", flags_query)
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
        sink.reconcile_mailbox(&mailbox.id, uid_validity, selected.highest_modseq, &states)
            .await?;
        sink.complete_mailbox(&mailbox.id, highest_uid).await?;
        observer.notify(SyncNotice::MailboxChanged {
            mailbox_id: mailbox.id,
            revision: 0,
        });
    }
    observer.notify(SyncNotice::Folders {
        completed: folder_total,
        total: folder_total,
    });
    let _ = session.logout().await;
    Ok(())
}

async fn fetch_raw<T>(session: &mut Session<T>, uid: u32) -> CommandResult<Option<Vec<u8>>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let messages = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?;
    Ok(messages
        .into_iter()
        .find(|message| message.uid == Some(uid))
        .and_then(|message| message.body().map(ToOwned::to_owned)))
}

fn parse_message<'a>(
    uid: u32,
    uid_validity: u32,
    size: u64,
    received_at: i64,
    flags: impl Iterator<Item = Flag<'a>>,
    header: &[u8],
    raw: Option<Vec<u8>>,
) -> CommandResult<RemoteMessage> {
    let parsed = raw
        .as_deref()
        .and_then(|value| MessageParser::default().parse(value));
    let parsed_headers = if parsed.is_none() {
        MessageParser::default().parse_headers(header)
    } else {
        None
    };
    let message = parsed.as_ref().or(parsed_headers.as_ref());
    let flags = flags.collect::<Vec<_>>();
    let unread = !flags.iter().any(|flag| matches!(flag, Flag::Seen));
    let flagged = flags.iter().any(|flag| matches!(flag, Flag::Flagged));
    let plain_text = parsed
        .as_ref()
        .and_then(|message| message.body_text(0))
        .map(|value| value.into_owned());
    let sanitized = parsed
        .as_ref()
        .and_then(|message| message.body_html(0))
        .map(|value| sanitize_mail_html(&value));
    let attachments = parsed
        .as_ref()
        .map(attachment_summaries)
        .unwrap_or_default();
    let preview = parsed
        .as_ref()
        .and_then(|message| message.body_preview(180))
        .map(|value| value.into_owned())
        .unwrap_or_default();
    Ok(RemoteMessage {
        uid,
        uid_validity,
        subject: message
            .and_then(|message| message.subject())
            .unwrap_or_default()
            .to_owned(),
        from: message
            .and_then(|message| message.from())
            .map(addresses)
            .unwrap_or_default(),
        to: message
            .and_then(|message| message.to())
            .map(addresses)
            .unwrap_or_default(),
        cc: message
            .and_then(|message| message.cc())
            .map(addresses)
            .unwrap_or_default(),
        received_at: message
            .and_then(|message| message.date())
            .map(|value| value.to_timestamp())
            .unwrap_or(received_at),
        preview,
        unread,
        flagged,
        size,
        message_id: message
            .and_then(|message| message.message_id())
            .map(str::to_owned),
        references: message
            .and_then(|message| message.references().as_text_list())
            .map(|values| values.iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
        in_reply_to: message
            .and_then(|message| message.in_reply_to().as_text())
            .map(str::to_owned),
        plain_text,
        safe_html: sanitized.as_ref().map(|value| value.document.clone()),
        raw,
        attachments,
        remote_images_blocked: sanitized
            .as_ref()
            .is_some_and(|value| value.remote_images_blocked),
        modseq: None,
    })
}

fn attachment_summaries(message: &Message<'_>) -> Vec<RemoteAttachment> {
    message
        .attachments()
        .enumerate()
        .map(|(index, attachment)| {
            let content_type = attachment
                .content_type()
                .map(|value| {
                    format!(
                        "{}/{}",
                        value.ctype(),
                        value.subtype().unwrap_or("octet-stream")
                    )
                })
                .unwrap_or_else(|| "application/octet-stream".to_owned());
            RemoteAttachment {
                part_index: index as u32,
                file_name: attachment
                    .attachment_name()
                    .unwrap_or("attachment")
                    .to_owned(),
                content_type,
                size: attachment.len() as u64,
                content_id: attachment.content_id().map(str::to_owned),
            }
        })
        .collect()
}

fn addresses(address: &Address<'_>) -> Vec<MessageAddress> {
    address
        .iter()
        .filter_map(|value| {
            Some(MessageAddress {
                name: value.name.as_deref().map(str::to_owned),
                email: value.address.as_deref()?.to_owned(),
            })
        })
        .collect()
}

fn mailbox_role(name: &str, attributes: &[NameAttribute<'_>]) -> MailboxRole {
    if name.eq_ignore_ascii_case("INBOX") {
        return MailboxRole::Inbox;
    }
    for attribute in attributes {
        match attribute {
            NameAttribute::Archive => return MailboxRole::Archive,
            NameAttribute::Drafts => return MailboxRole::Drafts,
            NameAttribute::Junk => return MailboxRole::Junk,
            NameAttribute::Sent => return MailboxRole::Sent,
            NameAttribute::Trash => return MailboxRole::Trash,
            _ => {}
        }
    }
    match name.trim().to_ascii_lowercase().as_str() {
        "sent" | "sent items" | "sent messages" => MailboxRole::Sent,
        "draft" | "drafts" => MailboxRole::Drafts,
        "trash" | "deleted" | "deleted items" => MailboxRole::Trash,
        "junk" | "junk e-mail" | "junk email" | "spam" => MailboxRole::Junk,
        "archive" | "archives" => MailboxRole::Archive,
        _ => MailboxRole::Other,
    }
}

pub fn decode_modified_utf7(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(relative_start) = input[cursor..].find('&') {
        let start = cursor + relative_start;
        output.push_str(&input[cursor..start]);
        let Some(relative_end) = input[start + 1..].find('-') else {
            output.push_str(&input[start..]);
            return output;
        };
        let end = start + 1 + relative_end;
        let encoded = &input[start + 1..end];
        if encoded.is_empty() {
            output.push('&');
        } else if let Some(decoded) = decode_modified_utf7_segment(encoded) {
            output.push_str(&decoded);
        } else {
            output.push_str(&input[start..=end]);
        }
        cursor = end + 1;
    }

    output.push_str(&input[cursor..]);
    output
}

fn decode_modified_utf7_segment(encoded: &str) -> Option<String> {
    let mut standard = encoded.replace(',', "/");
    while !standard.len().is_multiple_of(4) {
        standard.push('=');
    }
    let bytes = STANDARD.decode(standard).ok()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let utf16 = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

fn should_download_body(policy: SyncPolicy, received_at: i64) -> bool {
    sync_policy_cutoff(policy).is_none_or(|cutoff| received_at >= cutoff)
}

fn sync_policy_cutoff(policy: SyncPolicy) -> Option<i64> {
    let days = match policy {
        SyncPolicy::Days30 => 30,
        SyncPolicy::Days90 => 90,
        SyncPolicy::Days365 => 365,
        SyncPolicy::All => return None,
    };
    Some(now().saturating_sub(Duration::from_secs(days * 86_400).as_secs() as i64))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn connect_tls(
    host: &str,
    stream: TcpStream,
) -> CommandResult<tokio_rustls::client::TlsStream<TcpStream>> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err(CommandError::new("sync.system_certificates_unavailable"));
    }
    let mut roots = RootCertStore::empty();
    for certificate in native.certs {
        let _ = roots.add(certificate);
    }
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.to_owned())
        .map_err(|_| CommandError::new("sync.server_name_invalid"))?;
    TlsConnector::from(Arc::new(config))
        .connect(server_name, stream)
        .await
        .map_err(|_| CommandError::retryable("sync.imap_tls_failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn sync_policy_uses_expected_window() {
        assert!(should_download_body(SyncPolicy::All, 0));
        assert!(should_download_body(SyncPolicy::Days30, now()));
        assert!(!should_download_body(SyncPolicy::Days30, 0));
    }

    #[test]
    fn decodes_modified_utf7_mailbox_names() {
        assert_eq!(decode_modified_utf7("INBOX"), "INBOX");
        assert_eq!(decode_modified_utf7("A&-B"), "A&B");
        assert_eq!(decode_modified_utf7("&U,BTFw-"), "台北");
        assert_eq!(decode_modified_utf7("&ZeVnLIqe-"), "日本語");
        assert_eq!(mailbox_role("Drafts", &[]), MailboxRole::Drafts);
        assert_eq!(mailbox_role("Sent Items", &[]), MailboxRole::Sent);
    }
}

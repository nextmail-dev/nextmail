mod connection;
mod encoding;
mod parse;
mod path_lock;
mod provider;
mod session;
mod session_budget;
mod timeout;

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

pub use encoding::decode_modified_utf7;
use encoding::{mailbox_leaf_display_name, mailbox_role};
use parse::{message_flag_state, parse_message_in_background, MessageParseInput};
pub use provider::AsyncImapProvider;

use crate::core::{
    CommandError, CommandResult, ContentAvailability, ImapAccountConfig, MailSyncSink, MailboxRole,
    MailboxSyncTarget, MessageListItem, RemoteMailbox, RemoteMessage, RemoteMessageState,
    StoredMailbox, StoredMessageLocation, SyncNotice, SyncObserver,
};
use async_imap::{
    types::{Flag, NameAttribute},
    Session,
};
use futures_util::future::join_all;
use futures_util::TryStreamExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};

const FETCH_BATCH_SIZE: usize = 1;
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
    default_notification_enabled: bool,
}

struct FolderDescriptor {
    name: String,
    display_name: String,
    progress_name: String,
    delimiter: Option<String>,
    role: MailboxRole,
    selectable: bool,
}

async fn sync_session<T>(
    mut pool: Vec<Session<T>>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let folders = pool[0]
        .list(Some(""), Some("*"))
        .await
        .map_err(map_imap_err("sync.folder_list_failed", true))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_imap_err("sync.folder_list_failed", true))?;
    let capabilities = pool[0]
        .capabilities()
        .await
        .map_err(map_imap_err("sync.imap_capability_failed", true))?;
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
            &mut pool,
            account,
            sink,
            observer,
            condstore,
            account.download_full_messages,
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
    for mut session in pool {
        let _ = session.logout().await;
    }
    Ok(())
}

async fn sync_mailbox_session<T>(
    mut session: Session<T>,
    account: &ImapAccountConfig,
    mailbox: &MailboxSyncTarget,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let capabilities = session
        .capabilities()
        .await
        .map_err(map_imap_err("sync.imap_capability_failed", true))?;
    let condstore = capabilities.has_str("CONDSTORE");
    let mut sessions = vec![session];
    let result = sync_folder(
        &mut sessions,
        account,
        sink,
        observer,
        condstore,
        false,
        FolderDescriptor {
            name: mailbox.name.clone(),
            display_name: mailbox.display_name.clone(),
            progress_name: mailbox_leaf_display_name(
                &mailbox.display_name,
                mailbox.delimiter.as_deref(),
            )
            .to_owned(),
            delimiter: mailbox.delimiter.clone(),
            role: mailbox.role.clone(),
            selectable: true,
        },
    )
    .await;
    if let Some(mut session) = sessions.pop() {
        let _ = session.logout().await;
    }
    result
}

async fn sync_folder<T>(
    sessions: &mut [Session<T>],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    condstore: bool,
    download_full_messages: bool,
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

    // Enter the mailbox on every worker session so each can fetch from it.
    // Session 0 also supplies the selected metadata used below.
    let mut selected = None;
    for (index, session) in sessions.iter_mut().enumerate() {
        let mailbox = if condstore {
            session.select_condstore(&folder.name).await
        } else {
            session.examine(&folder.name).await
        }
        .map_err(map_imap_err("sync.mailbox_open_failed", true))?;
        if index == 0 {
            selected = Some(mailbox);
        }
    }
    let selected = selected.expect("at least one worker session");
    let uid_validity = selected.uid_validity.unwrap_or_default();
    if uid_validity == 0 {
        return Err(CommandError::new("sync.uid_not_supported"));
    }
    let unseen = sessions[0]
        .uid_search("UNSEEN")
        .await
        .map_err(map_imap_err("sync.mailbox_search_failed", true))?;
    let highest_modseq = selected.highest_modseq;
    let default_notification_enabled = folder.role == MailboxRole::Inbox;
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
        default_notification_enabled,
    };

    // Resumable sync: fetch only UIDs we don't already have a stored location
    // for. The previous `uid > last_uid` high-water mark only advanced on
    // full-folder completion (complete_mailbox), so any mid-folder failure left
    // last_uid at 0 and the next run refetched everything from 1. Diffing
    // against stored UIDs lets a failed run resume where it stopped. It is also
    // correct under contiguous chunking: a mid-chunk worker failure leaves a
    // gap that the next run simply fills in, rather than skipping it forever.
    let remote_uids = sessions[0]
        .uid_search("ALL")
        .await
        .map_err(map_imap_err("sync.mailbox_search_failed", true))?;
    let stored: HashSet<u32> = sink
        .stored_uids(&context.mailbox.id, uid_validity)
        .await?
        .into_iter()
        .collect();
    let mut uids: Vec<u32> = remote_uids
        .into_iter()
        .filter(|uid| !stored.contains(uid))
        .collect();
    uids.sort_unstable();
    let total = uids.len() as u64;
    let completed = AtomicU64::new(0);
    let chunks = split_uids(&uids, sessions.len());
    // SQLite serializes all writers through a single lock even in WAL mode.
    // Three worker sessions each opening a write transaction contend on that
    // lock and surface "database is locked"; this mutex serializes only the
    // upsert (the DB write) while workers keep fetching headers in parallel
    // over their own IMAP connections - the network-bound part stays
    // concurrent, the write-bound part does not.
    let write_lock = Mutex::new(());
    let results = join_all(sessions.iter_mut().enumerate().map(|(i, session)| {
        fetch_summaries_worker(
            session,
            &chunks[i],
            account,
            sink,
            observer,
            &context,
            condstore,
            &completed,
            total,
            &write_lock,
        )
    }))
    .await;
    let mut highest_uid = context.mailbox.last_uid;
    for result in results {
        highest_uid = highest_uid.max(result?);
    }

    if download_full_messages {
        fetch_missing_bodies(sessions, account, sink, observer, &context, &write_lock).await?;
    }

    reconcile_flags(
        &mut sessions[0],
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

fn split_uids(uids: &[u32], n: usize) -> Vec<Vec<u32>> {
    let n = n.max(1);
    let chunk_size = uids.len().div_ceil(n).max(1);
    let mut chunks: Vec<Vec<u32>> = Vec::with_capacity(n);
    let mut iter = uids.chunks(chunk_size);
    for _ in 0..n {
        chunks.push(iter.next().unwrap_or(&[]).to_vec());
    }
    chunks
}

#[allow(clippy::too_many_arguments)]
async fn fetch_summaries_worker<T>(
    session: &mut Session<T>,
    uids: &[u32],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    condstore: bool,
    completed: &AtomicU64,
    total: u64,
    write_lock: &Mutex<()>,
) -> CommandResult<u32>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut highest_uid = context.mailbox.last_uid;
    for batch in uids.chunks(FETCH_BATCH_SIZE) {
        let summaries = fetch_summary_batch(session, batch, condstore).await?;

        // Header-only: store the summary now (subject/sender/date/flags) so the
        // list appears immediately; the body — and with it the preview — is
        // fetched on demand when the message is opened. This keeps each summary
        // fetch to a single header round-trip and minimizes data transferred
        // during sync.
        for summary in summaries {
            let mut message = parse_message_in_background(MessageParseInput {
                uid: summary.uid,
                uid_validity: context.uid_validity,
                size: summary.size,
                received_at: summary.received_at,
                unread: summary.unread,
                flagged: summary.flagged,
                header: summary.header,
                raw: None,
            })
            .await?;
            message.modseq = summary.modseq;
            // Serialize the DB write across workers (see sync_folder). The guard
            // is held only for the upsert; parsing and observer notifies stay
            // outside the lock so a slow write never blocks another worker's
            // fetch or notification.
            let outcome = {
                let _write_guard = write_lock.lock().await;
                sink.upsert_message(&account.account_slot_id, &context.mailbox.id, &message)
                    .await?
            };
            if outcome.is_new_location {
                observer.notify(SyncNotice::MessageArrived {
                    mailbox_id: context.mailbox.id.clone(),
                    item: message_list_item_from_remote(
                        context.mailbox.id.clone(),
                        &message,
                        outcome.message_id.clone(),
                    ),
                });
                if message.unread && !context.mailbox.notification_baseline_required {
                    let sender = message.from.first();
                    observer.notify(SyncNotice::NewMessageCandidate {
                        mailbox_id: context.mailbox.id.clone(),
                        message_id: outcome.message_id,
                        sender_name: sender.and_then(|address| address.name.clone()),
                        sender_email: sender
                            .map_or_else(String::new, |address| address.email.clone()),
                        subject: message.subject.clone(),
                        default_enabled: context.default_notification_enabled,
                    });
                }
            }
            highest_uid = highest_uid.max(summary.uid);
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            observer.notify(SyncNotice::Summaries {
                completed: done,
                total,
                mailbox_name: context.mailbox_name.to_owned(),
            });
        }
    }
    Ok(highest_uid)
}

async fn fetch_missing_bodies<T>(
    sessions: &mut [Session<T>],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    write_lock: &Mutex<()>,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let uids = pending_body_uids(
        sink.pending_body_locations(&context.mailbox.id, None)
            .await?,
        context.uid_validity,
    );
    let total = uids.len() as u64;
    if total == 0 {
        return Ok(());
    }

    let completed = AtomicU64::new(0);
    let chunks = split_uids(&uids, sessions.len());
    let results = join_all(sessions.iter_mut().enumerate().map(|(index, session)| {
        fetch_bodies_worker(
            session,
            &chunks[index],
            account,
            sink,
            observer,
            context,
            &completed,
            total,
            write_lock,
        )
    }))
    .await;
    for result in results {
        result?;
    }
    Ok(())
}

fn pending_body_uids(locations: Vec<StoredMessageLocation>, current_uid_validity: u32) -> Vec<u32> {
    let mut uids = locations
        .into_iter()
        .filter(|location| location.uid_validity == current_uid_validity)
        .map(|location| location.uid)
        .collect::<Vec<_>>();
    uids.sort_unstable();
    uids
}

#[allow(clippy::too_many_arguments)]
async fn fetch_bodies_worker<T>(
    session: &mut Session<T>,
    uids: &[u32],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    completed: &AtomicU64,
    total: u64,
    write_lock: &Mutex<()>,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    for uid in uids {
        let messages = session::fetch_remote_messages(
            session,
            std::slice::from_ref(uid),
            context.uid_validity,
        )
        .await?;
        for message in messages {
            {
                let _write_guard = write_lock.lock().await;
                sink.upsert_message(&account.account_slot_id, &context.mailbox.id, &message)
                    .await?;
            }
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            observer.notify(SyncNotice::Bodies {
                completed: done,
                total,
                mailbox_name: context.mailbox_name.to_owned(),
            });
        }
    }
    Ok(())
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
        .map_err(map_imap_err("sync.message_fetch_failed", true))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_imap_err("sync.message_fetch_failed", true))?
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
        .map_err(map_imap_err("sync.flags_fetch_failed", true))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_imap_err("sync.flags_fetch_failed", true))?
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

fn message_list_item_from_remote(
    mailbox_id: String,
    message: &RemoteMessage,
    message_id: String,
) -> MessageListItem {
    MessageListItem {
        id: message_id,
        mailbox_id,
        subject: message.subject.clone(),
        from: message.from.clone(),
        received_at: message.received_at,
        preview: message.preview.clone(),
        unread: message.unread,
        flagged: message.flagged,
        has_attachments: !message.attachments.is_empty(),
        body_availability: if message.plain_text.is_some() || message.safe_html.is_some() {
            ContentAvailability::Available
        } else {
            ContentAvailability::Missing
        },
        pending_operation: false,
    }
}

pub(super) fn format_uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

// Wraps a swallowed IMAP/storage error into a `CommandError` while preserving
// the underlying cause in the log. Without this the original io/imap error is
// discarded by `.map_err(|_| ...)` and "同步失败" carries no diagnostics.
fn map_imap_err<E: std::fmt::Debug>(
    code: &'static str,
    retryable: bool,
) -> impl FnOnce(E) -> CommandError {
    move |error| {
        tracing::warn!(%code, ?error, "imap operation failed");
        if retryable {
            CommandError::retryable(code)
        } else {
            CommandError::new(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::imap::parse::parse_message;
    use mail_parser::MessageParser;

    #[test]
    fn formats_a_batch_as_one_uid_set() {
        assert_eq!(format_uid_set(&[3, 7, 9]), "3,7,9");
        assert_eq!(format_uid_set(&[]), "");
    }

    #[test]
    fn split_uids_partitions_disjointly_and_covers_all_workers() {
        // Contiguous, disjoint, every UID present exactly once.
        let chunks = split_uids(&(1..=10).collect::<Vec<_>>(), 3);
        assert_eq!(chunks.len(), 3);
        let mut all: Vec<u32> = chunks.iter().flatten().copied().collect();
        all.sort_unstable();
        assert_eq!(all, (1..=10).collect::<Vec<_>>());
        // Fewer messages than workers -> some chunks empty, count still matches.
        let sparse = split_uids(&[7u32], 3);
        assert_eq!(sparse.len(), 3);
        assert_eq!(sparse.iter().flatten().copied().sum::<u32>(), 7);
        // No messages at all -> no panic, n empty chunks (workers no-op).
        let empty = split_uids(&[], 3);
        assert_eq!(empty.len(), 3);
        assert!(empty.iter().all(Vec::is_empty));
    }

    #[test]
    fn full_message_sync_fetches_only_missing_bodies_for_current_uid_validity() {
        let uids = pending_body_uids(
            vec![
                StoredMessageLocation {
                    uid: 9,
                    uid_validity: 2,
                },
                StoredMessageLocation {
                    uid: 3,
                    uid_validity: 2,
                },
                StoredMessageLocation {
                    uid: 1,
                    uid_validity: 1,
                },
            ],
            2,
        );
        assert_eq!(uids, vec![3, 9]);
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
            "<p>Logo <img src=\"CID:logo%40example.test\"></p>\r\n",
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

        let safe_html = message.safe_html.expect("safe HTML");
        assert!(
            safe_html.contains("data:image/png;base64,aW1hZ2U="),
            "unexpected safe HTML: {safe_html}"
        );
        assert!(message.attachments.is_empty());
    }

    #[test]
    fn embeds_aliyun_style_content_ids_without_listing_them_as_attachments() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Inline image\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<img src=\"cid:__aliyun178512290634140581\">\r\n",
            "--nextmail\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Disposition: attachment; filename=image.png\r\n",
            "Content-ID: <__aliyun178512290634140581>\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\n",
            "iVBORw0KGgo=\r\n",
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

        let safe_html = message.safe_html.expect("safe HTML");
        assert!(
            safe_html.contains("data:image/png;base64,iVBORw0KGgo="),
            "unexpected safe HTML: {safe_html}"
        );
        assert!(message.attachments.is_empty());
    }

    #[test]
    fn leaves_non_image_octet_stream_cid_parts_as_attachments() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Invalid inline image\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<img src=\"cid:__aliyun178512290634140581\">\r\n",
            "--nextmail\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Disposition: inline; filename=image.png\r\n",
            "Content-ID: <__aliyun178512290634140581>\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\n",
            "bm90LWEtcG5n\r\n",
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

        assert!(!message.safe_html.expect("safe HTML").contains("data:image"));
        assert_eq!(message.attachments.len(), 1);
    }

    #[test]
    fn leaves_unreferenced_content_id_parts_in_the_attachment_list() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Unreferenced image\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>No inline image reference</p>\r\n",
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

        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].file_name, "logo.png");
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

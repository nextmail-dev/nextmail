mod connection;
mod encoding;
mod parse;
mod path_lock;
mod provider;
mod session;
mod session_budget;
mod structure;
mod timeout;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

pub use encoding::decode_modified_utf7;
use encoding::{mailbox_leaf_display_name, mailbox_role};
use parse::{message_flag_state, parse_message_in_background, MessageParseInput};
pub use provider::AsyncImapProvider;

use crate::core::{
    AddressPresentation, CommandError, CommandResult, ContentAvailability, ImapAccountConfig,
    MailSyncSink, MailboxRole, MailboxSyncTarget, MessageListItem, RemoteAttachment, RemoteMailbox,
    RemoteMessage, RemoteMessageState, StoredMailbox, StoredMessageLocation, SyncNotice,
    SyncObserver,
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

const FETCH_BATCH_SIZE: usize = 20;
pub const SELECTIVE_FETCH_UNSUPPORTED: &str = "sync.message_selective_fetch_unsupported";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BodystructureMode {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyncSessionOutcome {
    Complete,
    BodystructureIncompatible,
}
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
    bodystructure_mode: BodystructureMode,
}

struct FolderSyncOptions {
    condstore: bool,
    download_full_messages: bool,
    bodystructure_mode: BodystructureMode,
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
    pool: &mut [Session<T>],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    bodystructure_mode: BodystructureMode,
) -> CommandResult<SyncSessionOutcome>
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

    let descriptors = folders
        .into_iter()
        .map(|folder| {
            let name = folder.name().to_owned();
            let display_name = decode_modified_utf7(&name);
            let delimiter = folder.delimiter().map(str::to_owned);
            FolderDescriptor {
                role: mailbox_role(&display_name, folder.attributes()),
                selectable: !folder.attributes().contains(&NameAttribute::NoSelect),
                progress_name: mailbox_leaf_display_name(&display_name, delimiter.as_deref())
                    .to_owned(),
                delimiter,
                name,
                display_name,
            }
        })
        .collect::<Vec<_>>();

    // Pre-create the whole folder tree before per-folder message sync so the
    // sidebar shows the structure immediately instead of folders appearing
    // one by one. ensure_mailbox only inserts missing rows, so later syncs
    // are a no-op and stored sync metadata is never clobbered with
    // preliminary values.
    precreate_folder_tree(sink, &account.account_slot_id, &descriptors, observer).await?;

    let folder_total = descriptors.len() as u64;
    for (folder_index, folder) in descriptors.into_iter().enumerate() {
        observer.notify(SyncNotice::Folders {
            completed: folder_index as u64,
            total: folder_total,
            mailbox_name: Some(folder.progress_name.clone()),
        });
        let bodystructure_incompatible = sync_folder(
            pool,
            account,
            sink,
            observer,
            FolderSyncOptions {
                condstore,
                download_full_messages: account.download_full_messages,
                bodystructure_mode,
            },
            folder,
        )
        .await?;
        if bodystructure_incompatible {
            return Ok(SyncSessionOutcome::BodystructureIncompatible);
        }
    }
    observer.notify(SyncNotice::Folders {
        completed: folder_total,
        total: folder_total,
        mailbox_name: None,
    });
    Ok(SyncSessionOutcome::Complete)
}

async fn precreate_folder_tree(
    sink: &(dyn MailSyncSink + Send + Sync),
    account_slot_id: &str,
    descriptors: &[FolderDescriptor],
    observer: &(dyn SyncObserver + Send + Sync),
) -> CommandResult<()> {
    for descriptor in descriptors {
        if let Some(mailbox) = sink
            .ensure_mailbox(
                account_slot_id,
                &RemoteMailbox {
                    name: descriptor.name.clone(),
                    display_name: descriptor.display_name.clone(),
                    delimiter: descriptor.delimiter.clone(),
                    role: descriptor.role.clone(),
                    selectable: descriptor.selectable,
                    uid_validity: 0,
                    uid_next: 0,
                    total_count: 0,
                    unread_count: 0,
                    highest_modseq: None,
                },
            )
            .await?
        {
            notify_mailbox(observer, mailbox.id);
        }
    }
    Ok(())
}

async fn sync_mailbox_session<T>(
    mut session: Session<T>,
    account: &ImapAccountConfig,
    mailbox: &MailboxSyncTarget,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    bodystructure_mode: BodystructureMode,
) -> CommandResult<SyncSessionOutcome>
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
        FolderSyncOptions {
            condstore,
            download_full_messages: false,
            bodystructure_mode,
        },
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
    if result.as_ref().is_ok_and(|incompatible| !incompatible) {
        if let Some(mut session) = sessions.pop() {
            let _ = session.logout().await;
        }
    }
    result.map(|bodystructure_incompatible| {
        if bodystructure_incompatible {
            SyncSessionOutcome::BodystructureIncompatible
        } else {
            SyncSessionOutcome::Complete
        }
    })
}

async fn sync_folder<T>(
    sessions: &mut [Session<T>],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    options: FolderSyncOptions,
    folder: FolderDescriptor,
) -> CommandResult<bool>
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
        return Ok(false);
    }

    // Enter the mailbox on every worker session so each can fetch from it.
    // Session 0 also supplies the selected metadata used below.
    let mut selected = None;
    for (index, session) in sessions.iter_mut().enumerate() {
        let mailbox = if options.condstore {
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
        bodystructure_mode: options.bodystructure_mode,
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
        .iter()
        .copied()
        .filter(|uid| !stored.contains(uid))
        .collect();
    uids.sort_unstable();
    let total = uids.len() as u64;
    let completed = AtomicU64::new(0);
    let batches = Mutex::new(
        uids.chunks(FETCH_BATCH_SIZE)
            .map(<[u32]>::to_vec)
            .collect::<VecDeque<_>>(),
    );
    // SQLite serializes all writers through a single lock even in WAL mode.
    // Three worker sessions each opening a write transaction contend on that
    // lock and surface "database is locked"; this mutex serializes only the
    // upsert (the DB write) while workers keep fetching headers in parallel
    // over their own IMAP connections - the network-bound part stays
    // concurrent, the write-bound part does not.
    let write_lock = Mutex::new(());
    let results = join_all(sessions.iter_mut().map(|session| {
        fetch_summaries_worker(
            session,
            &batches,
            account,
            sink,
            observer,
            &context,
            options.condstore,
            &completed,
            total,
            &write_lock,
        )
    }))
    .await;
    let mut highest_uid = context.mailbox.last_uid;
    let mut sessions_usable = Vec::with_capacity(results.len());
    for result in results {
        let (worker_highest, usable) = result?;
        highest_uid = highest_uid.max(worker_highest);
        sessions_usable.push(usable);
    }

    let mut bodystructure_incompatible = sessions_usable.iter().any(|usable| !usable);
    if options.download_full_messages && !bodystructure_incompatible {
        let mut live_sessions = sessions
            .iter_mut()
            .zip(&sessions_usable)
            .filter_map(|(session, &usable)| usable.then_some(session))
            .collect::<Vec<_>>();
        if live_sessions.is_empty() {
            tracing::warn!(
                mailbox_name = %context.mailbox_name,
                "no usable session left for body prefetch; it will resume next sync"
            );
        } else {
            bodystructure_incompatible = fetch_missing_bodies(
                &mut live_sessions,
                account,
                sink,
                observer,
                &context,
                &write_lock,
                &remote_uids,
            )
            .await?;
        }
    }

    if sessions_usable[0] {
        reconcile_flags(
            &mut sessions[0],
            sink,
            options.condstore,
            uid_validity,
            highest_modseq,
            &mailbox,
        )
        .await?;
    } else {
        tracing::warn!(
            mailbox_name = %context.mailbox_name,
            "primary sync session became unusable; skipping flag reconciliation this round"
        );
    }
    sink.complete_mailbox(&mailbox.id, highest_uid).await?;
    notify_mailbox(observer, mailbox.id);
    Ok(bodystructure_incompatible)
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
    batches: &Mutex<VecDeque<Vec<u32>>>,
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    condstore: bool,
    completed: &AtomicU64,
    total: u64,
    write_lock: &Mutex<()>,
) -> CommandResult<(u32, bool)>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut highest_uid = context.mailbox.last_uid;
    let mut session_usable = true;
    loop {
        let Some(batch) = batches.lock().await.pop_front() else {
            break;
        };
        let query = if condstore {
            "(UID FLAGS MODSEQ INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
        } else {
            "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
        };
        let mut summaries = session
            .uid_fetch(format_uid_set(&batch), query)
            .await
            .map_err(map_imap_err("sync.message_fetch_failed", true))?;

        // Header-only: store the summary now (subject/sender/date/flags) so the
        // list appears immediately; the body — and with it the preview — is
        // fetched on demand when the message is opened. Consume the FETCH
        // response stream directly so every received header is committed even
        // when the connection fails before the rest of the batch arrives.
        let mut batch_messages: Vec<(u32, RemoteMessage)> = Vec::with_capacity(batch.len());
        while let Some(summary) = summaries
            .try_next()
            .await
            .map_err(map_imap_err("sync.message_fetch_failed", true))?
        {
            let Some(uid) = summary.uid else {
                continue;
            };
            let received_at = summary
                .internal_date()
                .map(|value| value.timestamp())
                .unwrap_or_default();
            let (unread, flagged) = message_flag_state(summary.flags());
            let summary = FetchedMessageSummary {
                uid,
                received_at,
                unread,
                flagged,
                header: summary.header().unwrap_or_default().to_vec(),
                size: summary.size.unwrap_or_default() as u64,
                modseq: summary.modseq,
            };
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
            if outcome.contacts_changed {
                observer.notify(SyncNotice::ContactsChanged);
            }
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
            batch_messages.push((uid, message));
        }
        drop(summaries);

        // BODYSTRUCTURE rides a separate FETCH so a server grammar quirk in it
        // can't fail the header batch. Some servers (e.g. QQ Mail delivery
        // status reports) send NIL for body-fld-enc; imap-proto 0.16 rejects
        // that and poisons the whole response stream, which previously aborted
        // the folder sync forever on the same UID. A failed BODYSTRUCTURE
        // fetch breaks the session, so this worker stops after the current
        // batch; committed headers keep the immediate reconnect cheap. The
        // retry disables BODYSTRUCTURE for this account for the rest of the
        // process. The error is not logged in full because it embeds the raw
        // server response.
        if context.bodystructure_mode == BodystructureMode::Disabled {
            continue;
        }
        match fetch_bodystructure_attachments(session, &batch).await {
            Ok(by_uid) => {
                for (uid, message) in &mut batch_messages {
                    let Some(attachments) = by_uid.get(uid) else {
                        continue;
                    };
                    if attachments.is_empty() {
                        continue;
                    }
                    message.attachments = attachments.clone();
                    let _write_guard = write_lock.lock().await;
                    sink.upsert_message(&account.account_slot_id, &context.mailbox.id, message)
                        .await?;
                }
            }
            Err(error) => {
                tracing::warn!(
                    code = %error.code,
                    mailbox_name = %context.mailbox_name,
                    batch = ?batch,
                    "bodystructure fetch failed; continuing without attachment metadata for this batch"
                );
                session_usable = false;
                break;
            }
        }
    }
    Ok((highest_uid, session_usable))
}

async fn fetch_bodystructure_attachments<T>(
    session: &mut Session<T>,
    batch: &[u32],
) -> CommandResult<HashMap<u32, Vec<RemoteAttachment>>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let responses = session
        .uid_fetch(format_uid_set(batch), "(UID BODYSTRUCTURE)")
        .await
        .map_err(|_| CommandError::retryable("sync.message_bodystructure_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_bodystructure_failed"))?;
    Ok(responses
        .iter()
        .filter_map(|fetched| {
            let uid = fetched.uid?;
            let attachments = fetched
                .bodystructure()
                .map(structure::analyze_bodystructure)
                .map(|structure| structure.attachments)
                .unwrap_or_default();
            Some((uid, attachments))
        })
        .collect())
}

async fn fetch_missing_bodies<T>(
    sessions: &mut [&mut Session<T>],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    write_lock: &Mutex<()>,
    remote_uids: &HashSet<u32>,
) -> CommandResult<bool>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let pending = pending_body_locations(
        sink.pending_body_locations(&context.mailbox.id, None)
            .await?,
        context.uid_validity,
    );
    // Only fetch bodies for messages the server still lists. A UID present
    // locally but absent from uid_search was expunged or moved since we stored
    // its header; reconcile_flags (run next) prunes the local stub.
    let pending_len = pending.len();
    let locations = pending
        .into_iter()
        .filter(|location| remote_uids.contains(&location.uid))
        .collect::<Vec<_>>();
    if locations.len() < pending_len {
        tracing::debug!(
            skipped = pending_len - locations.len(),
            mailbox_name = %context.mailbox_name,
            "skipped body fetch for messages no longer listed on server"
        );
    }
    let total = locations.len() as u64;
    if total == 0 {
        return Ok(false);
    }

    let completed = AtomicU64::new(0);
    let message_ids = locations
        .iter()
        .map(|location| (location.uid, location.message_id.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let uids = locations
        .iter()
        .map(|location| location.uid)
        .collect::<Vec<_>>();
    // Ordinary connection failures hand unprocessed UIDs back to surviving
    // workers. A parser-rejected BODYSTRUCTURE is different: the caller drops
    // the whole pool, reconnects once, and restarts in full-message mode so the
    // malformed response cannot poison each worker in turn.
    let mut queue = uids;
    let mut usable = sessions.iter_mut().collect::<Vec<_>>();
    while !queue.is_empty() && !usable.is_empty() {
        let chunks = split_uids(&queue, usable.len());
        let results = join_all(usable.iter_mut().enumerate().map(|(index, session)| {
            fetch_bodies_worker(
                session,
                &chunks[index],
                account,
                sink,
                observer,
                context,
                &message_ids,
                &completed,
                total,
                write_lock,
            )
        }))
        .await;
        let mut remaining = Vec::new();
        let mut surviving = Vec::new();
        let mut bodystructure_incompatible = false;
        for (index, result) in results.into_iter().enumerate() {
            let (session_usable, unprocessed, worker_incompatible) = result?;
            remaining.extend(unprocessed);
            bodystructure_incompatible |= worker_incompatible;
            if session_usable {
                surviving.push(index);
            }
        }
        if bodystructure_incompatible {
            return Ok(true);
        }
        if remaining.is_empty() {
            break;
        }
        queue = remaining;
        let mut next_usable = Vec::with_capacity(surviving.len());
        for index in surviving.into_iter().rev() {
            next_usable.push(usable.swap_remove(index));
        }
        usable = next_usable;
    }
    if !queue.is_empty() {
        tracing::warn!(
            remaining = queue.len(),
            mailbox_name = %context.mailbox_name,
            "no usable session left for body prefetch; remaining messages stay pending for the next sync"
        );
    }
    Ok(false)
}

fn pending_body_locations(
    mut locations: Vec<StoredMessageLocation>,
    current_uid_validity: u32,
) -> Vec<StoredMessageLocation> {
    locations.retain(|location| location.uid_validity == current_uid_validity);
    locations.sort_unstable_by_key(|location| location.uid);
    locations
}

#[allow(clippy::too_many_arguments)]
async fn fetch_bodies_worker<T>(
    session: &mut Session<T>,
    uids: &[u32],
    account: &ImapAccountConfig,
    sink: &(dyn MailSyncSink + Send + Sync),
    observer: &(dyn SyncObserver + Send + Sync),
    context: &FolderSyncContext<'_>,
    message_ids: &std::collections::HashMap<u32, &str>,
    completed: &AtomicU64,
    total: u64,
    write_lock: &Mutex<()>,
) -> CommandResult<(bool, Vec<u32>, bool)>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    for (index, uid) in uids.iter().enumerate() {
        if context.bodystructure_mode == BodystructureMode::Disabled {
            match session::fetch_remote_messages(
                session,
                std::slice::from_ref(uid),
                context.uid_validity,
            )
            .await
            {
                Ok(messages) => {
                    for message in messages {
                        let _write_guard = write_lock.lock().await;
                        sink.upsert_message(
                            &account.account_slot_id,
                            &context.mailbox.id,
                            &message,
                        )
                        .await?;
                    }
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    observer.notify(SyncNotice::Bodies {
                        completed: done,
                        total,
                        mailbox_name: context.mailbox_name.to_owned(),
                    });
                    continue;
                }
                Err(error) if is_message_unavailable_error(&error.code) => {
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    observer.notify(SyncNotice::Bodies {
                        completed: done,
                        total,
                        mailbox_name: context.mailbox_name.to_owned(),
                    });
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        match session::fetch_remote_message_body(session, *uid).await {
            Ok(body) => {
                let message_id = message_ids
                    .get(uid)
                    .ok_or_else(|| CommandError::new("message.not_found"))?;
                {
                    let _write_guard = write_lock.lock().await;
                    sink.replace_message_body(&account.account_slot_id, message_id, &body)
                        .await?;
                }
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                observer.notify(SyncNotice::Bodies {
                    completed: done,
                    total,
                    mailbox_name: context.mailbox_name.to_owned(),
                });
            }
            Err(error) if error.code == SELECTIVE_FETCH_UNSUPPORTED => {
                return Ok((false, uids[index..].to_vec(), true));
            }
            Err(error) if is_message_unavailable_error(&error.code) => {
                // Message vanished on the server after we stored its header;
                // reconcile_flags (run next) prunes the local stub, so skip
                // rather than fail the whole folder sync.
                tracing::warn!(
                    uid,
                    mailbox_name = %context.mailbox_name,
                    code = %error.code,
                    "message no longer available on server, skipping body fetch"
                );
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                observer.notify(SyncNotice::Bodies {
                    completed: done,
                    total,
                    mailbox_name: context.mailbox_name.to_owned(),
                });
            }
            Err(error) => return Err(error),
        }
    }
    Ok((true, Vec::new(), false))
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
        from: message
            .from
            .iter()
            .map(AddressPresentation::from_header)
            .collect(),
        received_at: message.received_at,
        preview: message.preview.clone(),
        unread: message.unread,
        flagged: message.flagged,
        high_priority: message.high_priority,
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

// A single message whose body can't be fetched right now is a per-message
// condition, not a connectivity or auth failure: the message was expunged or
// moved on the server after we stored its header. Body backfill skips these so
// one vanished message can't fail the whole folder sync; reconcile_flags prunes
// the local stub in the same run.
fn is_message_unavailable_error(code: &str) -> bool {
    matches!(code, "sync.message_not_found" | "sync.message_body_missing")
}

#[cfg(test)]
mod tests;

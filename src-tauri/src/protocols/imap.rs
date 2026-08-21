mod connection;
mod encoding;
mod parse;
mod path_lock;
mod provider;
mod session;
mod session_budget;
mod structure;
mod timeout;

use std::collections::{HashMap, HashSet};
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
        sync_folder(
            &mut pool,
            account,
            sink,
            observer,
            condstore,
            account.download_full_messages,
            folder,
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
        .iter()
        .copied()
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
    let mut sessions_usable = Vec::with_capacity(results.len());
    for result in results {
        let (worker_highest, usable) = result?;
        highest_uid = highest_uid.max(worker_highest);
        sessions_usable.push(usable);
    }

    if download_full_messages {
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
            fetch_missing_bodies(
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
            condstore,
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
) -> CommandResult<(u32, bool)>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut highest_uid = context.mailbox.last_uid;
    let mut session_usable = true;
    for batch in uids.chunks(FETCH_BATCH_SIZE) {
        let query = if condstore {
            "(UID FLAGS MODSEQ INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
        } else {
            "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])"
        };
        let mut summaries = session
            .uid_fetch(format_uid_set(batch), query)
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
        // batch; committed headers keep the diff short and the next sync round
        // picks up the rest. The error is not logged in full because it embeds
        // the raw server response.
        match fetch_bodystructure_attachments(session, batch).await {
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
) -> CommandResult<()>
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
        return Ok(());
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
    // A worker whose session dies mid-chunk (e.g. a BODYSTRUCTURE response the
    // parser rejects poisons the connection) hands its unprocessed UIDs back;
    // re-dispatch them to the sessions that survived so one stubborn message
    // can't starve the rest of the prefetch queue. The offending message
    // itself stays pending and is retried next round.
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
        for (index, result) in results.into_iter().enumerate() {
            let (session_usable, unprocessed) = result?;
            remaining.extend(unprocessed);
            if session_usable {
                surviving.push(index);
            }
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
    Ok(())
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
) -> CommandResult<(bool, Vec<u32>)>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    for (index, uid) in uids.iter().enumerate() {
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
                match session::fetch_remote_messages(
                    session,
                    std::slice::from_ref(uid),
                    context.uid_validity,
                )
                .await
                {
                    Ok(messages) => {
                        for message in messages {
                            {
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
                        }
                    }
                    Err(fallback_error) => {
                        // The structure fetch can kill the session outright
                        // (a BODYSTRUCTURE the parser rejects poisons the
                        // stream), so the full-message fallback on this
                        // session has no chance. Leave this message pending
                        // for the next sync and hand the rest of the chunk
                        // back to the caller for re-dispatch.
                        tracing::warn!(
                            uid,
                            mailbox_name = %context.mailbox_name,
                            code = %fallback_error.code,
                            "full-message fallback failed; leaving body pending for the next sync"
                        );
                        return Ok((false, uids[index + 1..].to_vec()));
                    }
                }
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
    Ok((true, Vec::new()))
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
mod tests {
    use super::*;
    use crate::core::ContactAddressRole;
    use crate::protocols::imap::parse::parse_message;
    use mail_parser::MessageParser;

    #[test]
    fn formats_a_batch_as_one_uid_set() {
        assert_eq!(format_uid_set(&[3, 7, 9]), "3,7,9");
        assert_eq!(format_uid_set(&[]), "");
    }

    #[test]
    fn caps_header_fetch_commands_at_twenty_uids() {
        let uids = (1..=45).collect::<Vec<_>>();
        assert_eq!(
            uids.chunks(FETCH_BATCH_SIZE)
                .map(<[u32]>::len)
                .collect::<Vec<_>>(),
            [20, 20, 5]
        );
    }

    #[test]
    fn classifies_message_unavailable_errors() {
        assert!(is_message_unavailable_error("sync.message_not_found"));
        assert!(is_message_unavailable_error("sync.message_body_missing"));
        assert!(!is_message_unavailable_error(
            "sync.message_body_fetch_failed"
        ));
        assert!(!is_message_unavailable_error("sync.uid_validity_changed"));
        assert!(!is_message_unavailable_error(""));
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
        let locations = pending_body_locations(
            vec![
                StoredMessageLocation {
                    message_id: "nine".to_owned(),
                    uid: 9,
                    uid_validity: 2,
                },
                StoredMessageLocation {
                    message_id: "three".to_owned(),
                    uid: 3,
                    uid_validity: 2,
                },
                StoredMessageLocation {
                    message_id: "one".to_owned(),
                    uid: 1,
                    uid_validity: 1,
                },
            ],
            2,
        );
        assert_eq!(
            locations
                .into_iter()
                .map(|location| location.uid)
                .collect::<Vec<_>>(),
            vec![3, 9]
        );
    }

    #[test]
    fn parses_and_sanitizes_html_message() {
        let raw = b"From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: Hello\r\nMessage-ID: <1@example.com>\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<!doctype html><html><head><title>Hidden title</title></head><body><p onclick=\"bad()\">Hello<script>bad()</script></p></body></html>";
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
        assert_eq!(message.plain_text.as_deref(), Some("Hello\n"));
        assert_eq!(message.preview, "Hello\n");
        let safe_html = message.safe_html.unwrap();
        assert!(!safe_html.contains("<script"));
        assert!(!safe_html.contains("Hidden title"));
    }

    #[test]
    fn collects_every_observable_address_header_for_contacts() {
        let raw = concat!(
            "From: From Person <from@example.com>\r\n",
            "Sender: sender@example.com\r\n",
            "Reply-To: reply@example.com\r\n",
            "To: to@example.com\r\n",
            "Cc: cc@example.com\r\n",
            "Bcc: bcc@example.com\r\n",
            "Subject: Contact headers\r\n\r\n",
            "Body"
        );
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            std::iter::empty(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();
        let values = message
            .contact_addresses
            .into_iter()
            .map(|value| (value.role, value.address.email))
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                (ContactAddressRole::From, "from@example.com".to_owned()),
                (ContactAddressRole::Sender, "sender@example.com".to_owned()),
                (ContactAddressRole::ReplyTo, "reply@example.com".to_owned()),
                (ContactAddressRole::To, "to@example.com".to_owned()),
                (ContactAddressRole::Cc, "cc@example.com".to_owned()),
                (ContactAddressRole::Bcc, "bcc@example.com".to_owned()),
            ]
        );
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
    fn decodes_rfc2047_attachment_names_split_across_continuation_parameters() {
        let raw = include_bytes!(
            "../../../testdata/mail-rendering/segmented-rfc2047-attachment-name.eml"
        );
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

        assert_eq!(message.attachments.len(), 1);
        assert_eq!(
            message.attachments[0].file_name,
            "黄龙机房搬迁割接第三期1.xlsx"
        );
    }

    #[test]
    fn decodes_split_rfc2047_name_from_content_type_when_filename_is_absent() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Split content type name\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n\r\n",
            "Body\r\n",
            "--nextmail\r\n",
            "Content-Type: application/octet-stream;\r\n",
            " name*0=\"=?UTF-8?B?6buE6b6Z5py65oi/5pCs6L+B5Ymy5o6l56ys5LiJ5pyfMS54bH\";\r\n",
            " name*1=\"N4?=\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "Content-Disposition: attachment\r\n\r\n",
            "YXR0YWNobWVudA==\r\n",
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
        assert_eq!(
            message.attachments[0].file_name,
            "黄龙机房搬迁割接第三期1.xlsx"
        );
    }

    #[test]
    fn preserves_standard_percent_encoded_rfc2231_continuations() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: RFC 2231 attachment name\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=nextmail\r\n\r\n",
            "--nextmail\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n\r\n",
            "Body\r\n",
            "--nextmail\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "Content-Disposition: attachment;\r\n",
            " filename*0*=UTF-8''%E9%BB%84%E9%BE%99%E6%9C%BA%E6%88%BF;\r\n",
            " filename*1*=%E6%90%AC%E8%BF%81.xlsx\r\n\r\n",
            "YXR0YWNobWVudA==\r\n",
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
        assert_eq!(message.attachments[0].file_name, "黄龙机房搬迁.xlsx");
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

    mod worker_tests {
        use super::*;
        use crate::core::{
            ConnectionSecurity, MessageUpsertOutcome, RemoteMessageBody, SyncObserver,
        };
        use async_trait::async_trait;
        use std::sync::Mutex as StdMutex;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

        const TEST_HEADER: &str =
            "From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: test\r\nMessage-ID: <m@example.com>\r\n";

        struct RecordingSink {
            upserts: StdMutex<Vec<(u32, usize)>>,
            bodies: StdMutex<Vec<String>>,
            pending: StdMutex<Vec<StoredMessageLocation>>,
            mailbox_events: StdMutex<Vec<String>>,
        }

        impl RecordingSink {
            fn new() -> Self {
                Self {
                    upserts: StdMutex::new(Vec::new()),
                    bodies: StdMutex::new(Vec::new()),
                    pending: StdMutex::new(Vec::new()),
                    mailbox_events: StdMutex::new(Vec::new()),
                }
            }

            fn mailbox_event_snapshot(&self) -> Vec<String> {
                self.mailbox_events.lock().unwrap().clone()
            }

            fn upsert_snapshot(&self) -> Vec<(u32, usize)> {
                self.upserts.lock().unwrap().clone()
            }

            fn body_snapshot(&self) -> Vec<String> {
                self.bodies.lock().unwrap().clone()
            }

            fn set_pending(&self, locations: Vec<StoredMessageLocation>) {
                *self.pending.lock().unwrap() = locations;
            }
        }

        #[async_trait]
        impl MailSyncSink for RecordingSink {
            async fn ensure_mailbox(
                &self,
                _account_slot_id: &str,
                mailbox: &RemoteMailbox,
            ) -> CommandResult<Option<StoredMailbox>> {
                self.mailbox_events
                    .lock()
                    .unwrap()
                    .push(format!("ensure:{}", mailbox.name));
                Ok(Some(StoredMailbox {
                    id: format!("mb-{}", mailbox.name),
                    last_uid: 0,
                    highest_modseq: None,
                    notification_baseline_required: true,
                }))
            }

            async fn upsert_mailbox(
                &self,
                _account_slot_id: &str,
                mailbox: &RemoteMailbox,
            ) -> CommandResult<StoredMailbox> {
                self.mailbox_events
                    .lock()
                    .unwrap()
                    .push(format!("upsert:{}", mailbox.name));
                Ok(StoredMailbox {
                    id: "mb".to_owned(),
                    last_uid: 0,
                    highest_modseq: None,
                    notification_baseline_required: true,
                })
            }

            async fn upsert_message(
                &self,
                _account_slot_id: &str,
                _mailbox_id: &str,
                message: &RemoteMessage,
            ) -> CommandResult<MessageUpsertOutcome> {
                self.upserts
                    .lock()
                    .unwrap()
                    .push((message.uid, message.attachments.len()));
                Ok(MessageUpsertOutcome {
                    message_id: format!("id-{}", message.uid),
                    is_new_location: true,
                    contacts_changed: false,
                })
            }

            async fn complete_notification_baseline(
                &self,
                _account_slot_id: &str,
            ) -> CommandResult<()> {
                Ok(())
            }

            async fn complete_mailbox(
                &self,
                _mailbox_id: &str,
                _last_uid: u32,
            ) -> CommandResult<()> {
                Ok(())
            }

            async fn stored_uids(
                &self,
                _mailbox_id: &str,
                _uid_validity: u32,
            ) -> CommandResult<Vec<u32>> {
                Ok(Vec::new())
            }

            async fn pending_body_locations(
                &self,
                _mailbox_id: &str,
                _received_after: Option<i64>,
            ) -> CommandResult<Vec<StoredMessageLocation>> {
                Ok(self.pending.lock().unwrap().clone())
            }

            async fn replace_message_body(
                &self,
                _account_slot_id: &str,
                message_id: &str,
                _body: &RemoteMessageBody,
            ) -> CommandResult<()> {
                self.bodies.lock().unwrap().push(message_id.to_owned());
                Ok(())
            }

            async fn reconcile_mailbox(
                &self,
                _mailbox_id: &str,
                _uid_validity: u32,
                _highest_modseq: Option<u64>,
                _states: &[RemoteMessageState],
            ) -> CommandResult<()> {
                Ok(())
            }
        }

        struct RecordingObserver {
            mailbox_changes: StdMutex<Vec<String>>,
        }

        impl RecordingObserver {
            fn new() -> Self {
                Self {
                    mailbox_changes: StdMutex::new(Vec::new()),
                }
            }

            fn mailbox_changes_snapshot(&self) -> Vec<String> {
                self.mailbox_changes.lock().unwrap().clone()
            }
        }

        impl SyncObserver for RecordingObserver {
            fn notify(&self, notice: SyncNotice) {
                if let SyncNotice::MailboxChanged { mailbox_id, .. } = notice {
                    self.mailbox_changes.lock().unwrap().push(mailbox_id);
                }
            }
        }

        struct WorkerHarness {
            mailbox: StoredMailbox,
        }

        impl WorkerHarness {
            fn context(&self) -> FolderSyncContext<'_> {
                FolderSyncContext {
                    uid_validity: 7,
                    mailbox: &self.mailbox,
                    mailbox_name: "Inbox",
                    default_notification_enabled: false,
                }
            }
        }

        fn test_account() -> ImapAccountConfig {
            ImapAccountConfig {
                account_id: "acc".to_owned(),
                account_slot_id: "slot".to_owned(),
                download_full_messages: false,
                host: "imap.example.com".to_owned(),
                port: 993,
                security: ConnectionSecurity::Tls,
                username: "user".to_owned(),
                password: "pass".to_owned(),
            }
        }

        fn header_response(uid: u32) -> String {
            format!(
                "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"15-Aug-2026 14:01:04 +0800\" RFC822.SIZE 100 BODY[HEADER] {{{}}}\r\n{TEST_HEADER})\r\n",
                TEST_HEADER.len()
            )
        }

        fn tagged_ok(tag: &str) -> String {
            format!("{tag} OK UID FETCH Completed\r\n")
        }

        fn attachment_bodystructure(uid: u32) -> String {
            format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"APPLICATION\" \"OCTET-STREAM\" (\"name\" \"mail.eml\" \"charset\" \"utf-8\") NIL NIL \"BASE64\" 2536 NIL (\"attachment\" (\"filename\" \"mail.eml\")) NIL))\r\n")
        }

        fn plain_bodystructure(uid: u32) -> String {
            format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"TEXT\" \"PLAIN\" (\"charset\" \"utf-8\") NIL NIL \"7BIT\" 5 1 NIL NIL NIL))\r\n")
        }

        // Delivery-status report shape observed from QQ Mail: the HTML part's
        // body-fld-enc is NIL, which the strict RFC 3501 grammar in imap-proto
        // rejects and poisons the whole response stream.
        fn qq_poison_bodystructure(uid: u32) -> String {
            format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE ((\"TEXT\" \"HTML\" (\"charset\" \"utf-8\") NIL NIL NIL 2715 34 NIL NIL NIL)(\"APPLICATION\" \"OCTET-STREAM\" (\"name\" \"mail.eml\" \"charset\" \"utf-8\") NIL NIL \"BASE64\" 2536 NIL (\"attachment\" (\"filename\" \"mail.eml\")) NIL) \"REPORT\" (\"BOUNDARY\" \"QQ_MAIL_RETURN\") NIL NIL))\r\n")
        }

        #[tokio::test]
        async fn bodystructure_parse_failure_commits_headers_and_degrades_gracefully() {
            let (client_stream, server) = tokio::io::duplex(1 << 16);
            let server_task = tokio::spawn(async move {
                let mut lines = BufReader::new(server);
                let mut line = String::new();
                // LOGIN
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                write_all(
                    lines.get_mut(),
                    format!("{tag} OK LOGIN completed\r\n").as_bytes(),
                )
                .await;
                // header batch
                line.clear();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                let mut out = String::new();
                for uid in 1..=3u32 {
                    out.push_str(&header_response(uid));
                }
                out.push_str(&tagged_ok(&tag));
                write_all(lines.get_mut(), out.as_bytes()).await;
                // bodystructure batch, UID 2 poisoned with the QQ shape
                line.clear();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                let mut out = String::new();
                out.push_str(&plain_bodystructure(1));
                out.push_str(&qq_poison_bodystructure(2));
                out.push_str(&plain_bodystructure(3));
                out.push_str(&tagged_ok(&tag));
                write_all(lines.get_mut(), out.as_bytes()).await;
            });

            let sink = RecordingSink::new();
            let observer = RecordingObserver::new();
            let harness = WorkerHarness {
                mailbox: StoredMailbox {
                    id: "mb".to_owned(),
                    last_uid: 0,
                    highest_modseq: None,
                    notification_baseline_required: true,
                },
            };
            let account = test_account();
            let context = harness.context();
            let completed = AtomicU64::new(0);
            let write_lock = Mutex::new(());
            let mut session = async_imap::Client::new(client_stream)
                .login("user", "pass")
                .await
                .unwrap();
            let result = fetch_summaries_worker(
                &mut session,
                &[1, 2, 3],
                &account,
                &sink,
                &observer,
                &context,
                false,
                &completed,
                3,
                &write_lock,
            )
            .await;

            server_task.await.unwrap();
            let (highest_uid, session_usable) = result.unwrap();
            assert_eq!(highest_uid, 3);
            assert!(!session_usable);
            let upserts = sink.upsert_snapshot();
            assert_eq!(upserts, vec![(1, 0), (2, 0), (3, 0)]);
        }

        #[tokio::test]
        async fn bodystructure_attachments_merge_into_committed_headers() {
            let (client_stream, server) = tokio::io::duplex(1 << 16);
            let server_task = tokio::spawn(async move {
                let mut lines = BufReader::new(server);
                let mut line = String::new();
                // LOGIN
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                write_all(
                    lines.get_mut(),
                    format!("{tag} OK LOGIN completed\r\n").as_bytes(),
                )
                .await;
                // header batch
                line.clear();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                let mut out = String::new();
                for uid in 1..=3u32 {
                    out.push_str(&header_response(uid));
                }
                out.push_str(&tagged_ok(&tag));
                write_all(lines.get_mut(), out.as_bytes()).await;
                // bodystructure batch, UID 2 carries one attachment
                line.clear();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                let mut out = String::new();
                out.push_str(&plain_bodystructure(1));
                out.push_str(&attachment_bodystructure(2));
                out.push_str(&plain_bodystructure(3));
                out.push_str(&tagged_ok(&tag));
                write_all(lines.get_mut(), out.as_bytes()).await;
            });

            let sink = RecordingSink::new();
            let observer = RecordingObserver::new();
            let harness = WorkerHarness {
                mailbox: StoredMailbox {
                    id: "mb".to_owned(),
                    last_uid: 0,
                    highest_modseq: None,
                    notification_baseline_required: true,
                },
            };
            let account = test_account();
            let context = harness.context();
            let completed = AtomicU64::new(0);
            let write_lock = Mutex::new(());
            let mut session = async_imap::Client::new(client_stream)
                .login("user", "pass")
                .await
                .unwrap();
            let result = fetch_summaries_worker(
                &mut session,
                &[1, 2, 3],
                &account,
                &sink,
                &observer,
                &context,
                false,
                &completed,
                3,
                &write_lock,
            )
            .await;

            server_task.await.unwrap();
            let (highest_uid, session_usable) = result.unwrap();
            assert_eq!(highest_uid, 3);
            assert!(session_usable);
            let upserts = sink.upsert_snapshot();
            assert_eq!(upserts.len(), 4);
            assert_eq!(&upserts[..3], &[(1, 0), (2, 0), (3, 0)]);
            assert_eq!(upserts[3], (2, 1));
        }

        async fn write_all(stream: &mut DuplexStream, bytes: &[u8]) {
            stream.write_all(bytes).await.unwrap();
        }

        #[tokio::test]
        async fn folder_tree_is_precreated_and_notified_before_message_sync() {
            let sink = RecordingSink::new();
            let observer = RecordingObserver::new();
            let account = test_account();
            let descriptors = vec![
                FolderDescriptor {
                    name: "INBOX".to_owned(),
                    display_name: "INBOX".to_owned(),
                    progress_name: "INBOX".to_owned(),
                    delimiter: None,
                    role: MailboxRole::Inbox,
                    selectable: true,
                },
                FolderDescriptor {
                    name: "[Gmail]".to_owned(),
                    display_name: "[Gmail]".to_owned(),
                    progress_name: "[Gmail]".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Other,
                    selectable: false,
                },
            ];
            precreate_folder_tree(&sink, &account.account_slot_id, &descriptors, &observer)
                .await
                .unwrap();
            assert_eq!(
                sink.mailbox_event_snapshot(),
                vec!["ensure:INBOX", "ensure:[Gmail]"]
            );
            assert_eq!(
                observer.mailbox_changes_snapshot(),
                vec!["mb-INBOX", "mb-[Gmail]"]
            );
        }

        const FULL_MESSAGE: &str = "From: a@example.com\r\nTo: b@example.com\r\nSubject: full\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nhello body\r\n";

        fn no_structure_response(tag: &str, uid: u32) -> String {
            format!("* {uid} FETCH (UID {uid})\r\n{tag} OK UID FETCH Completed\r\n")
        }

        fn full_message_response(tag: &str, uid: u32) -> String {
            format!(
                "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"15-Aug-2026 14:01:04 +0800\" RFC822.SIZE {} BODY[] {{{}}}\r\n{FULL_MESSAGE})\r\n{tag} OK UID FETCH Completed\r\n",
                FULL_MESSAGE.len(),
                FULL_MESSAGE.len(),
            )
        }

        #[tokio::test]
        async fn prefetch_requeues_tail_when_bodystructure_kills_a_session() {
            // Session A serves UID 1 with a poison BODYSTRUCTURE (kills the
            // session), session B serves everyone else. UIDs 2-3 must be
            // re-dispatched to session B in the same run; UID 1 stays pending.
            let (client_a, server_a) = tokio::io::duplex(1 << 16);
            let (client_b, server_b) = tokio::io::duplex(1 << 16);
            let server_task_a = tokio::spawn(async move {
                let mut lines = BufReader::new(server_a);
                let mut line = String::new();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                write_all(
                    lines.get_mut(),
                    format!("{tag} OK LOGIN completed\r\n").as_bytes(),
                )
                .await;
                line.clear();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                let mut out = qq_poison_bodystructure(1);
                out.push_str(&tagged_ok(&tag));
                write_all(lines.get_mut(), out.as_bytes()).await;
                // The full-message fallback on the poisoned session is doomed;
                // read the command and then close the connection.
                line.clear();
                let _ = lines.read_line(&mut line).await;
            });
            let server_task_b = tokio::spawn(async move {
                let mut lines = BufReader::new(server_b);
                let mut line = String::new();
                lines.read_line(&mut line).await.unwrap();
                let tag = line.split_whitespace().next().unwrap().to_owned();
                write_all(
                    lines.get_mut(),
                    format!("{tag} OK LOGIN completed\r\n").as_bytes(),
                )
                .await;
                loop {
                    line.clear();
                    let read = lines.read_line(&mut line).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    let tokens = line.split_whitespace().collect::<Vec<_>>();
                    let (tag, uid) = (tokens[0].to_owned(), tokens[3].to_owned());
                    let response = if line.contains("BODYSTRUCTURE") {
                        no_structure_response(&tag, uid.parse().unwrap())
                    } else {
                        full_message_response(&tag, uid.parse().unwrap())
                    };
                    write_all(lines.get_mut(), response.as_bytes()).await;
                }
            });

            let sink = RecordingSink::new();
            sink.set_pending(
                (1..=5u32)
                    .map(|uid| StoredMessageLocation {
                        message_id: format!("m{uid}"),
                        uid,
                        uid_validity: 7,
                    })
                    .collect(),
            );
            let observer = RecordingObserver::new();
            let harness = WorkerHarness {
                mailbox: StoredMailbox {
                    id: "mb".to_owned(),
                    last_uid: 0,
                    highest_modseq: None,
                    notification_baseline_required: true,
                },
            };
            let account = test_account();
            let context = harness.context();
            let write_lock = Mutex::new(());
            let remote_uids = (1..=5u32).collect::<HashSet<_>>();
            let mut session_a = async_imap::Client::new(client_a)
                .login("user", "pass")
                .await
                .unwrap();
            let mut session_b = async_imap::Client::new(client_b)
                .login("user", "pass")
                .await
                .unwrap();
            let mut sessions = vec![&mut session_a, &mut session_b];
            fetch_missing_bodies(
                &mut sessions,
                &account,
                &sink,
                &observer,
                &context,
                &write_lock,
                &remote_uids,
            )
            .await
            .unwrap();

            server_task_a.await.unwrap();
            drop(sessions);
            drop(session_a);
            drop(session_b);
            server_task_b.await.unwrap();
            let mut upserts = sink.upsert_snapshot();
            upserts.sort_unstable();
            assert_eq!(upserts, vec![(2, 0), (3, 0), (4, 0), (5, 0)]);
            assert!(sink.body_snapshot().is_empty());
        }
    }
}

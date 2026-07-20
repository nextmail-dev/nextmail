use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use async_imap::{extensions::idle::IdleResponse, Session};
use futures_util::TryStreamExt;
use mail_parser::MessageParser;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::core::{
    CommandError, CommandResult, InboxWatchOutcome, RemoteMessage, RemoteOperation,
    RemoteOperationKind, RemoteOperationOutcome,
};

use super::{
    format_uid_set,
    parse::{message_flag_state, parse_message_in_background, MessageParseInput},
};

pub(super) async fn apply_operation_session<T>(
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

pub(super) fn conditional_store_query(
    base_modseq: Option<u64>,
    action: &str,
    flag: &str,
) -> String {
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

pub(super) async fn append_message_session<T>(
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

pub(super) async fn replace_draft_session<T>(
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

pub(super) async fn wait_for_change_session<T>(
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

pub(super) async fn fetch_message_session<T>(
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
    fetch_remote_messages(session, &[uid], uid_validity)
        .await?
        .pop()
        .ok_or_else(|| CommandError::new("sync.message_not_found"))
}

pub(super) async fn fetch_remote_messages<T>(
    session: &mut Session<T>,
    uids: &[u32],
    uid_validity: u32,
) -> CommandResult<Vec<RemoteMessage>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    let requested = uids.iter().copied().collect::<HashSet<_>>();
    let messages = session
        .uid_fetch(
            format_uid_set(uids),
            "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])",
        )
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::retryable("sync.message_body_fetch_failed"))?;
    let mut parsed_by_uid = HashMap::with_capacity(messages.len());
    for fetched in messages {
        let Some(uid) = fetched.uid.filter(|uid| requested.contains(uid)) else {
            continue;
        };
        let raw = fetched
            .body()
            .map(ToOwned::to_owned)
            .ok_or_else(|| CommandError::new("sync.message_body_missing"))?;
        let received_at = fetched
            .internal_date()
            .map(|value| value.timestamp())
            .unwrap_or_default();
        let (unread, flagged) = message_flag_state(fetched.flags());
        let size = fetched.size.unwrap_or(raw.len() as u32) as u64;
        let message = parse_message_in_background(MessageParseInput {
            uid,
            uid_validity,
            size,
            received_at,
            unread,
            flagged,
            header: Vec::new(),
            raw: Some(raw),
        })
        .await?;
        parsed_by_uid.insert(uid, message);
    }
    uids.iter()
        .map(|uid| {
            parsed_by_uid
                .remove(uid)
                .ok_or_else(|| CommandError::new("sync.message_not_found"))
        })
        .collect()
}

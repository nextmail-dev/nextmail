use std::collections::{HashMap, HashSet};

use async_imap::{
    imap_proto::types::{MessageSection, SectionPath},
    Session,
};
use futures_util::TryStreamExt;
use mail_parser::MessageParser;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::core::{
    CommandError, CommandResult, RemoteMailboxOperation, RemoteMailboxOperationOutcome,
    RemoteMessage, RemoteMessageBody, RemoteOperation, RemoteOperationKind, RemoteOperationOutcome,
};
use crate::protocols::{sanitize_mail_html_with_inline_images, ReaderInlineImage};

use super::{
    encoding::encode_modified_utf7,
    format_uid_set,
    parse::{message_flag_state, parse_message_in_background, MessageParseInput},
    structure::{
        analyze_bodystructure, canonical_section_path, parse_binary_section, parse_text_section,
        MessageSectionDescriptor, MessageStructure, TextSectionKind,
    },
    SELECTIVE_FETCH_UNSUPPORTED,
};

const MAX_INLINE_SECTION_BYTES: u64 = (25 * 1024 * 1024 * 4 / 3) + 4;
const MAX_TOTAL_INLINE_SECTION_BYTES: u64 = (100 * 1024 * 1024 * 4 / 3) + 4;

pub(super) async fn apply_mailbox_operation_session<T>(
    mut session: Session<T>,
    operation: &RemoteMailboxOperation,
) -> CommandResult<RemoteMailboxOperationOutcome>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut outcome = RemoteMailboxOperationOutcome::default();
    match operation {
        RemoteMailboxOperation::Create {
            parent_mailbox,
            delimiter,
            leaf_name,
        } => {
            let mailbox_name =
                join_mailbox_path(parent_mailbox.as_deref(), delimiter.as_deref(), leaf_name)?;
            session
                .create(&mailbox_name)
                .await
                .map_err(map_operation_err("mailbox.create_failed"))?;
            outcome.mailbox_name = Some(mailbox_name);
        }
        RemoteMailboxOperation::Rename {
            source_mailbox,
            destination_parent,
            delimiter,
            leaf_name,
        } => {
            let mailbox_name = join_mailbox_path(
                destination_parent.as_deref(),
                delimiter.as_deref(),
                leaf_name,
            )?;
            session
                .rename(source_mailbox, &mailbox_name)
                .await
                .map_err(map_operation_err("mailbox.rename_failed"))?;
            outcome.mailbox_name = Some(mailbox_name);
        }
        RemoteMailboxOperation::Delete { mailbox_name } => {
            session
                .delete(mailbox_name)
                .await
                .map_err(map_operation_err("mailbox.delete_failed"))?;
        }
        RemoteMailboxOperation::MarkAllRead { mailbox_name } => {
            session
                .select(mailbox_name)
                .await
                .map_err(map_operation_err("operation.mailbox_open_failed"))?;
            session
                .uid_store("1:*", "+FLAGS.SILENT (\\Seen)")
                .await
                .map_err(map_operation_err("mailbox.mark_all_read_failed"))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_operation_err("mailbox.mark_all_read_failed"))?;
        }
    }
    let _ = session.logout().await;
    Ok(outcome)
}

fn join_mailbox_path(
    parent_mailbox: Option<&str>,
    delimiter: Option<&str>,
    leaf_name: &str,
) -> CommandResult<String> {
    let encoded_leaf = encode_modified_utf7(leaf_name);
    match parent_mailbox {
        Some(parent) => {
            let delimiter = delimiter
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CommandError::new("mailbox.hierarchy_unsupported"))?;
            Ok(format!("{parent}{delimiter}{encoded_leaf}"))
        }
        None => Ok(encoded_leaf),
    }
}

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
        .map_err(map_operation_err("operation.capability_failed"))?;
    let selected = if capabilities.has_str("CONDSTORE") {
        session.select_condstore(&operation.source_mailbox).await
    } else {
        session.select(&operation.source_mailbox).await
    }
    .map_err(map_operation_err("operation.mailbox_open_failed"))?;
    if selected.uid_validity.unwrap_or_default() != operation.uid_validity {
        return Err(CommandError::new("sync.uid_validity_changed"));
    }
    let uid = operation.uid.to_string();
    let source_contains_uid = session
        .uid_search(format!("UID {uid}"))
        .await
        .map_err(map_operation_err("operation.message_check_failed"))?
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
            store_flag_delta(&mut session, &uid, action, "\\Seen").await?;
        }
        RemoteOperationKind::SetFlagged(value) => {
            let action = if value { "+FLAGS" } else { "-FLAGS" };
            store_flag_delta(&mut session, &uid, action, "\\Flagged").await?;
        }
        RemoteOperationKind::Copy => {
            let destination = operation
                .destination_mailbox
                .as_deref()
                .ok_or_else(|| CommandError::new("operation.destination_required"))?;
            session
                .uid_copy(&uid, destination)
                .await
                .map_err(map_operation_err("operation.copy_failed"))?;
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
                    .map_err(map_operation_err("operation.move_failed"))?;
            } else {
                session
                    .uid_copy(&uid, destination)
                    .await
                    .map_err(map_operation_err("operation.copy_failed"))?;
                mark_deleted(&mut session, &uid).await?;
                if capabilities.has_str("UIDPLUS") {
                    session
                        .uid_expunge(&uid)
                        .await
                        .map_err(map_operation_err("operation.expunge_failed"))?
                        .try_collect::<Vec<_>>()
                        .await
                        .map_err(map_operation_err("operation.expunge_failed"))?;
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
                    .map_err(map_operation_err("operation.expunge_failed"))?
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(map_operation_err("operation.expunge_failed"))?;
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
    action: &str,
    flag: &str,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    // .SILENT STORE for set_read/set_flagged. +FLAGS/-FLAGS only touch the named
    // flag (no lost-update risk for others), and we treat command success as
    // "applied" rather than inspecting the FETCH response. The previous non-
    // SILENT form checked `update.uid.is_some()`, but some servers don't echo a
    // FETCH for +FLAGS: the STORE succeeds (the message is marked read server-
    // side) yet the app saw an empty response and reported operation.store_failed,
    // retrying forever. Existence was already confirmed by the UID SEARCH above,
    // so command success is sufficient. (This also drops the CONDSTORE
    // UNCHANGEDSINCE conditional, which was both malformed in token order and a
    // silent no-op on conflict.)
    session
        .uid_store(uid, format!("{action}.SILENT ({flag})"))
        .await
        .map_err(map_operation_err("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_operation_err("operation.store_failed"))?;
    Ok(())
}

// Mirrors `map_imap_err`: preserves the underlying imap error in the log
// instead of discarding it via `.map_err(|_| ...)`. Operation failures used to
// surface only a generic code (e.g. "operation.store_failed") with no cause, so
// a rejected STORE read as a generic "unable to update status" with nothing in
// the log to diagnose it.
fn map_operation_err<E: std::fmt::Debug>(code: &'static str) -> impl FnOnce(E) -> CommandError {
    move |error| {
        tracing::warn!(%code, ?error, "imap operation failed");
        CommandError::retryable(code)
    }
}

async fn mark_deleted<T>(session: &mut Session<T>, uid: &str) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    session
        .uid_store(uid, "+FLAGS.SILENT (\\Deleted)")
        .await
        .map_err(map_operation_err("operation.store_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_operation_err("operation.store_failed"))?;
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
            .map_err(map_operation_err("operation.mailbox_open_failed"))?;
        let existing = session
            .uid_search(format!("HEADER Message-ID \"{message_id}\""))
            .await
            .map_err(map_operation_err("operation.sent_search_failed"))?;
        if !existing.is_empty() {
            let _ = session.logout().await;
            return Ok(());
        }
    }
    session
        .append(mailbox_name, Some(flags), None, raw)
        .await
        .map_err(map_operation_err("operation.append_failed"))?;
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
        .map_err(map_operation_err("operation.capability_failed"))?;
    session
        .select(mailbox_name)
        .await
        .map_err(map_operation_err("operation.mailbox_open_failed"))?;
    let mut old_uids = session
        .uid_search(format!("HEADER X-NextMail-Draft-ID \"{draft_id}\""))
        .await
        .map_err(map_operation_err("operation.draft_search_failed"))?
        .into_iter()
        .collect::<Vec<_>>();
    old_uids.sort_unstable();
    session
        .append(mailbox_name, Some("(\\Draft)"), None, raw)
        .await
        .map_err(map_operation_err("operation.append_failed"))?;
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
                .map_err(map_operation_err("operation.expunge_failed"))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(map_operation_err("operation.expunge_failed"))?;
        } else {
            cleanup_pending = true;
        }
    }
    let _ = session.logout().await;
    Ok(RemoteOperationOutcome { cleanup_pending })
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
        .map_err(map_operation_err("sync.mailbox_open_failed"))?;
    let uid_validity = selected.uid_validity.unwrap_or_default();
    if uid_validity == 0 || uid_validity != expected_uid_validity {
        return Err(CommandError::new("sync.uid_validity_changed"));
    }
    let message = fetch_remote_message(&mut session, uid, uid_validity).await?;
    let _ = session.logout().await;
    Ok(message)
}

pub(super) async fn fetch_message_body_session<T>(
    mut session: Session<T>,
    mailbox_name: &str,
    uid: u32,
    expected_uid_validity: u32,
) -> CommandResult<RemoteMessageBody>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    validate_uid_validity(&mut session, mailbox_name, expected_uid_validity).await?;
    let body = fetch_remote_message_body(&mut session, uid).await?;
    let _ = session.logout().await;
    Ok(body)
}

pub(super) async fn fetch_attachment_session<T>(
    mut session: Session<T>,
    mailbox_name: &str,
    uid: u32,
    expected_uid_validity: u32,
    imap_section: &str,
) -> CommandResult<Vec<u8>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    validate_uid_validity(&mut session, mailbox_name, expected_uid_validity).await?;
    let content = fetch_remote_attachment(&mut session, uid, imap_section).await?;
    let _ = session.logout().await;
    Ok(content)
}

async fn validate_uid_validity<T>(
    session: &mut Session<T>,
    mailbox_name: &str,
    expected_uid_validity: u32,
) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let selected = session
        .examine(mailbox_name)
        .await
        .map_err(map_operation_err("sync.mailbox_open_failed"))?;
    let uid_validity = selected.uid_validity.unwrap_or_default();
    if uid_validity == 0 || uid_validity != expected_uid_validity {
        return Err(CommandError::new("sync.uid_validity_changed"));
    }
    Ok(())
}

pub(super) async fn fetch_remote_message_body<T>(
    session: &mut Session<T>,
    uid: u32,
) -> CommandResult<RemoteMessageBody>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let structure = fetch_message_structure(session, uid).await?;
    if structure.requires_full_fetch || structure.text_sections.is_empty() {
        return Err(CommandError::new(SELECTIVE_FETCH_UNSUPPORTED));
    }
    let fetched = fetch_section_pairs(
        session,
        uid,
        &structure.text_sections,
        "sync.message_body_fetch_failed",
    )
    .await?;
    let mut plain_text = None;
    let mut html = None;
    let mut preview = None;
    for descriptor in &structure.text_sections {
        let (mime, body) = fetched
            .get(&descriptor.section)
            .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
        let kind = descriptor
            .text_kind
            .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
        let parsed = parse_text_section(kind, mime, body)
            .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
        if preview.as_deref().is_none_or(str::is_empty) && !parsed.preview.is_empty() {
            preview = Some(parsed.preview);
        }
        match kind {
            TextSectionKind::Plain => plain_text = Some(parsed.text),
            TextSectionKind::Html => html = Some(parsed.text),
        }
    }

    let mut inline_images = Vec::new();
    if let Some(html) = html.as_deref() {
        let referenced = super::super::html::referenced_content_ids(html);
        let mut total = 0u64;
        let mut selected = Vec::new();
        for part in &structure.inline_sections {
            if part
                .content_id
                .as_ref()
                .is_some_and(|content_id| referenced.contains(content_id))
                && part.encoded_size <= MAX_INLINE_SECTION_BYTES
                && total
                    .checked_add(part.encoded_size)
                    .is_some_and(|next| next <= MAX_TOTAL_INLINE_SECTION_BYTES)
            {
                total += part.encoded_size;
                selected.push(part.clone());
            }
        }
        let fetched_inline =
            fetch_section_pairs(session, uid, &selected, "sync.message_body_fetch_failed").await?;
        for descriptor in selected {
            let (mime, body) = fetched_inline
                .get(&descriptor.section)
                .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
            let bytes = parse_binary_section(mime, body)
                .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
            inline_images.push(ReaderInlineImage {
                content_id: descriptor.content_id.unwrap_or_default(),
                content_type: descriptor.content_type,
                bytes,
            });
        }
    }

    let sanitized = html
        .as_deref()
        .map(|value| sanitize_mail_html_with_inline_images(value, &inline_images));
    Ok(RemoteMessageBody {
        plain_text,
        safe_html: sanitized.as_ref().map(|value| value.document.clone()),
        preview,
        attachments: structure.attachments,
        remote_images_blocked: sanitized
            .as_ref()
            .is_some_and(|value| value.remote_images_blocked),
        inline_content_ids: sanitized
            .map(|value| value.inline_content_ids.into_iter().collect())
            .unwrap_or_default(),
    })
}

pub(super) async fn fetch_remote_attachment<T>(
    session: &mut Session<T>,
    uid: u32,
    imap_section: &str,
) -> CommandResult<Vec<u8>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    canonical_section_path(imap_section)
        .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
    let descriptor = MessageSectionDescriptor {
        section: imap_section.to_owned(),
        content_type: String::new(),
        content_id: None,
        encoded_size: 0,
        text_kind: None,
    };
    let fetched = fetch_section_pairs(
        session,
        uid,
        std::slice::from_ref(&descriptor),
        "attachment.download_failed",
    )
    .await?;
    let (mime, body) = fetched
        .get(imap_section)
        .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
    parse_binary_section(mime, body).ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))
}

async fn fetch_message_structure<T>(
    session: &mut Session<T>,
    uid: u32,
) -> CommandResult<MessageStructure>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    // Any failure here routes callers to the full-message fallback instead of
    // failing the request: a BODYSTRUCTURE response the parser rejects (e.g.
    // QQ Mail sending NIL for body-fld-enc) also poisons the connection, so
    // retrying on this session is pointless and the caller's fallback path
    // reconnects. The error is not logged in full because stream parse errors
    // embed the raw server response.
    let responses = session
        .uid_fetch(uid.to_string(), "(UID BODYSTRUCTURE)")
        .await
        .map_err(|_| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|_| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
    let fetched = responses
        .iter()
        .find(|response| response.uid == Some(uid))
        .ok_or_else(|| CommandError::new("sync.message_not_found"))?;
    fetched
        .bodystructure()
        .map(analyze_bodystructure)
        .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))
}

async fn fetch_section_pairs<T>(
    session: &mut Session<T>,
    uid: u32,
    descriptors: &[MessageSectionDescriptor],
    failure_code: &'static str,
) -> CommandResult<HashMap<String, (Vec<u8>, Vec<u8>)>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    if descriptors.is_empty() {
        return Ok(HashMap::new());
    }
    let mut paths = Vec::with_capacity(descriptors.len());
    let mut items = Vec::with_capacity(descriptors.len() * 2 + 1);
    items.push("UID".to_owned());
    for descriptor in descriptors {
        let path = canonical_section_path(&descriptor.section)
            .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?;
        items.push(format!("BODY.PEEK[{}.MIME]", descriptor.section));
        items.push(format!("BODY.PEEK[{}]", descriptor.section));
        paths.push((descriptor.section.clone(), path));
    }
    let responses = session
        .uid_fetch(uid.to_string(), format!("({})", items.join(" ")))
        .await
        .map_err(map_operation_err(failure_code))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_operation_err(failure_code))?;
    let fetched = responses
        .iter()
        .find(|response| response.uid == Some(uid))
        .ok_or_else(|| CommandError::new("sync.message_not_found"))?;
    paths
        .into_iter()
        .map(|(section, path)| {
            let mime_path = SectionPath::Part(path.clone(), Some(MessageSection::Mime));
            let body_path = SectionPath::Part(path, None);
            let mime = fetched
                .section(&mime_path)
                .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?
                .to_vec();
            let body = fetched
                .section(&body_path)
                .ok_or_else(|| CommandError::new(SELECTIVE_FETCH_UNSUPPORTED))?
                .to_vec();
            Ok((section, (mime, body)))
        })
        .collect()
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
        .map_err(map_operation_err("sync.message_body_fetch_failed"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_operation_err("sync.message_body_fetch_failed"))?;
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

#[cfg(test)]
mod mailbox_operation_tests {
    use super::join_mailbox_path;

    #[test]
    fn builds_modified_utf7_mailbox_paths_without_exposing_encoding_to_runtime() {
        assert_eq!(
            join_mailbox_path(Some("Projects"), Some("/"), "日本語").unwrap(),
            "Projects/&ZeVnLIqe-"
        );
        assert_eq!(join_mailbox_path(None, Some("/"), "A&B").unwrap(), "A&-B");
        assert_eq!(
            join_mailbox_path(Some("Projects"), None, "2026")
                .unwrap_err()
                .code,
            "mailbox.hierarchy_unsupported"
        );
    }
}

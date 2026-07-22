use async_imap::types::Flag;
use mail_parser::{Address, Message, MessageParser, MimeHeaders};

use crate::{
    core::{CommandError, CommandResult, MessageAddress, RemoteAttachment, RemoteMessage},
    protocols::sanitize_mail_html_with_cid_images,
};

pub(super) struct MessageParseInput {
    pub(super) uid: u32,
    pub(super) uid_validity: u32,
    pub(super) size: u64,
    pub(super) received_at: i64,
    pub(super) unread: bool,
    pub(super) flagged: bool,
    pub(super) header: Vec<u8>,
    pub(super) raw: Option<Vec<u8>>,
}

#[cfg(test)]
pub(super) fn parse_message<'a>(
    uid: u32,
    uid_validity: u32,
    size: u64,
    received_at: i64,
    flags: impl Iterator<Item = Flag<'a>>,
    header: &[u8],
    raw: Option<Vec<u8>>,
) -> CommandResult<RemoteMessage> {
    let (unread, flagged) = message_flag_state(flags);
    parse_message_with_state(MessageParseInput {
        uid,
        uid_validity,
        size,
        received_at,
        unread,
        flagged,
        header: header.to_vec(),
        raw,
    })
}

pub(super) fn message_flag_state<'a>(flags: impl Iterator<Item = Flag<'a>>) -> (bool, bool) {
    let flags = flags.collect::<Vec<_>>();
    (
        !flags.iter().any(|flag| matches!(flag, Flag::Seen)),
        flags.iter().any(|flag| matches!(flag, Flag::Flagged)),
    )
}

pub(super) async fn parse_message_in_background(
    input: MessageParseInput,
) -> CommandResult<RemoteMessage> {
    tokio::task::spawn_blocking(move || parse_message_with_state(input))
        .await
        .map_err(|_| CommandError::new("sync.message_parse_failed"))?
}

fn parse_message_with_state(input: MessageParseInput) -> CommandResult<RemoteMessage> {
    let parsed = input
        .raw
        .as_deref()
        .and_then(|value| MessageParser::default().parse(value));
    let parsed_headers = if parsed.is_none() {
        MessageParser::default().parse_headers(&input.header)
    } else {
        None
    };
    let message = parsed.as_ref().or(parsed_headers.as_ref());
    let plain_text = parsed
        .as_ref()
        .and_then(|message| message.body_text(0))
        .map(|value| value.into_owned());
    let sanitized = parsed.as_ref().and_then(|message| {
        message
            .body_html(0)
            .map(|value| sanitize_mail_html_with_cid_images(&value, message))
    });
    let attachments = parsed
        .as_ref()
        .map(|message| {
            attachment_summaries(
                message,
                sanitized
                    .as_ref()
                    .map(|value| &value.inline_content_ids)
                    .unwrap_or(&std::collections::HashSet::new()),
            )
        })
        .unwrap_or_default();
    let preview = parsed
        .as_ref()
        .and_then(|message| message.body_preview(180))
        .map(|value| value.into_owned())
        .unwrap_or_default();
    Ok(RemoteMessage {
        uid: input.uid,
        uid_validity: input.uid_validity,
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
            .unwrap_or(input.received_at),
        preview,
        unread: input.unread,
        flagged: input.flagged,
        size: input.size,
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
        raw: input.raw,
        attachments,
        remote_images_blocked: sanitized
            .as_ref()
            .is_some_and(|value| value.remote_images_blocked),
        modseq: None,
    })
}

fn attachment_summaries(
    message: &Message<'_>,
    inline_content_ids: &std::collections::HashSet<String>,
) -> Vec<RemoteAttachment> {
    message
        .attachments()
        .enumerate()
        .filter_map(|(index, attachment)| {
            let content_id = attachment
                .content_id()
                .map(|value| value.trim().trim_matches(['<', '>']).to_owned());
            if content_id
                .as_ref()
                .is_some_and(|value| inline_content_ids.contains(&value.to_ascii_lowercase()))
            {
                return None;
            }
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
            Some(RemoteAttachment {
                part_index: index as u32,
                file_name: attachment
                    .attachment_name()
                    .unwrap_or("attachment")
                    .to_owned(),
                content_type,
                size: attachment.len() as u64,
                content_id,
            })
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

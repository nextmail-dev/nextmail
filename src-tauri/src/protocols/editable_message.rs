use mail_parser::{Address, MessageParser, MimeHeaders};

use super::{
    attachment_file_name, html::inline_raster_content_type, message_body_text,
    sanitize_rich_text_paste,
};
use crate::core::{
    CommandError, CommandResult, DraftRecipientFields, ImportedDraftAttachment,
    ImportedDraftSource, ImportedMessage, MessageAddress,
};

/// Both remote drafts and edit-as-new use the complete MIME, never the reader's
/// rewritten HTML or attachment projection (which intentionally omits CID images).
pub fn parse_editable_message(raw: &[u8]) -> CommandResult<ImportedMessage> {
    let message = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| CommandError::new("message.mime_parse_failed"))?;
    let safe_html = message
        .body_html(0)
        .map(|html| sanitize_rich_text_paste(&html));
    let normalized_html = safe_html
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut attachments = Vec::new();
    let mut total = 0usize;
    for part in message.attachments() {
        let bytes = part.contents();
        if bytes.len() > 25 * 1024 * 1024 {
            return Err(CommandError::new("attachment.too_large"));
        }
        total = total.saturating_add(bytes.len());
        if total > 100 * 1024 * 1024 {
            return Err(CommandError::new("attachment.total_too_large"));
        }
        let declared = part
            .content_type()
            .map(|value| {
                format!(
                    "{}/{}",
                    value.ctype(),
                    value.subtype().unwrap_or("octet-stream")
                )
            })
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        let cid = part
            .content_id()
            .map(|value| value.trim().trim_matches(['<', '>']).to_owned())
            .filter(|value| !value.is_empty());
        let inline_type = cid
            .as_ref()
            .filter(|cid| normalized_html.contains(&format!("cid:{}", cid.to_ascii_lowercase())))
            .and_then(|_| inline_raster_content_type(&declared, bytes));
        attachments.push(ImportedDraftAttachment {
            file_name: attachment_file_name(part, "attachment"),
            content_type: inline_type.unwrap_or(&declared).to_owned(),
            bytes: bytes.to_vec(),
            // Unverified/non-image CID parts remain ordinary downloadable attachments.
            content_id: inline_type.and(cid),
        });
    }
    Ok(ImportedMessage {
        source: ImportedDraftSource {
            recipients: DraftRecipientFields {
                to: addresses(message.to()),
                cc: addresses(message.cc()),
                bcc: addresses(message.bcc()),
            },
            subject: message.subject().unwrap_or_default().to_owned(),
            plain_text: message_body_text(&message).unwrap_or_default(),
            safe_html,
        },
        attachments,
    })
}

fn addresses(value: Option<&Address<'_>>) -> Vec<MessageAddress> {
    value
        .into_iter()
        .flat_map(|address| address.iter())
        .filter_map(|value| {
            Some(MessageAddress {
                name: value.name.as_deref().map(str::to_owned),
                email: value.address.as_deref()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;

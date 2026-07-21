use crate::core::{
    CommandError, CommandResult, DraftContent, DraftRecipientFields, MessageAddress,
};
use chrono::Local;
use mail_builder::{
    headers::{address::Address, raw::Raw},
    mime::MimePart,
    MessageBuilder,
};

#[derive(Clone, Debug)]
pub struct OutgoingAttachment {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub content_id: Option<String>,
}

pub fn build_outgoing_message(
    sender: &MessageAddress,
    recipients: &DraftRecipientFields,
    subject: &str,
    content: &DraftContent,
    attachments: Vec<OutgoingAttachment>,
) -> CommandResult<Vec<u8>> {
    let mut builder = MessageBuilder::new()
        .from(address(sender))
        .subject(subject.to_owned())
        .header("Date", Raw::new(Local::now().to_rfc2822()));

    if !recipients.to.is_empty() {
        builder = builder.to(address_list(&recipients.to));
    }
    if !recipients.cc.is_empty() {
        builder = builder.cc(address_list(&recipients.cc));
    }
    // Bcc is deliberately envelope-only and must not be serialized into the message headers.

    let mut inline_parts = Vec::new();
    let mut regular_parts = Vec::new();
    for attachment in attachments {
        if let Some(content_id) = attachment.content_id {
            inline_parts.push(
                MimePart::new(attachment.content_type, attachment.bytes)
                    .inline()
                    .cid(content_id),
            );
        } else {
            regular_parts.push(
                MimePart::new(attachment.content_type, attachment.bytes)
                    .attachment(attachment.file_name),
            );
        }
    }
    let html = MimePart::new("text/html", content.html.clone()).inline();
    let html = if inline_parts.is_empty() {
        html
    } else {
        let mut related = Vec::with_capacity(inline_parts.len() + 1);
        related.push(html);
        related.extend(inline_parts);
        MimePart::new("multipart/related", related)
    };
    let alternative = MimePart::new(
        "multipart/alternative",
        vec![
            MimePart::new("text/plain", content.plain_text.clone()).inline(),
            html,
        ],
    );
    let body = if regular_parts.is_empty() {
        alternative
    } else {
        let mut mixed = Vec::with_capacity(regular_parts.len() + 1);
        mixed.push(alternative);
        mixed.extend(regular_parts);
        MimePart::new("multipart/mixed", mixed)
    };
    builder = builder.body(body);

    let mut output = Vec::new();
    builder
        .write_to(&mut output)
        .map_err(|_| CommandError::new("send.mime_build_failed"))?;
    Ok(output)
}

fn address(value: &MessageAddress) -> Address<'static> {
    Address::new_address(value.name.clone(), value.email.clone())
}

fn address_list(values: &[MessageAddress]) -> Address<'static> {
    Address::new_list(values.iter().map(address).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_parser::{MessageParser, MimeHeaders};

    #[test]
    fn builds_unicode_multipart_message_without_bcc_header() {
        let raw = build_outgoing_message(
            &MessageAddress {
                name: Some("发件人".into()),
                email: "from@example.com".into(),
            },
            &DraftRecipientFields {
                to: vec![MessageAddress {
                    name: Some("收件人".into()),
                    email: "to@example.com".into(),
                }],
                cc: vec![],
                bcc: vec![MessageAddress {
                    name: None,
                    email: "hidden@example.com".into(),
                }],
            },
            "中文主题",
            &DraftContent {
                editor_json: "{}".into(),
                html: "<p>富文本正文</p>".into(),
                plain_text: "纯文本正文".into(),
            },
            vec![OutgoingAttachment {
                file_name: "报告.txt".into(),
                content_type: "text/plain".into(),
                bytes: b"attachment".to_vec(),
                content_id: None,
            }],
        )
        .unwrap();

        let parsed = MessageParser::default().parse(&raw).unwrap();
        assert_eq!(parsed.subject(), Some("中文主题"));
        assert_eq!(parsed.body_text(0).as_deref(), Some("纯文本正文"));
        assert!(parsed.body_html(0).unwrap().contains("富文本正文"));
        assert_eq!(parsed.attachments().count(), 1);
        assert_eq!(
            parsed.attachments().next().unwrap().attachment_name(),
            Some("报告.txt")
        );
        let raw_text = String::from_utf8_lossy(&raw).to_ascii_lowercase();
        assert!(!raw_text.contains("\r\nbcc:"));
        assert!(!raw_text.contains("hidden@example.com"));

        let date_header = String::from_utf8_lossy(&raw)
            .lines()
            .find(|line| line.starts_with("Date: "))
            .expect("generated MIME must contain a Date header")
            .to_owned();
        assert!(date_header.ends_with(&Local::now().format("%z").to_string()));
    }

    #[test]
    fn builds_related_html_with_a_cid_inline_image() {
        let raw = build_outgoing_message(
            &MessageAddress {
                name: None,
                email: "from@example.com".into(),
            },
            &DraftRecipientFields {
                to: vec![MessageAddress {
                    name: None,
                    email: "to@example.com".into(),
                }],
                cc: vec![],
                bcc: vec![],
            },
            "Inline",
            &DraftContent {
                editor_json: "{}".into(),
                html: "<p><img src=\"cid:logo@example.test\"></p>".into(),
                plain_text: "Logo".into(),
            },
            vec![OutgoingAttachment {
                file_name: "logo.png".into(),
                content_type: "image/png".into(),
                bytes: b"image".to_vec(),
                content_id: Some("logo@example.test".into()),
            }],
        )
        .unwrap();
        let parsed = MessageParser::default().parse(&raw).unwrap();
        let inline = parsed.attachments().next().expect("inline MIME part");
        assert_eq!(inline.content_id(), Some("logo@example.test"));
        assert_eq!(inline.contents(), b"image");
        let raw_text = String::from_utf8_lossy(&raw).to_ascii_lowercase();
        assert!(raw_text.contains("multipart/related"));
    }
}

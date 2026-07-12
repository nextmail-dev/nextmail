use mail_builder::{headers::address::Address, MessageBuilder};
use nextmail_core::{
    CommandError, CommandResult, DraftContent, DraftRecipientFields, MessageAddress,
};

#[derive(Clone, Debug)]
pub struct OutgoingAttachment {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
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
        .text_body(content.plain_text.clone())
        .html_body(content.html.clone());

    if !recipients.to.is_empty() {
        builder = builder.to(address_list(&recipients.to));
    }
    if !recipients.cc.is_empty() {
        builder = builder.cc(address_list(&recipients.cc));
    }
    // Bcc is deliberately envelope-only and must not be serialized into the message headers.

    for attachment in attachments {
        builder = builder.attachment(
            attachment.content_type,
            attachment.file_name,
            attachment.bytes,
        );
    }

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
    }
}

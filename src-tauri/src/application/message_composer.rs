use std::collections::HashSet;

use crate::core::{
    CommandError, CommandResult, ComposedMessageActionDraft, DraftContent, DraftRecipientFields,
    ImportedDraftSource, MessageActionSource, MessageAddress, MessageComposeAction,
};

pub struct MessageActionLabels<'a> {
    pub original_message: &'a str,
    pub wrote: &'a str,
    pub from: &'a str,
    pub to: &'a str,
    pub subject: &'a str,
}

pub fn compose_imported_draft(source: &ImportedDraftSource) -> CommandResult<DraftContent> {
    Ok(DraftContent {
        editor_json: editor_document_from_text(&source.plain_text)?,
        html: source
            .safe_html
            .clone()
            .unwrap_or_else(|| format!("<p>{}</p>", escape_html(&source.plain_text))),
        plain_text: source.plain_text.clone(),
    })
}

pub fn compose_message_action_draft(
    source: &MessageActionSource,
    own_email: &str,
    action: MessageComposeAction,
    labels: MessageActionLabels<'_>,
) -> CommandResult<ComposedMessageActionDraft> {
    let mut recipients = DraftRecipientFields::default();
    match action {
        MessageComposeAction::Reply => {
            recipients.to = reply_recipients(&source.from, &source.to, own_email);
        }
        MessageComposeAction::ReplyAll => {
            recipients.to = reply_recipients(&source.from, &source.to, own_email);
            recipients.cc = unique_addresses(
                source
                    .to
                    .iter()
                    .cloned()
                    .chain(source.cc.iter().cloned())
                    .collect(),
                own_email,
                &recipients.to,
            );
        }
        MessageComposeAction::Forward => {}
    }

    let sender = format_addresses(&source.from);
    let (original_header, original_header_html) = match action {
        MessageComposeAction::Reply | MessageComposeAction::ReplyAll => {
            let header = format!("{sender} {}", labels.wrote);
            let html = format!(
                "<p>{} {}</p>",
                escape_html(&sender),
                escape_html(labels.wrote)
            );
            (header, html)
        }
        MessageComposeAction::Forward => {
            let recipients = format_addresses(&source.to);
            let header = format!(
                "---------- {} ----------\n{}: {sender}\n{}: {recipients}\n{}: {}",
                labels.original_message, labels.from, labels.to, labels.subject, source.subject,
            );
            let html = format!(
                "<p>---------- {} ----------<br>{}: {}<br>{}: {}<br>{}: {}</p>",
                escape_html(labels.original_message),
                escape_html(labels.from),
                escape_html(&sender),
                escape_html(labels.to),
                escape_html(&recipients),
                escape_html(labels.subject),
                escape_html(&source.subject),
            );
            (header, html)
        }
    };
    let original_plain_text = format!("{original_header}\n\n{}", source.plain_text);
    let source_html = source
        .safe_html
        .clone()
        .unwrap_or_else(|| format!("<p>{}</p>", escape_html(&source.plain_text)));
    let original_html = format!("{original_header_html}{source_html}");
    let original_content = editor_content_from_text(&original_plain_text);
    let editor_json = serde_json::to_string(&serde_json::json!({
        "type": "doc",
        "content": [
            {
                "type": "nextmailReply",
                "content": [{ "type": "paragraph" }]
            },
            { "type": "paragraph" },
            {
                "type": "nextmailOriginalMessage",
                "attrs": {
                    "sourceHtml": original_html,
                    "sourcePlainText": original_plain_text
                },
                "content": original_content
            }
        ]
    }))
    .map_err(|_| CommandError::new("draft.editor_json_failed"))?;
    let html = format!(
        "<div data-nextmail-reply=\"\"><p></p></div><p></p><div data-nextmail-original-message=\"\">{original_html}</div>"
    );
    let plain_text = format!("\n\n{original_plain_text}");
    let mut references = source.references.clone();
    if let Some(value) = source.message_id.as_ref() {
        if !references.iter().any(|current| current == value) {
            references.push(value.clone());
        }
    }
    Ok(ComposedMessageActionDraft {
        recipients,
        subject: prefixed_subject(&source.subject, action),
        content: DraftContent {
            editor_json,
            html,
            plain_text,
        },
        in_reply_to: match action {
            MessageComposeAction::Forward => None,
            _ => source.message_id.clone(),
        },
        references,
    })
}

fn reply_recipients(
    from: &[MessageAddress],
    original_to: &[MessageAddress],
    own_email: &str,
) -> Vec<MessageAddress> {
    let preferred = unique_addresses(from.to_vec(), own_email, &[]);
    if preferred.is_empty() {
        unique_addresses(original_to.to_vec(), own_email, &[])
    } else {
        preferred
    }
}

fn unique_addresses(
    values: Vec<MessageAddress>,
    own_email: &str,
    excluded: &[MessageAddress],
) -> Vec<MessageAddress> {
    let own_email = own_email.trim().to_ascii_lowercase();
    let mut seen = excluded
        .iter()
        .map(|address| address.email.trim().to_ascii_lowercase())
        .collect::<HashSet<_>>();
    values
        .into_iter()
        .filter(|address| {
            let email = address.email.trim().to_ascii_lowercase();
            !email.is_empty() && email != own_email && seen.insert(email)
        })
        .collect()
}

fn format_addresses(values: &[MessageAddress]) -> String {
    values
        .iter()
        .map(|address| {
            address
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map_or_else(
                    || address.email.clone(),
                    |name| format!("{name} <{}>", address.email),
                )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn prefixed_subject(subject: &str, action: MessageComposeAction) -> String {
    let trimmed = subject.trim();
    match action {
        MessageComposeAction::Reply | MessageComposeAction::ReplyAll
            if trimmed.to_ascii_lowercase().starts_with("re:") =>
        {
            trimmed.to_owned()
        }
        MessageComposeAction::Forward
            if trimmed.to_ascii_lowercase().starts_with("fwd:")
                || trimmed.to_ascii_lowercase().starts_with("fw:") =>
        {
            trimmed.to_owned()
        }
        MessageComposeAction::Reply | MessageComposeAction::ReplyAll => format!("Re: {trimmed}"),
        MessageComposeAction::Forward => format!("Fwd: {trimmed}"),
    }
}

fn editor_document_from_text(value: &str) -> CommandResult<String> {
    serde_json::to_string(&serde_json::json!({
        "type": "doc",
        "content": editor_content_from_text(value),
    }))
    .map_err(|_| CommandError::new("draft.editor_json_failed"))
}

fn editor_content_from_text(value: &str) -> Vec<serde_json::Value> {
    value
        .split("\n\n")
        .map(|paragraph| {
            if paragraph.is_empty() {
                serde_json::json!({ "type": "paragraph" })
            } else {
                let mut lines = Vec::new();
                for (index, line) in paragraph.split('\n').enumerate() {
                    if index > 0 {
                        lines.push(serde_json::json!({ "type": "hardBreak" }));
                    }
                    if !line.is_empty() {
                        lines.push(serde_json::json!({ "type": "text", "text": line }));
                    }
                }
                serde_json::json!({ "type": "paragraph", "content": lines })
            }
        })
        .collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(email: &str) -> MessageAddress {
        MessageAddress {
            name: None,
            email: email.to_owned(),
        }
    }

    fn labels() -> MessageActionLabels<'static> {
        MessageActionLabels {
            original_message: "Forwarded message",
            wrote: "wrote:",
            from: "From",
            to: "To",
            subject: "Subject",
        }
    }

    #[test]
    fn reply_all_deduplicates_own_sender_and_cc_addresses() {
        let source = MessageActionSource {
            subject: "Topic".into(),
            from: vec![address("sender@example.com")],
            to: vec![address("me@example.com"), address("other@example.com")],
            cc: vec![address("SENDER@example.com"), address("other@example.com")],
            message_id: Some("child@example.com".into()),
            references: vec!["root@example.com".into()],
            plain_text: "First\nSecond".into(),
            safe_html: Some("<p><strong>First</strong><br>Second</p>".into()),
        };
        let draft = compose_message_action_draft(
            &source,
            "ME@example.com",
            MessageComposeAction::ReplyAll,
            labels(),
        )
        .unwrap();

        assert_eq!(draft.recipients.to, vec![address("sender@example.com")]);
        assert_eq!(draft.recipients.cc, vec![address("other@example.com")]);
        assert_eq!(draft.subject, "Re: Topic");
        assert_eq!(
            draft.references,
            vec!["root@example.com", "child@example.com"]
        );
        assert!(draft.content.editor_json.contains("nextmailReply"));
        assert!(draft
            .content
            .editor_json
            .contains("nextmailOriginalMessage"));
        assert!(draft.content.editor_json.contains("<strong>First</strong>"));
        assert!(!draft.content.plain_text.contains("> First"));
    }

    #[test]
    fn existing_prefixes_are_not_duplicated_and_forward_does_not_thread() {
        let source = MessageActionSource {
            subject: "FW: Existing".into(),
            from: vec![address("sender@example.com")],
            to: vec![address("me@example.com")],
            cc: vec![],
            message_id: Some("message@example.com".into()),
            references: vec![],
            plain_text: "<original>".into(),
            safe_html: None,
        };
        let draft = compose_message_action_draft(
            &source,
            "me@example.com",
            MessageComposeAction::Forward,
            labels(),
        )
        .unwrap();

        assert_eq!(draft.subject, "FW: Existing");
        assert_eq!(draft.in_reply_to, None);
        assert!(draft.content.html.contains("&lt;original&gt;"));
        assert!(draft
            .content
            .html
            .contains("data-nextmail-original-message"));
    }
}

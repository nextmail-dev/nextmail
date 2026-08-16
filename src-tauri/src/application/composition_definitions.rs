use crate::core::{
    CommandError, CommandResult, DraftContent, LanguagePreference, MailSignature,
    MailSignatureDraft, MailTemplate, MailTemplateDraft, MessageAddress, RenderedMailSignature,
    RenderedMailTemplate,
};
use chrono::{Datelike, Local, NaiveDate};
use lettre::Address;
use serde_json::Value;

const MAX_DEFINITION_NAME_CHARS: usize = 80;
const MAX_TEMPLATE_SUBJECT_CHARS: usize = 998;
const MAX_CONTENT_BYTES: usize = 5_000_000;

pub fn normalize_mail_template_draft(
    mut draft: MailTemplateDraft,
) -> CommandResult<MailTemplateDraft> {
    draft.name = normalize_name(draft.name, "template")?;
    draft.subject = draft.subject.trim().to_owned();
    if draft.subject.chars().count() > MAX_TEMPLATE_SUBJECT_CHARS {
        return Err(CommandError::new("template.subject_too_long"));
    }
    normalize_template_recipients(&mut draft.recipients)?;
    validate_content(&draft.content, "template")?;
    validate_variables(
        [
            draft.subject.as_str(),
            draft.content.html.as_str(),
            draft.content.plain_text.as_str(),
        ],
        "template",
    )?;
    validate_editor_variables(&draft.content.editor_json, "template")?;
    Ok(draft)
}

pub fn normalize_mail_signature_draft(
    mut draft: MailSignatureDraft,
) -> CommandResult<MailSignatureDraft> {
    draft.name = normalize_name(draft.name, "signature")?;
    validate_content(&draft.content, "signature")?;
    validate_variables(
        [
            draft.content.html.as_str(),
            draft.content.plain_text.as_str(),
        ],
        "signature",
    )?;
    validate_editor_variables(&draft.content.editor_json, "signature")?;
    Ok(draft)
}

pub struct CompositionRenderContext<'a> {
    pub sender: MessageAddress,
    pub recipient: Option<&'a MessageAddress>,
    pub language: LanguagePreference,
}

pub fn render_mail_template(
    template: &MailTemplate,
    context: &CompositionRenderContext<'_>,
) -> CommandResult<RenderedMailTemplate> {
    Ok(RenderedMailTemplate {
        id: template.id.clone(),
        subject: render_text(&template.subject, context, TextContext::Subject)?,
        recipients: template.recipients.clone(),
        content: render_content(&template.content, context)?,
    })
}

fn normalize_template_recipients(
    recipients: &mut crate::core::DraftRecipientFields,
) -> CommandResult<()> {
    for address in recipients
        .to
        .iter_mut()
        .chain(&mut recipients.cc)
        .chain(&mut recipients.bcc)
    {
        address.email = address.email.trim().to_owned();
        address.name = address
            .name
            .take()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        address
            .email
            .parse::<Address>()
            .map_err(|_| CommandError::new("template.recipient_invalid"))?;
    }
    Ok(())
}

pub fn render_mail_signature(
    signature: &MailSignature,
    context: &CompositionRenderContext<'_>,
) -> CommandResult<RenderedMailSignature> {
    Ok(RenderedMailSignature {
        id: signature.id.clone(),
        content: render_content(&signature.content, context)?,
    })
}

pub fn assemble_composition_content(
    base: &DraftContent,
    template: Option<&RenderedMailTemplate>,
    signature: Option<&RenderedMailSignature>,
) -> CommandResult<DraftContent> {
    if template.is_none() && signature.is_none() {
        return Ok(base.clone());
    }
    let base_document: Value = serde_json::from_str(&base.editor_json)
        .map_err(|_| CommandError::new("draft.editor_json_invalid"))?;
    if let Some((mut reply, original)) = message_action_nodes(&base_document) {
        if let Some(template) = template {
            reply["content"] = Value::Array(vec![definition_node(
                "nextmailTemplate",
                &template.id,
                &template.content.editor_json,
            )?]);
        }
        let mut content = vec![reply];
        if let Some(signature) = signature {
            content.push(serde_json::json!({ "type": "nextmailSignatureDivider" }));
            content.push(definition_node(
                "nextmailSignature",
                &signature.id,
                &signature.content.editor_json,
            )?);
            content.push(serde_json::json!({ "type": "paragraph" }));
        }
        content.push(original);
        let editor_json = serde_json::to_string(&serde_json::json!({
            "type": "doc",
            "content": content,
        }))
        .map_err(|_| CommandError::new("draft.editor_json_failed"))?;

        let reply_html = template.map_or_else(
            || "<p></p>".to_owned(),
            |value| {
                format!(
                    "<div data-nextmail-template-id=\"{}\">{}</div>",
                    value.id, value.content.html
                )
            },
        );
        let signature_html = signature.map_or_else(String::new, |value| {
            format!(
                "<hr data-nextmail-signature-divider=\"\" style=\"border:0;border-top:1px solid #d9dee7;margin:20px 0\"><div data-nextmail-signature-id=\"{}\">{}</div><p></p>",
                value.id, value.content.html
            )
        });
        let original_html = base
            .html
            .find("<div data-nextmail-original-message")
            .map(|index| &base.html[index..])
            .unwrap_or(base.html.as_str());
        let html = format!(
            "<div data-nextmail-reply=\"\">{reply_html}</div>{signature_html}{original_html}"
        );
        let reply_plain = template.map_or("", |value| value.content.plain_text.as_str());
        let original_plain = base.plain_text.trim_start_matches('\n');
        let plain_text = signature.map_or_else(
            || {
                [reply_plain, original_plain]
                    .into_iter()
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            },
            |value| {
                [
                    reply_plain,
                    "----------------",
                    value.content.plain_text.as_str(),
                    original_plain,
                ]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
            },
        );
        return Ok(DraftContent {
            editor_json,
            html,
            plain_text,
        });
    }
    let mut content = Vec::new();
    if let Some(template) = template {
        content.push(definition_node(
            "nextmailTemplate",
            &template.id,
            &template.content.editor_json,
        )?);
    }
    content.push(serde_json::json!({ "type": "paragraph" }));
    if let Some(signature) = signature {
        content.push(serde_json::json!({ "type": "nextmailSignatureDivider" }));
        content.push(definition_node(
            "nextmailSignature",
            &signature.id,
            &signature.content.editor_json,
        )?);
    }
    if !is_empty_document(&base_document) {
        if let Some(values) = base_document.get("content").and_then(Value::as_array) {
            content.extend(values.iter().cloned());
        }
    }
    let editor_json = serde_json::to_string(&serde_json::json!({
        "type": "doc",
        "content": content,
    }))
    .map_err(|_| CommandError::new("draft.editor_json_failed"))?;

    let mut html = String::new();
    if let Some(template) = template {
        html.push_str(&format!(
            "<div data-nextmail-template-id=\"{}\">{}</div>",
            template.id, template.content.html
        ));
    }
    html.push_str("<p></p>");
    if let Some(signature) = signature {
        html.push_str(
            "<hr data-nextmail-signature-divider=\"\" style=\"border:0;border-top:1px solid #d9dee7;margin:20px 0\">",
        );
        html.push_str(&format!(
            "<div data-nextmail-signature-id=\"{}\">{}</div>",
            signature.id, signature.content.html
        ));
    }
    if !is_empty_content(base) {
        html.push_str(&base.html);
    }

    let plain_text = [
        template.map(|value| value.content.plain_text.as_str()),
        signature.map(|_| "----------------"),
        signature.map(|value| value.content.plain_text.as_str()),
        (!is_empty_content(base)).then_some(base.plain_text.as_str()),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
    Ok(DraftContent {
        editor_json,
        html,
        plain_text,
    })
}

fn message_action_nodes(document: &Value) -> Option<(Value, Value)> {
    let content = document.get("content")?.as_array()?;
    let reply = content
        .iter()
        .find(|node| node.get("type").and_then(Value::as_str) == Some("nextmailReply"))?
        .clone();
    let original = content
        .iter()
        .find(|node| node.get("type").and_then(Value::as_str) == Some("nextmailOriginalMessage"))?
        .clone();
    Some((reply, original))
}

fn normalize_name(value: String, kind: &str) -> CommandResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(CommandError::new(format!("{kind}.name_required")));
    }
    if value.chars().count() > MAX_DEFINITION_NAME_CHARS {
        return Err(CommandError::new(format!("{kind}.name_too_long")));
    }
    Ok(value)
}

fn validate_content(content: &DraftContent, kind: &str) -> CommandResult<()> {
    if content.editor_json.len() > MAX_CONTENT_BYTES
        || content.html.len() > MAX_CONTENT_BYTES
        || content.plain_text.len() > MAX_CONTENT_BYTES
    {
        return Err(CommandError::new(format!("{kind}.content_too_large")));
    }
    serde_json::from_str::<serde_json::Value>(&content.editor_json)
        .map_err(|_| CommandError::new(format!("{kind}.editor_json_invalid")))?;
    Ok(())
}

const ALLOWED_VARIABLES: [&str; 5] = [
    "sender_name",
    "sender_email",
    "recipient_name",
    "recipient_email",
    "date",
];

fn validate_editor_variables(value: &str, kind: &str) -> CommandResult<()> {
    let document: Value = serde_json::from_str(value)
        .map_err(|_| CommandError::new(format!("{kind}.editor_json_invalid")))?;
    let mut texts = Vec::new();
    collect_text_values(&document, &mut texts);
    validate_variables(texts, kind)
}

fn collect_text_values<'a>(value: &'a Value, values: &mut Vec<&'a str>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get("text") {
                values.push(text);
            }
            for child in map.values() {
                collect_text_values(child, values);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_text_values(child, values);
            }
        }
        _ => {}
    }
}

fn validate_variables<'a>(
    values: impl IntoIterator<Item = &'a str>,
    kind: &str,
) -> CommandResult<()> {
    for value in values {
        for variable in variables_in(value) {
            if !ALLOWED_VARIABLES.contains(&variable.as_str()) {
                return Err(CommandError::new(format!("{kind}.variable_unknown"))
                    .with_param("variable", variable));
            }
        }
    }
    Ok(())
}

fn variables_in(value: &str) -> Vec<String> {
    let mut variables = Vec::new();
    let mut remainder = value;
    while let Some(start) = remainder.find("{{") {
        remainder = &remainder[start + 2..];
        let Some(end) = remainder.find("}}") else {
            break;
        };
        variables.push(remainder[..end].trim().to_owned());
        remainder = &remainder[end + 2..];
    }
    variables
}

#[derive(Clone, Copy)]
enum TextContext {
    Subject,
    Html,
    Plain,
}

fn render_content(
    content: &DraftContent,
    context: &CompositionRenderContext<'_>,
) -> CommandResult<DraftContent> {
    let mut document: Value = serde_json::from_str(&content.editor_json)
        .map_err(|_| CommandError::new("composition.editor_json_invalid"))?;
    render_editor_text(&mut document, context)?;
    Ok(DraftContent {
        editor_json: serde_json::to_string(&document)
            .map_err(|_| CommandError::new("composition.editor_json_failed"))?,
        html: render_text(&content.html, context, TextContext::Html)?,
        plain_text: render_text(&content.plain_text, context, TextContext::Plain)?,
    })
}

fn render_editor_text(
    value: &mut Value,
    context: &CompositionRenderContext<'_>,
) -> CommandResult<()> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get_mut("text") {
                *text = render_text(text, context, TextContext::Plain)?;
            }
            for child in map.values_mut() {
                render_editor_text(child, context)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                render_editor_text(child, context)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_text(
    value: &str,
    context: &CompositionRenderContext<'_>,
    text_context: TextContext,
) -> CommandResult<String> {
    let mut rendered = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some(start) = remainder.find("{{") {
        rendered.push_str(&remainder[..start]);
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find("}}") else {
            rendered.push_str(&remainder[start..]);
            return Ok(rendered);
        };
        let variable = after_start[..end].trim();
        let replacement = variable_value(variable, context)?;
        let replacement = match text_context {
            TextContext::Html => escape_html(&replacement),
            TextContext::Subject => replacement.replace(['\r', '\n'], " "),
            TextContext::Plain => replacement,
        };
        rendered.push_str(&replacement);
        remainder = &after_start[end + 2..];
    }
    rendered.push_str(remainder);
    Ok(rendered)
}

fn variable_value(variable: &str, context: &CompositionRenderContext<'_>) -> CommandResult<String> {
    let missing = || {
        CommandError::new("composition.variable_context_missing").with_param("variable", variable)
    };
    match variable {
        "sender_name" => context
            .sender
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(missing),
        "sender_email" => Ok(context.sender.email.clone()),
        "recipient_name" => context
            .recipient
            .and_then(|value| value.name.as_deref())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(missing),
        "recipient_email" => context
            .recipient
            .map(|value| value.email.clone())
            .ok_or_else(missing),
        "date" => Ok(format_local_date(
            Local::now().date_naive(),
            &context.language,
        )),
        _ => {
            Err(CommandError::new("composition.variable_unknown").with_param("variable", variable))
        }
    }
}

fn format_local_date(date: NaiveDate, language: &LanguagePreference) -> String {
    match language {
        LanguagePreference::ZhCn => format!("{}年{}月{}日", date.year(), date.month(), date.day()),
        LanguagePreference::EnUs => {
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!(
                "{} {}, {}",
                MONTHS[date.month0() as usize],
                date.day(),
                date.year()
            )
        }
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn definition_node(kind: &str, id: &str, document: &str) -> CommandResult<Value> {
    let value: Value = serde_json::from_str(document)
        .map_err(|_| CommandError::new("composition.editor_json_invalid"))?;
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![serde_json::json!({ "type": "paragraph" })]);
    Ok(serde_json::json!({
        "type": kind,
        "attrs": { "definitionId": id },
        "content": content,
    }))
}

fn is_empty_document(document: &Value) -> bool {
    document
        .get("content")
        .and_then(Value::as_array)
        .is_none_or(|content| {
            content.is_empty()
                || (content.len() == 1
                    && content[0].get("type").and_then(Value::as_str) == Some("paragraph")
                    && content[0].get("content").is_none())
        })
}

pub(crate) fn is_empty_content(content: &DraftContent) -> bool {
    content.plain_text.is_empty()
        && (content.html.is_empty() || content.html == "<p></p>")
        && serde_json::from_str(&content.editor_json).is_ok_and(|value| is_empty_document(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        CompositionDefinitionScope, DraftRecipientFields, MailSignature, MailTemplate,
        MessageAddress,
    };

    fn content() -> DraftContent {
        DraftContent {
            editor_json: r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.to_owned(),
            html: "<p></p>".to_owned(),
            plain_text: String::new(),
        }
    }

    #[test]
    fn normalizes_definition_names_and_template_subjects() {
        let value = normalize_mail_template_draft(MailTemplateDraft {
            name: "  Follow up  ".to_owned(),
            subject: "  Next steps  ".to_owned(),
            recipients: DraftRecipientFields {
                to: vec![MessageAddress {
                    name: Some("  Team  ".to_owned()),
                    email: "  team@example.com  ".to_owned(),
                }],
                cc: Vec::new(),
                bcc: Vec::new(),
            },
            content: content(),
        })
        .expect("valid template");

        assert_eq!(value.name, "Follow up");
        assert_eq!(value.subject, "Next steps");
        assert_eq!(value.recipients.to[0].name.as_deref(), Some("Team"));
        assert_eq!(value.recipients.to[0].email, "team@example.com");
    }

    #[test]
    fn rejects_empty_names_and_invalid_editor_json() {
        let empty = normalize_mail_signature_draft(MailSignatureDraft {
            name: "  ".to_owned(),
            content: content(),
        })
        .expect_err("empty name");
        assert_eq!(empty.code, "signature.name_required");

        let invalid = normalize_mail_template_draft(MailTemplateDraft {
            name: "Broken".to_owned(),
            subject: String::new(),
            recipients: DraftRecipientFields::default(),
            content: DraftContent {
                editor_json: "{".to_owned(),
                html: String::new(),
                plain_text: String::new(),
            },
        })
        .expect_err("invalid JSON");
        assert_eq!(invalid.code, "template.editor_json_invalid");

        let invalid_recipient = normalize_mail_template_draft(MailTemplateDraft {
            name: "Broken recipient".to_owned(),
            subject: String::new(),
            recipients: DraftRecipientFields {
                to: vec![MessageAddress {
                    name: None,
                    email: "not-an-email".to_owned(),
                }],
                cc: Vec::new(),
                bcc: Vec::new(),
            },
            content: content(),
        })
        .expect_err("invalid recipient");
        assert_eq!(invalid_recipient.code, "template.recipient_invalid");
    }

    #[test]
    fn rejects_unknown_variables_when_saving_definitions() {
        let invalid = normalize_mail_template_draft(MailTemplateDraft {
            name: "Broken".to_owned(),
            subject: "Hello {{account_password}}".to_owned(),
            recipients: DraftRecipientFields::default(),
            content: content(),
        })
        .expect_err("unknown variable");
        assert_eq!(invalid.code, "template.variable_unknown");
        assert_eq!(
            invalid.params.get("variable").map(String::as_str),
            Some("account_password")
        );
    }

    #[test]
    fn renders_variables_with_context_specific_html_escaping() {
        let template = MailTemplate {
            id: "template-one".to_owned(),
            scope: CompositionDefinitionScope::Global,
            account_id: None,
            name: "Greeting".to_owned(),
            subject: "Hello {{ recipient_name }}".to_owned(),
            recipients: Some(DraftRecipientFields {
                to: vec![MessageAddress {
                    name: Some("Bob".to_owned()),
                    email: "bob@example.com".to_owned(),
                }],
                cc: Vec::new(),
                bcc: Vec::new(),
            }),
            content: DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"From {{sender_name}}"}]}]}"#.to_owned(),
                html: "<p>From {{sender_name}}</p>".to_owned(),
                plain_text: "From {{sender_name}}".to_owned(),
            },
            revision: 1,
            updated_at: 1,
        };
        let sender = MessageAddress {
            name: Some("Alice <Admin>".to_owned()),
            email: "alice@example.com".to_owned(),
        };
        let recipient = MessageAddress {
            name: Some("Bob".to_owned()),
            email: "bob@example.com".to_owned(),
        };
        let rendered = render_mail_template(
            &template,
            &CompositionRenderContext {
                sender,
                recipient: Some(&recipient),
                language: LanguagePreference::EnUs,
            },
        )
        .expect("rendered template");

        assert_eq!(rendered.subject, "Hello Bob");
        assert_eq!(rendered.recipients, template.recipients);
        assert!(rendered.content.editor_json.contains("Alice <Admin>"));
        assert!(rendered.content.html.contains("Alice &lt;Admin&gt;"));
        assert_eq!(rendered.content.plain_text, "From Alice <Admin>");
    }

    #[test]
    fn reports_missing_recipient_context_and_builds_stable_definition_nodes() {
        let signature = MailSignature {
            id: "signature-one".to_owned(),
            scope: CompositionDefinitionScope::Global,
            account_id: None,
            name: "Recipient-aware".to_owned(),
            content: DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"For {{recipient_email}}"}]}]}"#.to_owned(),
                html: "<p>For {{recipient_email}}</p>".to_owned(),
                plain_text: "For {{recipient_email}}".to_owned(),
            },
            revision: 1,
            updated_at: 1,
        };
        let missing = render_mail_signature(
            &signature,
            &CompositionRenderContext {
                sender: MessageAddress {
                    name: Some("Alice".to_owned()),
                    email: "alice@example.com".to_owned(),
                },
                recipient: None,
                language: LanguagePreference::ZhCn,
            },
        )
        .expect_err("missing recipient");
        assert_eq!(missing.code, "composition.variable_context_missing");

        let rendered = RenderedMailSignature {
            id: "signature-one".to_owned(),
            content: DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Alice"}]}]}"#.to_owned(),
                html: "<p>Alice</p>".to_owned(),
                plain_text: "Alice".to_owned(),
            },
        };
        let assembled = assemble_composition_content(&content(), None, Some(&rendered))
            .expect("assembled signature");
        assert!(assembled.editor_json.contains("nextmailSignatureDivider"));
        assert!(assembled.editor_json.contains("nextmailSignature"));
        assert!(assembled.editor_json.contains("signature-one"));
        assert!(assembled.html.contains("data-nextmail-signature-divider"));
        assert!(assembled.html.contains("data-nextmail-signature-id"));
        assert_eq!(assembled.plain_text, "----------------\n\nAlice");
    }

    #[test]
    fn assembles_action_template_and_signature_without_crossing_original_boundary() {
        let base = DraftContent {
            editor_json: serde_json::json!({
                "type": "doc",
                "content": [
                    { "type": "nextmailReply", "content": [{ "type": "paragraph" }] },
                    {
                        "type": "nextmailOriginalMessage",
                        "attrs": { "sourceHtml": "<table><tr><td>Original</td></tr></table>" },
                        "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Original" }] }]
                    }
                ]
            })
            .to_string(),
            html: "<div data-nextmail-reply=\"\"><p></p></div><div data-nextmail-original-message=\"\"><table><tr><td>Original</td></tr></table></div>".to_owned(),
            plain_text: "\n\n---------- Original message ----------\n\nOriginal".to_owned(),
        };
        let template = RenderedMailTemplate {
            id: "template-one".to_owned(),
            subject: String::new(),
            recipients: None,
            content: DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Reply"}]}]}"#.to_owned(),
                html: "<p>Reply</p>".to_owned(),
                plain_text: "Reply".to_owned(),
            },
        };
        let signature = RenderedMailSignature {
            id: "signature-one".to_owned(),
            content: DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Regards"}]}]}"#.to_owned(),
                html: "<p>Regards</p>".to_owned(),
                plain_text: "Regards".to_owned(),
            },
        };

        let assembled = assemble_composition_content(&base, Some(&template), Some(&signature))
            .expect("assembled action content");
        let document: Value = serde_json::from_str(&assembled.editor_json).unwrap();
        let types = document["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["type"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            [
                "nextmailReply",
                "nextmailSignatureDivider",
                "nextmailSignature",
                "paragraph",
                "nextmailOriginalMessage"
            ]
        );
        assert!(document["content"][0]
            .to_string()
            .contains("nextmailTemplate"));
        assert!(document["content"][4]
            .to_string()
            .contains("<table><tr><td>Original</td></tr></table>"));
        assert!(assembled.html.contains("data-nextmail-signature-divider"));
        assert!(assembled.html.find("signature-one") < assembled.html.find("Original"));
        assert_eq!(
            assembled.plain_text,
            "Reply\n\n----------------\n\nRegards\n\n---------- Original message ----------\n\nOriginal"
        );
    }
}

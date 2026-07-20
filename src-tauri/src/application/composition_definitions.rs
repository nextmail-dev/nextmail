use crate::core::{
    CommandError, CommandResult, DraftContent, LanguagePreference, MailSignature,
    MailSignatureDraft, MailTemplate, MailTemplateDraft, MessageAddress, RenderedMailSignature,
    RenderedMailTemplate,
};
use chrono::{Datelike, Local, NaiveDate};
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
        content: render_content(&template.content, context)?,
    })
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

fn is_empty_content(content: &DraftContent) -> bool {
    content.plain_text.is_empty()
        && (content.html.is_empty() || content.html == "<p></p>")
        && serde_json::from_str(&content.editor_json).is_ok_and(|value| is_empty_document(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CompositionDefinitionScope, MailSignature, MailTemplate, MessageAddress};

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
            content: content(),
        })
        .expect("valid template");

        assert_eq!(value.name, "Follow up");
        assert_eq!(value.subject, "Next steps");
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
            content: DraftContent {
                editor_json: "{".to_owned(),
                html: String::new(),
                plain_text: String::new(),
            },
        })
        .expect_err("invalid JSON");
        assert_eq!(invalid.code, "template.editor_json_invalid");
    }

    #[test]
    fn rejects_unknown_variables_when_saving_definitions() {
        let invalid = normalize_mail_template_draft(MailTemplateDraft {
            name: "Broken".to_owned(),
            subject: "Hello {{account_password}}".to_owned(),
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
        assert!(assembled.editor_json.contains("nextmailSignature"));
        assert!(assembled.editor_json.contains("signature-one"));
        assert!(assembled.html.contains("data-nextmail-signature-id"));
        assert_eq!(assembled.plain_text, "Alice");
    }
}

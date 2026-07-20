use crate::core::{
    CommandError, CommandResult, DraftContent, MailSignatureDraft, MailTemplateDraft,
};

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
    Ok(draft)
}

pub fn normalize_mail_signature_draft(
    mut draft: MailSignatureDraft,
) -> CommandResult<MailSignatureDraft> {
    draft.name = normalize_name(draft.name, "signature")?;
    validate_content(&draft.content, "signature")?;
    Ok(draft)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

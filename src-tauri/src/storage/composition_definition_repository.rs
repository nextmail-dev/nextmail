use crate::core::{
    CommandError, CommandResult, CompositionDefinitionScope, DraftContent, MailSignature,
    MailSignatureDraft, MailTemplate, MailTemplateDraft,
};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::repository::now;

#[derive(Clone)]
pub struct CompositionDefinitionRepository {
    pub(crate) pool: SqlitePool,
}

#[derive(FromRow)]
struct MailTemplateRow {
    id: String,
    name: String,
    subject_template: String,
    editor_json: String,
    html: String,
    plain_text: String,
    revision: i64,
    updated_at: i64,
}

#[derive(FromRow)]
struct MailSignatureRow {
    id: String,
    name: String,
    editor_json: String,
    html: String,
    plain_text: String,
    revision: i64,
    updated_at: i64,
}

impl CompositionDefinitionRepository {
    pub async fn list_mail_templates(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
    ) -> CommandResult<Vec<MailTemplate>> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let rows = sqlx::query_as::<_, MailTemplateRow>(
            "SELECT id, name, subject_template, editor_json, html, plain_text, revision, updated_at \
             FROM mail_templates \
             WHERE (? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ? \
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.list_failed"))?;
        Ok(rows
            .into_iter()
            .map(|row| template_from_row(row, scope, account_id))
            .collect())
    }

    pub async fn create_mail_template(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        draft: &MailTemplateDraft,
    ) -> CommandResult<MailTemplate> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO mail_templates( \
                 id, account_slot_id, name, subject_template, editor_json, html, plain_text, created_at, updated_at \
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(&draft.name)
        .bind(&draft.subject)
        .bind(&draft.content.editor_json)
        .bind(&draft.content.html)
        .bind(&draft.content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.create_failed"))?;
        self.mail_template(&id, account_id, account_slot_id, scope)
            .await
    }

    pub async fn update_mail_template(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        template_id: &str,
        draft: &MailTemplateDraft,
        expected_revision: u64,
    ) -> CommandResult<MailTemplate> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let result = sqlx::query(
            "UPDATE mail_templates SET \
                 name = ?, subject_template = ?, editor_json = ?, html = ?, plain_text = ?, \
                 revision = revision + 1, updated_at = ? \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) \
                 AND revision = ?",
        )
        .bind(&draft.name)
        .bind(&draft.subject)
        .bind(&draft.content.editor_json)
        .bind(&draft.content.html)
        .bind(&draft.content.plain_text)
        .bind(now())
        .bind(template_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.update_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("template.revision_conflict"));
        }
        self.mail_template(template_id, account_id, account_slot_id, scope)
            .await
    }

    pub async fn delete_mail_template(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        template_id: &str,
        expected_revision: u64,
    ) -> CommandResult<()> {
        definition_scope(account_id, account_slot_id)?;
        let result = sqlx::query(
            "DELETE FROM mail_templates \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) \
                 AND revision = ?",
        )
        .bind(template_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.delete_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("template.revision_conflict"));
        }
        Ok(())
    }

    pub async fn list_mail_signatures(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
    ) -> CommandResult<Vec<MailSignature>> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let rows = sqlx::query_as::<_, MailSignatureRow>(
            "SELECT id, name, editor_json, html, plain_text, revision, updated_at \
             FROM mail_signatures \
             WHERE (? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ? \
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.list_failed"))?;
        Ok(rows
            .into_iter()
            .map(|row| signature_from_row(row, scope, account_id))
            .collect())
    }

    pub async fn create_mail_signature(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        draft: &MailSignatureDraft,
    ) -> CommandResult<MailSignature> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO mail_signatures( \
                 id, account_slot_id, name, editor_json, html, plain_text, created_at, updated_at \
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(&draft.name)
        .bind(&draft.content.editor_json)
        .bind(&draft.content.html)
        .bind(&draft.content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.create_failed"))?;
        self.mail_signature(&id, account_id, account_slot_id, scope)
            .await
    }

    pub async fn update_mail_signature(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        signature_id: &str,
        draft: &MailSignatureDraft,
        expected_revision: u64,
    ) -> CommandResult<MailSignature> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let result = sqlx::query(
            "UPDATE mail_signatures SET \
                 name = ?, editor_json = ?, html = ?, plain_text = ?, \
                 revision = revision + 1, updated_at = ? \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) \
                 AND revision = ?",
        )
        .bind(&draft.name)
        .bind(&draft.content.editor_json)
        .bind(&draft.content.html)
        .bind(&draft.content.plain_text)
        .bind(now())
        .bind(signature_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.update_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("signature.revision_conflict"));
        }
        self.mail_signature(signature_id, account_id, account_slot_id, scope)
            .await
    }

    pub async fn delete_mail_signature(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        signature_id: &str,
        expected_revision: u64,
    ) -> CommandResult<()> {
        definition_scope(account_id, account_slot_id)?;
        let result = sqlx::query(
            "DELETE FROM mail_signatures \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) \
                 AND revision = ?",
        )
        .bind(signature_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.delete_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("signature.revision_conflict"));
        }
        Ok(())
    }

    async fn mail_template(
        &self,
        template_id: &str,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        scope: CompositionDefinitionScope,
    ) -> CommandResult<MailTemplate> {
        let row = sqlx::query_as::<_, MailTemplateRow>(
            "SELECT id, name, subject_template, editor_json, html, plain_text, revision, updated_at \
             FROM mail_templates \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?)",
        )
        .bind(template_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.read_failed"))?
        .ok_or_else(|| CommandError::new("template.not_found"))?;
        Ok(template_from_row(row, scope, account_id))
    }

    async fn mail_signature(
        &self,
        signature_id: &str,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        scope: CompositionDefinitionScope,
    ) -> CommandResult<MailSignature> {
        let row = sqlx::query_as::<_, MailSignatureRow>(
            "SELECT id, name, editor_json, html, plain_text, revision, updated_at \
             FROM mail_signatures \
             WHERE id = ? AND ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?)",
        )
        .bind(signature_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.read_failed"))?
        .ok_or_else(|| CommandError::new("signature.not_found"))?;
        Ok(signature_from_row(row, scope, account_id))
    }
}

fn definition_scope(
    account_id: Option<&str>,
    account_slot_id: Option<&str>,
) -> CommandResult<CompositionDefinitionScope> {
    match (account_id, account_slot_id) {
        (None, None) => Ok(CompositionDefinitionScope::Global),
        (Some(_), Some(_)) => Ok(CompositionDefinitionScope::Account),
        _ => Err(CommandError::new("composition.scope_invalid")),
    }
}

fn template_from_row(
    row: MailTemplateRow,
    scope: CompositionDefinitionScope,
    account_id: Option<&str>,
) -> MailTemplate {
    MailTemplate {
        id: row.id,
        scope,
        account_id: account_id.map(str::to_owned),
        name: row.name,
        subject: row.subject_template,
        content: DraftContent {
            editor_json: row.editor_json,
            html: row.html,
            plain_text: row.plain_text,
        },
        revision: row.revision as u64,
        updated_at: row.updated_at,
    }
}

fn signature_from_row(
    row: MailSignatureRow,
    scope: CompositionDefinitionScope,
    account_id: Option<&str>,
) -> MailSignature {
    MailSignature {
        id: row.id,
        scope,
        account_id: account_id.map(str::to_owned),
        name: row.name,
        content: DraftContent {
            editor_json: row.editor_json,
            html: row.html,
            plain_text: row.plain_text,
        },
        revision: row.revision as u64,
        updated_at: row.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{create_account_slot, initialize_content_database, MailRepository};

    fn content(value: &str) -> DraftContent {
        DraftContent {
            editor_json: format!(
                r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{value}"}}]}}]}}"#
            ),
            html: format!("<p>{value}</p>"),
            plain_text: value.to_owned(),
        }
    }

    #[tokio::test]
    async fn keeps_global_and_account_definitions_in_explicit_scopes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        initialize_content_database(directory.path())
            .await
            .expect("initialize database");
        let repository = MailRepository::open(directory.path())
            .await
            .expect("repository");
        create_account_slot(directory.path(), "slot-one", 1)
            .await
            .expect("first slot");
        create_account_slot(directory.path(), "slot-two", 2)
            .await
            .expect("second slot");
        let definitions = repository.composition_definitions();

        definitions
            .create_mail_template(
                None,
                None,
                &MailTemplateDraft {
                    name: "Global".to_owned(),
                    subject: "Hello".to_owned(),
                    content: content("Shared body"),
                },
            )
            .await
            .expect("global template");
        definitions
            .create_mail_template(
                Some("account-one"),
                Some("slot-one"),
                &MailTemplateDraft {
                    name: "Account".to_owned(),
                    subject: String::new(),
                    content: content("Private body"),
                },
            )
            .await
            .expect("account template");

        let global = definitions
            .list_mail_templates(None, None)
            .await
            .expect("global list");
        let first = definitions
            .list_mail_templates(Some("account-one"), Some("slot-one"))
            .await
            .expect("first account list");
        let second = definitions
            .list_mail_templates(Some("account-two"), Some("slot-two"))
            .await
            .expect("second account list");

        assert_eq!(global.len(), 1);
        assert_eq!(global[0].scope, CompositionDefinitionScope::Global);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].account_id.as_deref(), Some("account-one"));
        assert!(second.is_empty());
    }

    #[tokio::test]
    async fn persists_signature_updates_and_rejects_stale_revisions() {
        let directory = tempfile::tempdir().expect("temporary directory");
        initialize_content_database(directory.path())
            .await
            .expect("initialize database");
        let repository = MailRepository::open(directory.path())
            .await
            .expect("repository");
        let definitions = repository.composition_definitions();
        let signature = definitions
            .create_mail_signature(
                None,
                None,
                &MailSignatureDraft {
                    name: "Default".to_owned(),
                    content: content("Alice"),
                },
            )
            .await
            .expect("signature");
        let updated = definitions
            .update_mail_signature(
                None,
                None,
                &signature.id,
                &MailSignatureDraft {
                    name: "Primary".to_owned(),
                    content: content("Alice Example"),
                },
                signature.revision,
            )
            .await
            .expect("update signature");

        let stale = definitions
            .update_mail_signature(
                None,
                None,
                &signature.id,
                &MailSignatureDraft {
                    name: "Stale".to_owned(),
                    content: content("Old"),
                },
                signature.revision,
            )
            .await
            .expect_err("stale revision");
        assert_eq!(stale.code, "signature.revision_conflict");

        drop(repository);
        let reopened = MailRepository::open(directory.path())
            .await
            .expect("reopened repository");
        let stored = reopened
            .composition_definitions()
            .list_mail_signatures(None, None)
            .await
            .expect("stored signatures");
        assert_eq!(stored[0].name, "Primary");
        assert_eq!(stored[0].revision, updated.revision);
    }
}

use crate::core::{
    CommandError, CommandResult, CompositionDefinitionScope, CompositionScene,
    CompositionSceneRule, CompositionSceneRuleDraft, DraftContent, MailSignature,
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
    account_slot_id: Option<String>,
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
    account_slot_id: Option<String>,
    name: String,
    editor_json: String,
    html: String,
    plain_text: String,
    revision: i64,
    updated_at: i64,
}

#[derive(FromRow)]
struct CompositionSceneRuleRow {
    template_id: Option<String>,
    signature_id: Option<String>,
    revision: i64,
}

impl CompositionDefinitionRepository {
    pub async fn list_mail_templates(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
    ) -> CommandResult<Vec<MailTemplate>> {
        let scope = definition_scope(account_id, account_slot_id)?;
        let rows = sqlx::query_as::<_, MailTemplateRow>(
            "SELECT id, account_slot_id, name, subject_template, editor_json, html, plain_text, revision, updated_at \
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
        let references = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM composition_scene_rules WHERE template_id = ?",
        )
        .bind(template_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.delete_failed"))?;
        if references > 0 {
            return Err(CommandError::new("template.in_use"));
        }
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
            "SELECT id, account_slot_id, name, editor_json, html, plain_text, revision, updated_at \
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
        let references = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM composition_scene_rules WHERE signature_id = ?",
        )
        .bind(signature_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.delete_failed"))?;
        if references > 0 {
            return Err(CommandError::new("signature.in_use"));
        }
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

    pub async fn available_mail_templates(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<Vec<MailTemplate>> {
        let rows = sqlx::query_as::<_, MailTemplateRow>(
            "SELECT id, account_slot_id, name, subject_template, editor_json, html, plain_text, revision, updated_at \
             FROM mail_templates WHERE account_slot_id IS NULL OR account_slot_id = ? \
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("template.list_failed"))?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let is_account = row.account_slot_id.is_some();
                template_from_row(
                    row,
                    if is_account {
                        CompositionDefinitionScope::Account
                    } else {
                        CompositionDefinitionScope::Global
                    },
                    is_account.then_some(account_id),
                )
            })
            .collect())
    }

    pub async fn available_mail_signatures(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<Vec<MailSignature>> {
        let rows = sqlx::query_as::<_, MailSignatureRow>(
            "SELECT id, account_slot_id, name, editor_json, html, plain_text, revision, updated_at \
             FROM mail_signatures WHERE account_slot_id IS NULL OR account_slot_id = ? \
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("signature.list_failed"))?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let is_account = row.account_slot_id.is_some();
                signature_from_row(
                    row,
                    if is_account {
                        CompositionDefinitionScope::Account
                    } else {
                        CompositionDefinitionScope::Global
                    },
                    is_account.then_some(account_id),
                )
            })
            .collect())
    }

    pub async fn available_mail_template(
        &self,
        account_id: &str,
        account_slot_id: &str,
        template_id: &str,
    ) -> CommandResult<MailTemplate> {
        self.available_mail_templates(account_id, account_slot_id)
            .await?
            .into_iter()
            .find(|value| value.id == template_id)
            .ok_or_else(|| CommandError::new("template.not_found"))
    }

    pub async fn available_mail_signature(
        &self,
        account_id: &str,
        account_slot_id: &str,
        signature_id: &str,
    ) -> CommandResult<MailSignature> {
        self.available_mail_signatures(account_id, account_slot_id)
            .await?
            .into_iter()
            .find(|value| value.id == signature_id)
            .ok_or_else(|| CommandError::new("signature.not_found"))
    }

    pub async fn list_composition_scene_rules(
        &self,
        account_slot_id: Option<&str>,
    ) -> CommandResult<Vec<CompositionSceneRule>> {
        let mut rules = Vec::new();
        for scene in all_scenes() {
            let exact = self.exact_scene_rule(account_slot_id, scene).await?;
            if account_slot_id.is_some() && exact.is_none() {
                let inherited = self.exact_scene_rule(None, scene).await?;
                rules.push(rule_from_row(scene, inherited, true));
            } else {
                rules.push(rule_from_row(scene, exact, false));
            }
        }
        Ok(rules)
    }

    pub async fn save_composition_scene_rule(
        &self,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        draft: &CompositionSceneRuleDraft,
        expected_revision: u64,
    ) -> CommandResult<CompositionSceneRule> {
        definition_scope(account_id, account_slot_id)?;
        if account_slot_id.is_none() && draft.inherit {
            return Err(CommandError::new("composition_rule.global_cannot_inherit"));
        }
        if draft.inherit {
            if expected_revision == 0
                && self
                    .exact_scene_rule(account_slot_id, draft.scene)
                    .await?
                    .is_some()
            {
                return Err(CommandError::new("composition_rule.revision_conflict"));
            }
            let result = sqlx::query(
                "DELETE FROM composition_scene_rules WHERE account_slot_id = ? AND scene = ? AND revision = ?",
            )
            .bind(account_slot_id)
            .bind(scene_name(draft.scene))
            .bind(expected_revision as i64)
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("composition_rule.save_failed"))?;
            if expected_revision > 0 && result.rows_affected() != 1 {
                return Err(CommandError::new("composition_rule.revision_conflict"));
            }
            return Ok(rule_from_row(
                draft.scene,
                self.exact_scene_rule(None, draft.scene).await?,
                true,
            ));
        }
        self.validate_rule_references(account_slot_id, draft)
            .await?;
        let timestamp = now();
        if expected_revision == 0 {
            sqlx::query(
                "INSERT INTO composition_scene_rules(id, account_slot_id, scene, template_id, signature_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(account_slot_id)
            .bind(scene_name(draft.scene))
            .bind(&draft.template_id)
            .bind(&draft.signature_id)
            .bind(timestamp)
            .bind(timestamp)
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("composition_rule.revision_conflict"))?;
        } else {
            let result = sqlx::query(
                "UPDATE composition_scene_rules SET template_id = ?, signature_id = ?, revision = revision + 1, updated_at = ? \
                 WHERE ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) AND scene = ? AND revision = ?",
            )
            .bind(&draft.template_id)
            .bind(&draft.signature_id)
            .bind(timestamp)
            .bind(account_slot_id)
            .bind(account_slot_id)
            .bind(scene_name(draft.scene))
            .bind(expected_revision as i64)
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("composition_rule.save_failed"))?;
            if result.rows_affected() != 1 {
                return Err(CommandError::new("composition_rule.revision_conflict"));
            }
        }
        Ok(rule_from_row(
            draft.scene,
            self.exact_scene_rule(account_slot_id, draft.scene).await?,
            false,
        ))
    }

    pub async fn resolved_composition_scene_rule(
        &self,
        account_slot_id: &str,
        scene: CompositionScene,
    ) -> CommandResult<CompositionSceneRule> {
        if let Some(row) = self.exact_scene_rule(Some(account_slot_id), scene).await? {
            return Ok(rule_from_row(scene, Some(row), false));
        }
        Ok(rule_from_row(
            scene,
            self.exact_scene_rule(None, scene).await?,
            true,
        ))
    }

    async fn validate_rule_references(
        &self,
        account_slot_id: Option<&str>,
        draft: &CompositionSceneRuleDraft,
    ) -> CommandResult<()> {
        if let Some(template_id) = draft.template_id.as_deref() {
            let found = definition_reference_exists(
                &self.pool,
                "mail_templates",
                account_slot_id,
                template_id,
            )
            .await?;
            if !found {
                return Err(CommandError::new("composition_rule.template_unavailable"));
            }
        }
        if let Some(signature_id) = draft.signature_id.as_deref() {
            let found = definition_reference_exists(
                &self.pool,
                "mail_signatures",
                account_slot_id,
                signature_id,
            )
            .await?;
            if !found {
                return Err(CommandError::new("composition_rule.signature_unavailable"));
            }
        }
        Ok(())
    }

    async fn exact_scene_rule(
        &self,
        account_slot_id: Option<&str>,
        scene: CompositionScene,
    ) -> CommandResult<Option<CompositionSceneRuleRow>> {
        sqlx::query_as::<_, CompositionSceneRuleRow>(
            "SELECT template_id, signature_id, revision FROM composition_scene_rules \
             WHERE ((? IS NULL AND account_slot_id IS NULL) OR account_slot_id = ?) AND scene = ?",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(scene_name(scene))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("composition_rule.read_failed"))
    }

    async fn mail_template(
        &self,
        template_id: &str,
        account_id: Option<&str>,
        account_slot_id: Option<&str>,
        scope: CompositionDefinitionScope,
    ) -> CommandResult<MailTemplate> {
        let row = sqlx::query_as::<_, MailTemplateRow>(
            "SELECT id, account_slot_id, name, subject_template, editor_json, html, plain_text, revision, updated_at \
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
            "SELECT id, account_slot_id, name, editor_json, html, plain_text, revision, updated_at \
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

fn all_scenes() -> [CompositionScene; 4] {
    [
        CompositionScene::New,
        CompositionScene::Reply,
        CompositionScene::ReplyAll,
        CompositionScene::Forward,
    ]
}

fn scene_name(scene: CompositionScene) -> &'static str {
    match scene {
        CompositionScene::New => "new",
        CompositionScene::Reply => "reply",
        CompositionScene::ReplyAll => "reply_all",
        CompositionScene::Forward => "forward",
    }
}

fn rule_from_row(
    scene: CompositionScene,
    row: Option<CompositionSceneRuleRow>,
    inherited: bool,
) -> CompositionSceneRule {
    row.map_or(
        CompositionSceneRule {
            scene,
            template_id: None,
            signature_id: None,
            inherited,
            revision: 0,
        },
        |row| CompositionSceneRule {
            scene,
            template_id: row.template_id,
            signature_id: row.signature_id,
            inherited,
            revision: if inherited { 0 } else { row.revision as u64 },
        },
    )
}

async fn definition_reference_exists(
    pool: &SqlitePool,
    table: &str,
    account_slot_id: Option<&str>,
    definition_id: &str,
) -> CommandResult<bool> {
    let query = match table {
        "mail_templates" => {
            "SELECT COUNT(*) FROM mail_templates WHERE id = ? AND \
             ((? IS NULL AND account_slot_id IS NULL) OR (? IS NOT NULL AND (account_slot_id IS NULL OR account_slot_id = ?)))"
        }
        "mail_signatures" => {
            "SELECT COUNT(*) FROM mail_signatures WHERE id = ? AND \
             ((? IS NULL AND account_slot_id IS NULL) OR (? IS NOT NULL AND (account_slot_id IS NULL OR account_slot_id = ?)))"
        }
        _ => return Err(CommandError::new("composition_rule.reference_invalid")),
    };
    let count = sqlx::query_scalar::<_, i64>(query)
        .bind(definition_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_one(pool)
        .await
        .map_err(|_| CommandError::new("composition_rule.read_failed"))?;
    Ok(count == 1)
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

    #[tokio::test]
    async fn resolves_account_rules_over_global_rules_and_protects_references() {
        let directory = tempfile::tempdir().expect("temporary directory");
        initialize_content_database(directory.path())
            .await
            .expect("initialize database");
        let repository = MailRepository::open(directory.path())
            .await
            .expect("repository");
        create_account_slot(directory.path(), "slot-one", 1)
            .await
            .expect("account slot");
        let definitions = repository.composition_definitions();
        let template = definitions
            .create_mail_template(
                None,
                None,
                &MailTemplateDraft {
                    name: "Shared".to_owned(),
                    subject: String::new(),
                    content: content("Shared"),
                },
            )
            .await
            .expect("template");
        let global = definitions
            .save_composition_scene_rule(
                None,
                None,
                &CompositionSceneRuleDraft {
                    scene: CompositionScene::New,
                    template_id: Some(template.id.clone()),
                    signature_id: None,
                    inherit: false,
                },
                0,
            )
            .await
            .expect("global rule");
        assert_eq!(global.revision, 1);
        for scene in [
            CompositionScene::Reply,
            CompositionScene::ReplyAll,
            CompositionScene::Forward,
        ] {
            definitions
                .save_composition_scene_rule(
                    None,
                    None,
                    &CompositionSceneRuleDraft {
                        scene,
                        template_id: None,
                        signature_id: None,
                        inherit: false,
                    },
                    0,
                )
                .await
                .expect("global scene rule");
        }
        let global_rules = definitions
            .list_composition_scene_rules(None)
            .await
            .expect("four global rules");
        assert_eq!(global_rules.len(), 4);
        assert!(global_rules.iter().all(|value| value.revision == 1));

        let inherited = definitions
            .list_composition_scene_rules(Some("slot-one"))
            .await
            .expect("account rules");
        let inherited_new = inherited
            .iter()
            .find(|value| value.scene == CompositionScene::New)
            .expect("new rule");
        assert!(inherited_new.inherited);
        assert_eq!(inherited_new.revision, 0);
        assert_eq!(
            inherited_new.template_id.as_deref(),
            Some(template.id.as_str())
        );

        let account = definitions
            .save_composition_scene_rule(
                Some("account-one"),
                Some("slot-one"),
                &CompositionSceneRuleDraft {
                    scene: CompositionScene::New,
                    template_id: None,
                    signature_id: None,
                    inherit: false,
                },
                0,
            )
            .await
            .expect("account override");
        assert!(!account.inherited);
        let resolved = definitions
            .resolved_composition_scene_rule("slot-one", CompositionScene::New)
            .await
            .expect("resolved account rule");
        assert_eq!(resolved.template_id, None);

        let protected = definitions
            .delete_mail_template(None, None, &template.id, template.revision)
            .await
            .expect_err("referenced template");
        assert_eq!(protected.code, "template.in_use");
    }
}

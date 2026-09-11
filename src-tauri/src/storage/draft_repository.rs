mod send_job_repository;

use crate::core::{
    CommandError, CommandResult, ComposedMessageActionDraft, DraftAttachmentSummary, DraftContent,
    DraftDetail, DraftListItem, DraftRecipientFields, DraftStatus, ImportedDraftSource,
    MessageActionSource, MessageAddress, MessageComposeAction, SendJobStatus,
};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{now, sanitize_attachment_file_name, ContentStore};

#[derive(Clone)]
pub struct DraftRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone)]
pub struct SendJobRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone, Debug)]
pub struct StoredDraftAttachment {
    pub summary: DraftAttachmentSummary,
    pub content_hash: String,
}

#[derive(Clone, Debug)]
pub struct ClaimedSendJob {
    pub id: String,
    pub draft_id: String,
    pub account_slot_id: String,
    pub mime_hash: String,
    pub envelope_recipients: Vec<String>,
    pub attempt_count: u32,
    pub revision: u64,
}

pub struct SaveDraftRequest<'a> {
    pub account_id: &'a str,
    pub account_slot_id: &'a str,
    pub draft_id: &'a str,
    pub recipients: &'a DraftRecipientFields,
    pub subject: &'a str,
    pub content: &'a DraftContent,
    pub expected_revision: u64,
}

pub struct PersistMessageActionDraftRequest<'a> {
    pub account_id: &'a str,
    pub account_slot_id: &'a str,
    pub message_id: &'a str,
    pub action: MessageComposeAction,
    pub draft: &'a ComposedMessageActionDraft,
}

pub struct PersistImportedDraftRequest<'a> {
    pub account_id: &'a str,
    pub account_slot_id: &'a str,
    pub message_id: &'a str,
    pub source: &'a ImportedDraftSource,
    pub content: &'a DraftContent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftThreadingHeaders {
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

impl DraftRepository {
    pub async fn list_editing_drafts(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<Vec<DraftListItem>> {
        let rows = sqlx::query(
            "SELECT id, subject, to_json, updated_at FROM drafts \
             WHERE account_slot_id = ? AND status = 'editing' ORDER BY updated_at DESC",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.list_failed", false, &error))?;
        rows.into_iter()
            .map(|row| {
                Ok(DraftListItem {
                    id: row.try_get("id").map_err(read_error)?,
                    account_id: account_id.to_owned(),
                    subject: row.try_get("subject").map_err(read_error)?,
                    recipients: decode_addresses(row.try_get("to_json").map_err(read_error)?)?,
                    updated_at: row.try_get("updated_at").map_err(read_error)?,
                })
            })
            .collect()
    }

    pub async fn create_draft(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<DraftDetail> {
        self.create_initialized_draft(
            account_id,
            account_slot_id,
            "",
            &DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.to_owned(),
                html: "<p></p>".to_owned(),
                plain_text: String::new(),
            },
        )
        .await
    }

    pub async fn create_initialized_draft(
        &self,
        account_id: &str,
        account_slot_id: &str,
        subject: &str,
        content: &DraftContent,
    ) -> CommandResult<DraftDetail> {
        self.create_initialized_draft_with_recipients(
            account_id,
            account_slot_id,
            &DraftRecipientFields::default(),
            subject,
            content,
        )
        .await
    }

    pub async fn create_initialized_draft_with_recipients(
        &self,
        account_id: &str,
        account_slot_id: &str,
        recipients: &DraftRecipientFields,
        subject: &str,
        content: &DraftContent,
    ) -> CommandResult<DraftDetail> {
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let to_json = encode_addresses(&recipients.to)?;
        let cc_json = encode_addresses(&recipients.cc)?;
        let bcc_json = encode_addresses(&recipients.bcc)?;
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, to_json, cc_json, bcc_json, subject, editor_json, html, plain_text, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(to_json)
        .bind(cc_json)
        .bind(bcc_json)
        .bind(subject)
        .bind(&content.editor_json)
        .bind(&content.html)
        .bind(&content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.create_failed", false, &error))?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn message_action_source(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<MessageActionSource> {
        let message = sqlx::query(
            "SELECT subject, from_json, to_json, cc_json, received_at, message_id, references_json \
             FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.create_from_message_failed", false, &error))?
        .ok_or_else(|| CommandError::new("message.not_found"))?;
        let body = sqlx::query(
            "SELECT b.plain_text, b.safe_html FROM message_bodies b \
             INNER JOIN messages m ON m.id = b.message_id \
             WHERE b.message_id = ? AND m.account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("draft.create_from_message_failed", false, &error)
        })?;
        let (plain_text, safe_html) = if let Some(row) = body {
            (
                row.try_get::<Option<String>, _>("plain_text")
                    .map_err(read_error)?
                    .unwrap_or_default(),
                row.try_get::<Option<String>, _>("safe_html")
                    .map_err(read_error)?,
            )
        } else {
            (String::new(), None)
        };
        Ok(MessageActionSource {
            subject: message.try_get("subject").map_err(read_error)?,
            from: decode_addresses(message.try_get("from_json").map_err(read_error)?)?,
            to: decode_addresses(message.try_get("to_json").map_err(read_error)?)?,
            cc: decode_addresses(message.try_get("cc_json").map_err(read_error)?)?,
            received_at: message.try_get("received_at").map_err(read_error)?,
            message_id: message.try_get("message_id").map_err(read_error)?,
            references: serde_json::from_str(
                &message
                    .try_get::<String, _>("references_json")
                    .map_err(read_error)?,
            )
            .map_err(json_error)?,
            plain_text,
            safe_html,
        })
    }

    pub async fn persist_message_action_draft(
        &self,
        request: PersistMessageActionDraftRequest<'_>,
    ) -> CommandResult<DraftDetail> {
        let PersistMessageActionDraftRequest {
            account_id,
            account_slot_id,
            message_id,
            action,
            draft,
        } = request;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let mut transaction = super::begin_write(&self.pool).await.map_err(|error| {
            crate::diagnostics::command_error("draft.create_from_message_failed", false, &error)
        })?;
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, related_message_id, in_reply_to, references_json, \
             to_json, cc_json, subject, editor_json, html, plain_text, discard_if_untouched, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(message_id)
        .bind(&draft.in_reply_to)
        .bind(serde_json::to_string(&draft.references).map_err(json_error)?)
        .bind(encode_addresses(&draft.recipients.to)?)
        .bind(encode_addresses(&draft.recipients.cc)?)
        .bind(&draft.subject)
        .bind(&draft.content.editor_json)
        .bind(&draft.content.html)
        .bind(&draft.content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.create_from_message_failed", false, &error))?;

        if action == MessageComposeAction::Forward {
            let attachments = sqlx::query(
                "SELECT file_name, content_type, size, content_hash FROM attachments \
                 WHERE message_id = ? AND content_hash IS NOT NULL AND content_id IS NULL ORDER BY part_index",
            )
            .bind(message_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(|error| crate::diagnostics::command_error("draft.create_from_message_failed", false, &error))?;
            for (index, attachment) in attachments.into_iter().enumerate() {
                sqlx::query(
                    "INSERT INTO draft_attachments(id, draft_id, file_name, content_type, size, content_hash, sort_order, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&id)
                .bind(attachment.try_get::<String, _>("file_name").map_err(read_error)?)
                .bind(attachment.try_get::<String, _>("content_type").map_err(read_error)?)
                .bind(attachment.try_get::<i64, _>("size").map_err(read_error)?)
                .bind(attachment.try_get::<String, _>("content_hash").map_err(read_error)?)
                .bind(index as i64)
                .bind(timestamp)
                .execute(&mut *transaction)
                .await
                .map_err(|error| crate::diagnostics::command_error("draft.create_from_message_failed", false, &error))?;
            }
        }
        transaction.commit().await.map_err(|error| {
            crate::diagnostics::command_error("draft.create_from_message_failed", false, &error)
        })?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn draft_threading_headers(
        &self,
        account_slot_id: &str,
        draft_id: &str,
    ) -> CommandResult<DraftThreadingHeaders> {
        let row = sqlx::query(
            "SELECT in_reply_to, references_json FROM drafts WHERE id = ? AND account_slot_id = ?",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.read_failed", false, &error))?
        .ok_or_else(|| CommandError::new("draft.not_found"))?;
        Ok(DraftThreadingHeaders {
            in_reply_to: row.try_get("in_reply_to").map_err(read_error)?,
            references: serde_json::from_str(
                &row.try_get::<String, _>("references_json")
                    .map_err(read_error)?,
            )
            .map_err(json_error)?,
        })
    }

    pub async fn existing_imported_draft(
        &self,
        account_id: &str,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<Option<DraftDetail>> {
        if let Some(existing) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM drafts WHERE account_slot_id = ? AND source_message_id = ? AND status = 'editing'",
        )
        .bind(account_slot_id)
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.import_failed", false, &error))?
        {
            return self
                .get_draft(account_id, account_slot_id, &existing)
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub async fn imported_draft_source(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<ImportedDraftSource> {
        let message = sqlx::query(
            "SELECT subject, to_json, cc_json FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.import_failed", false, &error))?
        .ok_or_else(|| CommandError::new("message.not_found"))?;
        let body =
            sqlx::query("SELECT plain_text, safe_html FROM message_bodies WHERE message_id = ?")
                .bind(message_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| {
                    crate::diagnostics::command_error("draft.import_failed", false, &error)
                })?;
        let plain_text = body
            .as_ref()
            .and_then(|row| row.try_get::<Option<String>, _>("plain_text").ok())
            .flatten()
            .unwrap_or_default();
        let safe_html = body
            .as_ref()
            .and_then(|row| row.try_get::<Option<String>, _>("safe_html").ok())
            .flatten();
        Ok(ImportedDraftSource {
            recipients: DraftRecipientFields {
                to: decode_addresses(message.try_get("to_json").map_err(read_error)?)?,
                cc: decode_addresses(message.try_get("cc_json").map_err(read_error)?)?,
                bcc: Vec::new(),
            },
            subject: message.try_get("subject").map_err(read_error)?,
            plain_text,
            safe_html,
        })
    }

    pub async fn persist_imported_draft(
        &self,
        request: PersistImportedDraftRequest<'_>,
    ) -> CommandResult<DraftDetail> {
        let PersistImportedDraftRequest {
            account_id,
            account_slot_id,
            message_id,
            source,
            content,
        } = request;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let mut transaction = super::begin_write(&self.pool).await.map_err(|error| {
            crate::diagnostics::command_error("draft.import_failed", false, &error)
        })?;
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, source_message_id, to_json, cc_json, subject, \
             editor_json, html, plain_text, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(message_id)
        .bind(encode_addresses(&source.recipients.to)?)
        .bind(encode_addresses(&source.recipients.cc)?)
        .bind(&source.subject)
        .bind(&content.editor_json)
        .bind(&content.html)
        .bind(&content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.import_failed", false, &error))?;
        let attachments = sqlx::query(
            "SELECT file_name, content_type, size, content_hash, content_id FROM attachments \
             WHERE message_id = ? AND content_hash IS NOT NULL ORDER BY part_index",
        )
        .bind(message_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.import_failed", false, &error))?;
        for (index, attachment) in attachments.into_iter().enumerate() {
            sqlx::query(
                "INSERT INTO draft_attachments(id, draft_id, file_name, content_type, size, content_hash, content_id, is_inline, sort_order, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(attachment.try_get::<String, _>("file_name").map_err(read_error)?)
            .bind(attachment.try_get::<String, _>("content_type").map_err(read_error)?)
            .bind(attachment.try_get::<i64, _>("size").map_err(read_error)?)
            .bind(attachment.try_get::<String, _>("content_hash").map_err(read_error)?)
            .bind(attachment.try_get::<Option<String>, _>("content_id").map_err(read_error)?)
            .bind(i64::from(
                attachment
                    .try_get::<Option<String>, _>("content_id")
                    .map_err(read_error)?
                    .is_some(),
            ))
            .bind(index as i64)
            .bind(timestamp)
            .execute(&mut *transaction)
            .await
            .map_err(|error| crate::diagnostics::command_error("draft.import_failed", false, &error))?;
        }
        transaction.commit().await.map_err(|error| {
            crate::diagnostics::command_error("draft.import_failed", false, &error)
        })?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn discard_empty_draft(
        &self,
        account_slot_id: &str,
        draft_id: &str,
    ) -> CommandResult<bool> {
        let result = sqlx::query(
            "DELETE FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing' \
             AND ((discard_if_untouched = 1 AND user_edited = 0) OR ( \
               subject = '' AND to_json = '[]' AND cc_json = '[]' AND bcc_json = '[]' \
               AND plain_text = '' AND (html = '' OR html = '<p></p>') \
               AND NOT EXISTS(SELECT 1 FROM draft_attachments WHERE draft_id = drafts.id)))",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("draft.discard_failed", false, &error)
        })?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete_editing_draft(
        &self,
        account_slot_id: &str,
        draft_id: &str,
    ) -> CommandResult<()> {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM drafts WHERE id = ? AND account_slot_id = ?",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.read_failed", false, &error))?
        .ok_or_else(|| CommandError::new("draft.not_found"))?;
        if status != "editing" {
            return Err(CommandError::new("draft.not_editable"));
        }
        let result = sqlx::query(
            "DELETE FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing'",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.delete_failed", false, &error))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("draft.delete_failed"));
        }
        Ok(())
    }

    pub async fn get_draft(
        &self,
        account_id: &str,
        account_slot_id: &str,
        draft_id: &str,
    ) -> CommandResult<DraftDetail> {
        let row = sqlx::query(
            "SELECT id, status, to_json, cc_json, bcc_json, subject, editor_json, html, plain_text, revision \
             FROM drafts WHERE id = ? AND account_slot_id = ?",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.read_failed", false, &error))?
        .ok_or_else(|| CommandError::new("draft.not_found"))?;
        let attachments = self.draft_attachments(account_slot_id, draft_id).await?;
        Ok(DraftDetail {
            id: row.try_get("id").map_err(read_error)?,
            account_id: account_id.to_owned(),
            status: draft_status(row.try_get("status").map_err(read_error)?),
            recipients: DraftRecipientFields {
                to: decode_addresses(row.try_get("to_json").map_err(read_error)?)?,
                cc: decode_addresses(row.try_get("cc_json").map_err(read_error)?)?,
                bcc: decode_addresses(row.try_get("bcc_json").map_err(read_error)?)?,
            },
            subject: row.try_get("subject").map_err(read_error)?,
            content: DraftContent {
                editor_json: row.try_get("editor_json").map_err(read_error)?,
                html: row.try_get("html").map_err(read_error)?,
                plain_text: row.try_get("plain_text").map_err(read_error)?,
            },
            attachments: attachments.into_iter().map(|value| value.summary).collect(),
            revision: row.try_get::<i64, _>("revision").map_err(read_error)? as u64,
        })
    }

    pub async fn save_draft(&self, request: SaveDraftRequest<'_>) -> CommandResult<DraftDetail> {
        let result = sqlx::query(
            "UPDATE drafts SET to_json = ?, cc_json = ?, bcc_json = ?, subject = ?, editor_json = ?, \
             html = ?, plain_text = ?, user_edited = 1, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND account_slot_id = ? AND revision = ? AND status = 'editing'",
        )
        .bind(encode_addresses(&request.recipients.to)?)
        .bind(encode_addresses(&request.recipients.cc)?)
        .bind(encode_addresses(&request.recipients.bcc)?)
        .bind(request.subject)
        .bind(&request.content.editor_json)
        .bind(&request.content.html)
        .bind(&request.content.plain_text)
        .bind(now())
        .bind(request.draft_id)
        .bind(request.account_slot_id)
        .bind(request.expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.save_failed", false, &error))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("draft.revision_conflict"));
        }
        self.get_draft(
            request.account_id,
            request.account_slot_id,
            request.draft_id,
        )
        .await
    }

    pub async fn add_draft_attachment(
        &self,
        account_slot_id: &str,
        draft_id: &str,
        file_name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> CommandResult<DraftAttachmentSummary> {
        let editable = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing'",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.read_failed", false, &error))?;
        if editable != 1 {
            return Err(CommandError::new("draft.not_editable"));
        }
        let hash = self.content.write_attachment(bytes).await?;
        let id = Uuid::new_v4().to_string();
        let sort_order = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM draft_attachments WHERE draft_id = ?",
        )
        .bind(draft_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("draft.attachment_write_failed", false, &error)
        })?;
        sqlx::query(
            "INSERT INTO draft_attachments(id, draft_id, file_name, content_type, size, content_hash, sort_order, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(draft_id)
        .bind(file_name)
        .bind(content_type)
        .bind(bytes.len() as i64)
        .bind(hash)
        .bind(sort_order)
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.attachment_write_failed", false, &error))?;
        Ok(DraftAttachmentSummary {
            id,
            file_name: file_name.to_owned(),
            content_type: content_type.to_owned(),
            size: bytes.len() as u64,
            content_id: None,
            is_inline: false,
            preview_data_url: None,
        })
    }

    pub async fn add_draft_inline_image(
        &self,
        account_slot_id: &str,
        draft_id: &str,
        file_name: &str,
        content_type: &str,
        content_id: Option<&str>,
        bytes: &[u8],
    ) -> CommandResult<DraftAttachmentSummary> {
        let editable = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing'",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.read_failed", false, &error))?;
        if editable != 1 {
            return Err(CommandError::new("draft.not_editable"));
        }
        let content_id = content_id
            .map(str::trim)
            .map(|value| value.trim_matches(['<', '>']))
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 255
                    && value
                        .chars()
                        .all(|character| !character.is_control() && !character.is_whitespace())
            })
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}@nextmail.local", Uuid::new_v4()));
        let hash = self.content.write_attachment(bytes).await?;
        let id = Uuid::new_v4().to_string();
        let file_name = sanitize_attachment_file_name(file_name);
        let sort_order = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM draft_attachments WHERE draft_id = ?",
        )
        .bind(draft_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("draft.attachment_write_failed", false, &error)
        })?;
        sqlx::query(
            "INSERT INTO draft_attachments(id, draft_id, file_name, content_type, size, content_hash, content_id, is_inline, sort_order, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(&id)
        .bind(draft_id)
        .bind(&file_name)
        .bind(content_type)
        .bind(bytes.len() as i64)
        .bind(hash)
        .bind(&content_id)
        .bind(sort_order)
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|error| {
            if error.to_string().contains("draft_attachments_inline_cid_idx") {
                CommandError::new("draft.inline_image_duplicate")
            } else {
                CommandError::new("draft.attachment_write_failed")
            }
        })?;
        Ok(DraftAttachmentSummary {
            id,
            file_name,
            content_type: content_type.to_owned(),
            size: bytes.len() as u64,
            content_id: Some(content_id),
            is_inline: true,
            preview_data_url: None,
        })
    }

    pub async fn remove_draft_attachment(
        &self,
        account_slot_id: &str,
        draft_id: &str,
        attachment_id: &str,
    ) -> CommandResult<()> {
        let result = sqlx::query(
            "DELETE FROM draft_attachments WHERE id = ? AND draft_id = ? \
             AND EXISTS(SELECT 1 FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing')",
        )
        .bind(attachment_id)
        .bind(draft_id)
        .bind(draft_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.attachment_remove_failed", false, &error))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("draft.attachment_not_found"));
        }
        Ok(())
    }

    pub async fn draft_attachments(
        &self,
        account_slot_id: &str,
        draft_id: &str,
    ) -> CommandResult<Vec<StoredDraftAttachment>> {
        let rows = sqlx::query(
            "SELECT a.id, a.file_name, a.content_type, a.size, a.content_hash, a.content_id, a.is_inline FROM draft_attachments a \
             JOIN drafts d ON d.id = a.draft_id WHERE a.draft_id = ? AND d.account_slot_id = ? \
             ORDER BY a.sort_order, a.id",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| crate::diagnostics::command_error("draft.attachment_read_failed", false, &error))?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredDraftAttachment {
                    summary: DraftAttachmentSummary {
                        id: row.try_get("id").map_err(read_error)?,
                        file_name: row.try_get("file_name").map_err(read_error)?,
                        content_type: row.try_get("content_type").map_err(read_error)?,
                        size: row.try_get::<i64, _>("size").map_err(read_error)? as u64,
                        content_id: row.try_get("content_id").map_err(read_error)?,
                        is_inline: row.try_get::<i64, _>("is_inline").map_err(read_error)? != 0,
                        preview_data_url: None,
                    },
                    content_hash: row.try_get("content_hash").map_err(read_error)?,
                })
            })
            .collect()
    }

    pub async fn attachment_bytes(&self, hash: &str) -> CommandResult<Vec<u8>> {
        self.content.read_attachment(hash).await
    }
}

fn encode_addresses(value: &[MessageAddress]) -> CommandResult<String> {
    serde_json::to_string(value).map_err(json_error)
}

fn decode_addresses(value: String) -> CommandResult<Vec<MessageAddress>> {
    serde_json::from_str(&value).map_err(json_error)
}

fn draft_status(value: String) -> DraftStatus {
    match value.as_str() {
        "queued" => DraftStatus::Queued,
        "sent" => DraftStatus::Sent,
        _ => DraftStatus::Editing,
    }
}

fn send_status(value: String) -> SendJobStatus {
    match value.as_str() {
        "sending" => SendJobStatus::Sending,
        "sent" => SendJobStatus::Sent,
        "failed" => SendJobStatus::Failed,
        _ => SendJobStatus::Queued,
    }
}

fn read_error(error: sqlx::Error) -> CommandError {
    crate::diagnostics::command_error("storage.read_failed", false, &error)
}

fn json_error(error: serde_json::Error) -> CommandError {
    crate::diagnostics::command_error("storage.json_failed", false, &error)
}

#[cfg(test)]
mod tests;

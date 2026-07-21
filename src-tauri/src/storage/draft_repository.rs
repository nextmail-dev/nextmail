use crate::core::{
    CommandError, CommandResult, ComposedMessageActionDraft, DraftAttachmentSummary, DraftContent,
    DraftDetail, DraftListItem, DraftRecipientFields, DraftStatus, ImportedDraftSource,
    MessageActionSource, MessageAddress, MessageComposeAction, SendJobStatus, SendJobSummary,
};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{repository::now, sanitize_attachment_file_name, ContentStore};

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
        .map_err(|_| CommandError::new("draft.list_failed"))?;
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
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, subject, editor_json, html, plain_text, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(subject)
        .bind(&content.editor_json)
        .bind(&content.html)
        .bind(&content.plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.create_failed"))?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn message_action_source(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<MessageActionSource> {
        let message = sqlx::query(
            "SELECT subject, from_json, to_json, cc_json, message_id, references_json \
             FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.create_from_message_failed"))?
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
        .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;
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
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, related_message_id, in_reply_to, references_json, \
             to_json, cc_json, subject, editor_json, html, plain_text, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;

        if action == MessageComposeAction::Forward {
            let attachments = sqlx::query(
                "SELECT file_name, content_type, size, content_hash FROM attachments \
                 WHERE message_id = ? AND content_hash IS NOT NULL AND content_id IS NULL ORDER BY part_index",
            )
            .bind(message_id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;
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
                .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;
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
        .map_err(|_| CommandError::new("draft.read_failed"))?
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
        .map_err(|_| CommandError::new("draft.import_failed"))?
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
        .map_err(|_| CommandError::new("draft.import_failed"))?
        .ok_or_else(|| CommandError::new("message.not_found"))?;
        let body =
            sqlx::query("SELECT plain_text, safe_html FROM message_bodies WHERE message_id = ?")
                .bind(message_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| CommandError::new("draft.import_failed"))?;
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
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("draft.import_failed"))?;
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
        .map_err(|_| CommandError::new("draft.import_failed"))?;
        let attachments = sqlx::query(
            "SELECT file_name, content_type, size, content_hash, content_id FROM attachments \
             WHERE message_id = ? AND content_hash IS NOT NULL ORDER BY part_index",
        )
        .bind(message_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("draft.import_failed"))?;
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
            .map_err(|_| CommandError::new("draft.import_failed"))?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("draft.import_failed"))?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn discard_empty_draft(&self, account_slot_id: &str, draft_id: &str) {
        let _ = sqlx::query(
            "DELETE FROM drafts WHERE id = ? AND account_slot_id = ? AND status = 'editing' \
             AND subject = '' AND to_json = '[]' AND cc_json = '[]' AND bcc_json = '[]' \
             AND plain_text = '' AND (html = '' OR html = '<p></p>') \
             AND NOT EXISTS(SELECT 1 FROM draft_attachments WHERE draft_id = drafts.id)",
        )
        .bind(draft_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await;
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
        .map_err(|_| CommandError::new("draft.read_failed"))?
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
        .map_err(|_| CommandError::new("draft.delete_failed"))?;
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
        .map_err(|_| CommandError::new("draft.read_failed"))?
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
             html = ?, plain_text = ?, revision = revision + 1, updated_at = ? \
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
        .map_err(|_| CommandError::new("draft.save_failed"))?;
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
        .map_err(|_| CommandError::new("draft.read_failed"))?;
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
        .map_err(|_| CommandError::new("draft.attachment_write_failed"))?;
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
        .map_err(|_| CommandError::new("draft.attachment_write_failed"))?;
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
        .map_err(|_| CommandError::new("draft.read_failed"))?;
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
        .map_err(|_| CommandError::new("draft.attachment_write_failed"))?;
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
        .bind(account_slot_id)
        .bind(draft_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.attachment_remove_failed"))?;
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
        .map_err(|_| CommandError::new("draft.attachment_read_failed"))?;
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

impl SendJobRepository {
    pub async fn queue_send_job(
        &self,
        account_id: &str,
        account_slot_id: &str,
        draft_id: &str,
        mime_hash: &str,
        envelope_recipients: &[String],
    ) -> CommandResult<SendJobSummary> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("send.queue_failed"))?;
        let timestamp = now();
        let job_id = Uuid::new_v4().to_string();
        let draft_result = sqlx::query(
            "UPDATE drafts SET status = 'queued', revision = revision + 1, updated_at = ? \
             WHERE id = ? AND account_slot_id = ? AND status = 'editing'",
        )
        .bind(timestamp)
        .bind(draft_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.queue_failed"))?;
        if draft_result.rows_affected() != 1 {
            return Err(CommandError::new("draft.not_editable"));
        }
        sqlx::query(
            "INSERT INTO send_jobs(id, draft_id, account_slot_id, mime_hash, envelope_recipients_json, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job_id)
        .bind(draft_id)
        .bind(account_slot_id)
        .bind(mime_hash)
        .bind(serde_json::to_string(envelope_recipients).map_err(json_error)?)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.queue_failed"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("send.queue_failed"))?;
        self.get_send_job(account_id, account_slot_id, &job_id)
            .await
    }

    pub async fn recover_interrupted_send_jobs(&self) -> CommandResult<()> {
        sqlx::query(
            "UPDATE send_jobs SET status = 'queued', error_code = NULL, next_attempt_at = ?, \
             revision = revision + 1, updated_at = ? WHERE status = 'sending'",
        )
        .bind(now())
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.recovery_failed"))?;
        Ok(())
    }

    pub async fn claim_next_send_job(&self) -> CommandResult<Option<ClaimedSendJob>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("send.claim_failed"))?;
        let row = sqlx::query(
            "SELECT id, draft_id, account_slot_id, mime_hash, envelope_recipients_json, attempt_count, revision \
             FROM send_jobs WHERE status = 'queued' AND (next_attempt_at IS NULL OR next_attempt_at <= ?) \
             ORDER BY created_at, rowid LIMIT 1",
        )
        .bind(now())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.claim_failed"))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|_| CommandError::new("send.claim_failed"))?;
            return Ok(None);
        };
        let id: String = row.try_get("id").map_err(read_error)?;
        let revision = row.try_get::<i64, _>("revision").map_err(read_error)? as u64;
        let claimed = sqlx::query(
            "UPDATE send_jobs SET status = 'sending', attempt_count = attempt_count + 1, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND status = 'queued' AND revision = ?",
        )
        .bind(now())
        .bind(&id)
        .bind(revision as i64)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.claim_failed"))?;
        if claimed.rows_affected() != 1 {
            transaction.rollback().await.ok();
            return Ok(None);
        }
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("send.claim_failed"))?;
        Ok(Some(ClaimedSendJob {
            id,
            draft_id: row.try_get("draft_id").map_err(read_error)?,
            account_slot_id: row.try_get("account_slot_id").map_err(read_error)?,
            mime_hash: row.try_get("mime_hash").map_err(read_error)?,
            envelope_recipients: serde_json::from_str(
                &row.try_get::<String, _>("envelope_recipients_json")
                    .map_err(read_error)?,
            )
            .map_err(json_error)?,
            attempt_count: row.try_get::<i64, _>("attempt_count").map_err(read_error)? as u32 + 1,
            revision: revision + 1,
        }))
    }

    pub async fn ready_send_account_slots(&self) -> CommandResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT account_slot_id FROM send_jobs WHERE status = 'queued' \
             AND (next_attempt_at IS NULL OR next_attempt_at <= ?) \
             GROUP BY account_slot_id ORDER BY MIN(created_at), account_slot_id",
        )
        .bind(now())
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.claim_failed"))
    }

    pub async fn claim_next_send_job_for_account(
        &self,
        account_slot_id: &str,
    ) -> CommandResult<Option<ClaimedSendJob>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("send.claim_failed"))?;
        let row = sqlx::query(
            "SELECT id, draft_id, account_slot_id, mime_hash, envelope_recipients_json, attempt_count, revision \
             FROM send_jobs WHERE account_slot_id = ? AND status = 'queued' \
             AND (next_attempt_at IS NULL OR next_attempt_at <= ?) ORDER BY created_at, rowid LIMIT 1",
        )
        .bind(account_slot_id)
        .bind(now())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.claim_failed"))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|_| CommandError::new("send.claim_failed"))?;
            return Ok(None);
        };
        let id: String = row.try_get("id").map_err(read_error)?;
        let revision = row.try_get::<i64, _>("revision").map_err(read_error)? as u64;
        let claimed = sqlx::query(
            "UPDATE send_jobs SET status = 'sending', attempt_count = attempt_count + 1, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND account_slot_id = ? AND status = 'queued' AND revision = ?",
        )
        .bind(now())
        .bind(&id)
        .bind(account_slot_id)
        .bind(revision as i64)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.claim_failed"))?;
        if claimed.rows_affected() != 1 {
            transaction.rollback().await.ok();
            return Ok(None);
        }
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("send.claim_failed"))?;
        Ok(Some(ClaimedSendJob {
            id,
            draft_id: row.try_get("draft_id").map_err(read_error)?,
            account_slot_id: row.try_get("account_slot_id").map_err(read_error)?,
            mime_hash: row.try_get("mime_hash").map_err(read_error)?,
            envelope_recipients: serde_json::from_str(
                &row.try_get::<String, _>("envelope_recipients_json")
                    .map_err(read_error)?,
            )
            .map_err(json_error)?,
            attempt_count: row.try_get::<i64, _>("attempt_count").map_err(read_error)? as u32 + 1,
            revision: revision + 1,
        }))
    }

    pub async fn complete_send_job(&self, job_id: &str) -> CommandResult<()> {
        self.complete_send_job_and_queue_sent(job_id, None).await
    }

    pub async fn complete_send_job_and_queue_sent(
        &self,
        job_id: &str,
        sent_mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        let timestamp = now();
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("send.status_write_failed"))?;
        sqlx::query(
            "UPDATE send_jobs SET status = 'sent', error_code = NULL, sent_at = ?, revision = revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(timestamp)
        .bind(timestamp)
        .bind(job_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("send.status_write_failed"))?;
        sqlx::query(
            "UPDATE drafts SET status = 'sent', revision = revision + 1, updated_at = ? WHERE id = (SELECT draft_id FROM send_jobs WHERE id = ?)",
        )
        .bind(timestamp)
        .bind(job_id)
        .execute(&mut *transaction)
        .await
            .map_err(|_| CommandError::new("send.status_write_failed"))?;
        if let Some(sent_mailbox_id) = sent_mailbox_id {
            let job = sqlx::query("SELECT account_slot_id, mime_hash FROM send_jobs WHERE id = ?")
                .bind(job_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| CommandError::new("send.status_write_failed"))?;
            let account_slot_id: String = job.try_get("account_slot_id").map_err(read_error)?;
            let mime_hash: String = job.try_get("mime_hash").map_err(read_error)?;
            sqlx::query(
                "INSERT INTO pending_operations(id, account_slot_id, kind, destination_mailbox_id, \
                 payload_json, created_at, updated_at) VALUES (?, ?, 'append_sent', ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(account_slot_id)
            .bind(sent_mailbox_id)
            .bind(serde_json::json!({ "mimeHash": mime_hash, "sendJobId": job_id }).to_string())
            .bind(timestamp)
            .bind(timestamp)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CommandError::new("send.status_write_failed"))?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("send.status_write_failed"))?;
        Ok(())
    }

    pub async fn fail_send_job(&self, job_id: &str, code: &str) -> CommandResult<()> {
        sqlx::query(
            "UPDATE send_jobs SET status = 'failed', error_code = ?, next_attempt_at = NULL, revision = revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(code)
        .bind(now())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.status_write_failed"))?;
        Ok(())
    }

    pub async fn defer_send_job(
        &self,
        job_id: &str,
        code: &str,
        next_attempt_at: i64,
    ) -> CommandResult<()> {
        sqlx::query(
            "UPDATE send_jobs SET status = 'queued', error_code = ?, next_attempt_at = ?, revision = revision + 1, updated_at = ? WHERE id = ?",
        )
        .bind(code)
        .bind(next_attempt_at)
        .bind(now())
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.status_write_failed"))?;
        Ok(())
    }

    pub async fn retry_send_job(&self, account_slot_id: &str, job_id: &str) -> CommandResult<()> {
        let result = sqlx::query(
            "UPDATE send_jobs SET status = 'queued', error_code = NULL, next_attempt_at = ?, revision = revision + 1, updated_at = ? \
             WHERE id = ? AND account_slot_id = ? AND status = 'failed'",
        )
        .bind(now())
        .bind(now())
        .bind(job_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.retry_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("send.not_retryable"));
        }
        Ok(())
    }

    pub async fn get_send_job(
        &self,
        account_id: &str,
        account_slot_id: &str,
        job_id: &str,
    ) -> CommandResult<SendJobSummary> {
        let row = sqlx::query(
            "SELECT id, draft_id, status, attempt_count, error_code, revision FROM send_jobs WHERE id = ? AND account_slot_id = ?",
        )
        .bind(job_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("send.read_failed"))?
        .ok_or_else(|| CommandError::new("send.not_found"))?;
        Ok(SendJobSummary {
            id: row.try_get("id").map_err(read_error)?,
            draft_id: row.try_get("draft_id").map_err(read_error)?,
            account_id: account_id.to_owned(),
            status: send_status(row.try_get("status").map_err(read_error)?),
            attempt_count: row.try_get::<i64, _>("attempt_count").map_err(read_error)? as u32,
            error_code: row.try_get("error_code").map_err(read_error)?,
            revision: row.try_get::<i64, _>("revision").map_err(read_error)? as u64,
        })
    }

    pub async fn read_send_mime(&self, hash: &str) -> CommandResult<Vec<u8>> {
        self.content.read_raw(hash).await
    }

    pub async fn write_send_mime(&self, bytes: &[u8]) -> CommandResult<String> {
        self.content.write_raw(bytes).await
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

fn read_error(_: sqlx::Error) -> CommandError {
    CommandError::new("storage.read_failed")
}

fn json_error(_: serde_json::Error) -> CommandError {
    CommandError::new("storage.json_failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        compose_imported_draft, compose_message_action_draft, MessageActionLabels,
    };
    use crate::storage::{create_account_slot, initialize_content_database, MailRepository};

    #[tokio::test]
    async fn imports_a_server_message_as_an_editable_local_draft() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        sqlx::query(
            "INSERT INTO messages(id, account_slot_id, subject, to_json, cc_json, received_at) \
             VALUES ('message', 'slot', 'Imported', '[{\"name\":null,\"email\":\"to@example.com\"}]', '[]', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
             VALUES ('message', 'First paragraph\n\nSecond paragraph', '<p>First paragraph</p><p>Second paragraph</p>', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();

        let source = repository
            .drafts()
            .imported_draft_source("slot", "message")
            .await
            .unwrap();
        let content = compose_imported_draft(&source).unwrap();
        let draft = repository
            .drafts()
            .persist_imported_draft(PersistImportedDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                message_id: "message",
                source: &source,
                content: &content,
            })
            .await
            .unwrap();
        assert_eq!(draft.subject, "Imported");
        assert_eq!(draft.recipients.to[0].email, "to@example.com");
        assert!(draft.content.editor_json.contains("Second paragraph"));
        assert_eq!(draft.status, DraftStatus::Editing);
    }

    #[tokio::test]
    async fn creates_reply_all_with_deduplicated_recipients_and_thread_headers() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        sqlx::query(
            "INSERT INTO messages(id, account_slot_id, subject, from_json, to_json, cc_json, \
             message_id, references_json, received_at) VALUES ( \
             'message', 'slot', 'Topic', \
             '[{\"name\":\"Sender\",\"email\":\"sender@example.com\"}]', \
             '[{\"name\":null,\"email\":\"me@example.com\"},{\"name\":null,\"email\":\"other@example.com\"}]', \
             '[{\"name\":null,\"email\":\"sender@example.com\"}]', \
             'child@example.com', '[\"root@example.com\"]', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
             VALUES ('message', 'Original body', '<p>Original body</p>', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();

        let source = repository
            .drafts()
            .message_action_source("slot", "message")
            .await
            .unwrap();
        let composed = compose_message_action_draft(
            &source,
            "me@example.com",
            MessageComposeAction::ReplyAll,
            MessageActionLabels {
                original_message: "Forwarded message",
                wrote: "wrote:",
                from: "From",
                to: "To",
                subject: "Subject",
            },
        )
        .unwrap();
        let draft = repository
            .drafts()
            .persist_message_action_draft(PersistMessageActionDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                message_id: "message",
                action: MessageComposeAction::ReplyAll,
                draft: &composed,
            })
            .await
            .unwrap();
        assert_eq!(draft.subject, "Re: Topic");
        assert_eq!(draft.recipients.to.len(), 1);
        assert_eq!(draft.recipients.to[0].email, "sender@example.com");
        assert_eq!(draft.recipients.cc.len(), 1);
        assert_eq!(draft.recipients.cc[0].email, "other@example.com");
        assert!(draft.content.plain_text.contains("Original body"));
        assert!(!draft.content.plain_text.contains("> Original body"));
        assert!(draft
            .content
            .editor_json
            .contains("nextmailOriginalMessage"));
        assert!(draft.content.html.contains("<p>Original body</p>"));
        let threading = repository
            .drafts()
            .draft_threading_headers("slot", &draft.id)
            .await
            .unwrap();
        assert_eq!(threading.in_reply_to.as_deref(), Some("child@example.com"));
        assert_eq!(
            threading.references,
            vec!["root@example.com", "child@example.com"]
        );
    }

    #[tokio::test]
    async fn discards_only_completely_empty_drafts() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let empty = repository
            .drafts()
            .create_draft("account", "slot")
            .await
            .unwrap();
        repository
            .drafts()
            .discard_empty_draft("slot", &empty.id)
            .await;
        assert_eq!(
            repository
                .drafts()
                .get_draft("account", "slot", &empty.id)
                .await
                .unwrap_err()
                .code,
            "draft.not_found"
        );

        let retained = repository
            .drafts()
            .create_draft("account", "slot")
            .await
            .unwrap();
        repository
            .drafts()
            .save_draft(SaveDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                draft_id: &retained.id,
                recipients: &DraftRecipientFields::default(),
                subject: "",
                content: &DraftContent {
                    editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"body"}]}]}"#.into(),
                    html: "<p>body</p>".into(),
                    plain_text: "body".into(),
                },
                expected_revision: retained.revision,
            })
            .await
            .unwrap();
        repository
            .drafts()
            .discard_empty_draft("slot", &retained.id)
            .await;
        assert!(repository
            .drafts()
            .get_draft("account", "slot", &retained.id)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn deletes_an_editing_draft_explicitly() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let draft = repository
            .drafts()
            .create_draft("account", "slot")
            .await
            .unwrap();
        repository
            .drafts()
            .delete_editing_draft("slot", &draft.id)
            .await
            .unwrap();
        assert_eq!(
            repository
                .drafts()
                .get_draft("account", "slot", &draft.id)
                .await
                .unwrap_err()
                .code,
            "draft.not_found"
        );
    }

    #[tokio::test]
    async fn draft_and_send_job_survive_repository_reopen() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let draft = repository
            .drafts()
            .create_draft("account", "slot")
            .await
            .unwrap();
        let saved = repository
            .drafts()
            .save_draft(SaveDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                draft_id: &draft.id,
                recipients: &DraftRecipientFields {
                    to: vec![MessageAddress {
                        name: Some("收件人".into()),
                        email: "to@example.com".into(),
                    }],
                    ..Default::default()
                },
                subject: "中文主题",
                content: &DraftContent {
                    editor_json: "{}".into(),
                    html: "<p>正文</p>".into(),
                    plain_text: "正文".into(),
                },
                expected_revision: draft.revision,
            })
            .await
            .unwrap();
        assert_eq!(saved.subject, "中文主题");
        let drafts = repository
            .drafts()
            .list_editing_drafts("account", "slot")
            .await
            .unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, draft.id);
        repository
            .drafts()
            .add_draft_attachment("slot", &draft.id, "报告.txt", "text/plain", b"file")
            .await
            .unwrap();
        let mime_hash = repository
            .send_jobs()
            .write_send_mime(b"From: a@example.com\r\n\r\nbody")
            .await
            .unwrap();
        let job = repository
            .send_jobs()
            .queue_send_job(
                "account",
                "slot",
                &draft.id,
                &mime_hash,
                &["to@example.com".into()],
            )
            .await
            .unwrap();
        assert_eq!(job.status, SendJobStatus::Queued);
        drop(repository);

        let repository = MailRepository::open(directory.path()).await.unwrap();
        let claimed = repository
            .send_jobs()
            .claim_next_send_job()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, job.id);
        repository
            .send_jobs()
            .recover_interrupted_send_jobs()
            .await
            .unwrap();
        let reclaimed = repository
            .send_jobs()
            .claim_next_send_job()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reclaimed.id, job.id);
        repository
            .send_jobs()
            .complete_send_job(&job.id)
            .await
            .unwrap();
        assert_eq!(
            repository
                .send_jobs()
                .get_send_job("account", "slot", &job.id)
                .await
                .unwrap()
                .status,
            SendJobStatus::Sent
        );
    }

    #[tokio::test]
    async fn draft_attachments_are_isolated_by_account_slot() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot-a", 1)
            .await
            .unwrap();
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let draft = repository
            .drafts()
            .create_draft("account-a", "slot-a")
            .await
            .unwrap();
        let attachment = repository
            .drafts()
            .add_draft_attachment("slot-a", &draft.id, "private.txt", "text/plain", b"secret")
            .await
            .unwrap();
        let inline = repository
            .drafts()
            .add_draft_inline_image(
                "slot-a",
                &draft.id,
                "logo.png",
                "image/png",
                Some("logo@example.test"),
                b"image",
            )
            .await
            .unwrap();
        assert!(inline.is_inline);
        assert_eq!(inline.content_id.as_deref(), Some("logo@example.test"));

        assert!(repository
            .drafts()
            .draft_attachments("slot-b", &draft.id)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            repository
                .drafts()
                .remove_draft_attachment("slot-b", &draft.id, &attachment.id)
                .await
                .unwrap_err()
                .code,
            "draft.attachment_not_found"
        );
        assert_eq!(
            repository
                .drafts()
                .draft_attachments("slot-a", &draft.id)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn per_account_send_claims_preserve_fifo_and_do_not_cross_slots() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot-a", 1)
            .await
            .unwrap();
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let draft_a1 = repository
            .drafts()
            .create_draft("account-a", "slot-a")
            .await
            .unwrap();
        let draft_a2 = repository
            .drafts()
            .create_draft("account-a", "slot-a")
            .await
            .unwrap();
        let draft_b = repository
            .drafts()
            .create_draft("account-b", "slot-b")
            .await
            .unwrap();
        let mime_hash = repository
            .send_jobs()
            .write_send_mime(b"From: a@example.com\r\n\r\nbody")
            .await
            .unwrap();
        let job_a1 = repository
            .send_jobs()
            .queue_send_job(
                "account-a",
                "slot-a",
                &draft_a1.id,
                &mime_hash,
                &["to@example.com".to_owned()],
            )
            .await
            .unwrap();
        let job_a2 = repository
            .send_jobs()
            .queue_send_job(
                "account-a",
                "slot-a",
                &draft_a2.id,
                &mime_hash,
                &["to@example.com".to_owned()],
            )
            .await
            .unwrap();
        let job_b = repository
            .send_jobs()
            .queue_send_job(
                "account-b",
                "slot-b",
                &draft_b.id,
                &mime_hash,
                &["to@example.com".to_owned()],
            )
            .await
            .unwrap();

        let slots = repository
            .send_jobs()
            .ready_send_account_slots()
            .await
            .unwrap();
        assert_eq!(slots, vec!["slot-a", "slot-b"]);
        assert_eq!(
            repository
                .send_jobs()
                .claim_next_send_job_for_account("slot-a")
                .await
                .unwrap()
                .unwrap()
                .id,
            job_a1.id
        );
        assert_eq!(
            repository
                .send_jobs()
                .claim_next_send_job_for_account("slot-a")
                .await
                .unwrap()
                .unwrap()
                .id,
            job_a2.id
        );
        assert_eq!(
            repository
                .send_jobs()
                .claim_next_send_job_for_account("slot-b")
                .await
                .unwrap()
                .unwrap()
                .id,
            job_b.id
        );
        assert!(repository
            .send_jobs()
            .claim_next_send_job_for_account("slot-b")
            .await
            .unwrap()
            .is_none());
    }
}

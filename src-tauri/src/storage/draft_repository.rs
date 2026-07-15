use crate::core::{
    CommandError, CommandResult, DraftAttachmentSummary, DraftContent, DraftDetail, DraftListItem,
    DraftRecipientFields, DraftStatus, MessageAddress, MessageComposeAction, SendJobStatus,
    SendJobSummary,
};
use sqlx::Row;
use uuid::Uuid;

use super::{repository::now, MailRepository};

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

pub struct CreateMessageActionDraftRequest<'a> {
    pub account_id: &'a str,
    pub account_slot_id: &'a str,
    pub own_email: &'a str,
    pub message_id: &'a str,
    pub action: MessageComposeAction,
    pub original_message_label: &'a str,
    pub wrote_label: &'a str,
    pub from_label: &'a str,
    pub to_label: &'a str,
    pub subject_label: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DraftThreadingHeaders {
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

impl MailRepository {
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
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO drafts(id, account_slot_id, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.create_failed"))?;
        self.get_draft(account_id, account_slot_id, &id).await
    }

    pub async fn create_message_action_draft(
        &self,
        request: CreateMessageActionDraftRequest<'_>,
    ) -> CommandResult<DraftDetail> {
        let CreateMessageActionDraftRequest {
            account_id,
            account_slot_id,
            own_email,
            message_id,
            action,
            original_message_label,
            wrote_label,
            from_label,
            to_label,
            subject_label,
        } = request;
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
        let from = decode_addresses(message.try_get("from_json").map_err(read_error)?)?;
        let to = decode_addresses(message.try_get("to_json").map_err(read_error)?)?;
        let cc = decode_addresses(message.try_get("cc_json").map_err(read_error)?)?;
        let original_subject: String = message.try_get("subject").map_err(read_error)?;
        let header_message_id: Option<String> =
            message.try_get("message_id").map_err(read_error)?;
        let mut references: Vec<String> = serde_json::from_str(
            &message
                .try_get::<String, _>("references_json")
                .map_err(read_error)?,
        )
        .map_err(json_error)?;
        if let Some(value) = header_message_id.as_ref() {
            if !references.iter().any(|current| current == value) {
                references.push(value.clone());
            }
        }
        let body = sqlx::query_scalar::<_, Option<String>>(
            "SELECT plain_text FROM message_bodies WHERE message_id = ?",
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.create_from_message_failed"))?
        .flatten()
        .unwrap_or_default();

        let mut recipients = DraftRecipientFields::default();
        match action {
            MessageComposeAction::Reply => {
                recipients.to = reply_recipients(&from, &to, own_email);
            }
            MessageComposeAction::ReplyAll => {
                recipients.to = reply_recipients(&from, &to, own_email);
                recipients.cc = unique_addresses(
                    to.iter().cloned().chain(cc.iter().cloned()).collect(),
                    own_email,
                    &recipients.to,
                );
            }
            MessageComposeAction::Forward => {}
        }

        let sender = format_addresses(&from);
        let plain_text = match action {
            MessageComposeAction::Reply | MessageComposeAction::ReplyAll => {
                let quoted = body
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("\n\n{sender} {wrote_label}\n{quoted}")
            }
            MessageComposeAction::Forward => format!(
                "\n\n---------- {original_message_label} ----------\n{from_label}: {sender}\n{to_label}: {}\n{subject_label}: {}\n\n{}",
                format_addresses(&to),
                original_subject,
                body,
            ),
        };
        let subject = prefixed_subject(&original_subject, action);
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
        .bind(match action {
            MessageComposeAction::Forward => None,
            _ => header_message_id,
        })
        .bind(serde_json::to_string(&references).map_err(json_error)?)
        .bind(encode_addresses(&recipients.to)?)
        .bind(encode_addresses(&recipients.cc)?)
        .bind(subject)
        .bind(editor_document_from_text(&plain_text)?)
        .bind(format!("<p>{}</p>", escape_html(&plain_text).replace('\n', "<br>")))
        .bind(plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("draft.create_from_message_failed"))?;

        if action == MessageComposeAction::Forward {
            let attachments = sqlx::query(
                "SELECT file_name, content_type, size, content_hash FROM attachments \
                 WHERE message_id = ? AND content_hash IS NOT NULL ORDER BY part_index",
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

    pub async fn import_message_as_draft(
        &self,
        account_id: &str,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<DraftDetail> {
        if let Some(existing) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM drafts WHERE account_slot_id = ? AND source_message_id = ? AND status = 'editing'",
        )
        .bind(account_slot_id)
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("draft.import_failed"))?
        {
            return self.get_draft(account_id, account_slot_id, &existing).await;
        }
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
        let html = body
            .as_ref()
            .and_then(|row| row.try_get::<Option<String>, _>("safe_html").ok())
            .flatten()
            .unwrap_or_else(|| format!("<p>{}</p>", escape_html(&plain_text)));
        let editor_json = editor_document_from_text(&plain_text)?;
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
        .bind(message.try_get::<String, _>("to_json").map_err(read_error)?)
        .bind(message.try_get::<String, _>("cc_json").map_err(read_error)?)
        .bind(message.try_get::<String, _>("subject").map_err(read_error)?)
        .bind(editor_json)
        .bind(html)
        .bind(plain_text)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("draft.import_failed"))?;
        let attachments = sqlx::query(
            "SELECT file_name, content_type, size, content_hash FROM attachments \
             WHERE message_id = ? AND content_hash IS NOT NULL ORDER BY part_index",
        )
        .bind(message_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("draft.import_failed"))?;
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
        let attachments = self.draft_attachments(draft_id).await?;
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
        draft_id: &str,
        file_name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> CommandResult<DraftAttachmentSummary> {
        let editable = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM drafts WHERE id = ? AND status = 'editing'",
        )
        .bind(draft_id)
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
        })
    }

    pub async fn remove_draft_attachment(
        &self,
        draft_id: &str,
        attachment_id: &str,
    ) -> CommandResult<()> {
        let result = sqlx::query(
            "DELETE FROM draft_attachments WHERE id = ? AND draft_id = ? \
             AND EXISTS(SELECT 1 FROM drafts WHERE id = ? AND status = 'editing')",
        )
        .bind(attachment_id)
        .bind(draft_id)
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
        draft_id: &str,
    ) -> CommandResult<Vec<StoredDraftAttachment>> {
        let rows = sqlx::query(
            "SELECT id, file_name, content_type, size, content_hash FROM draft_attachments \
             WHERE draft_id = ? ORDER BY sort_order, id",
        )
        .bind(draft_id)
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
                    },
                    content_hash: row.try_get("content_hash").map_err(read_error)?,
                })
            })
            .collect()
    }

    pub async fn attachment_bytes(&self, hash: &str) -> CommandResult<Vec<u8>> {
        self.content.read_attachment(hash).await
    }

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
             ORDER BY created_at LIMIT 1",
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

    pub async fn retry_send_job(&self, job_id: &str) -> CommandResult<()> {
        let result = sqlx::query(
            "UPDATE send_jobs SET status = 'queued', error_code = NULL, next_attempt_at = ?, revision = revision + 1, updated_at = ? WHERE id = ? AND status = 'failed'",
        )
        .bind(now())
        .bind(now())
        .bind(job_id)
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
        .collect::<std::collections::HashSet<_>>();
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

fn editor_document_from_text(value: &str) -> CommandResult<String> {
    let content = value
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
                serde_json::json!({
                    "type": "paragraph",
                    "content": lines
                })
            }
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({ "type": "doc", "content": content }))
        .map_err(json_error)
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
    use crate::storage::{create_account_slot, initialize_content_database};

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

        let draft = repository
            .import_message_as_draft("account", "slot", "message")
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

        let draft = repository
            .create_message_action_draft(CreateMessageActionDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                own_email: "me@example.com",
                message_id: "message",
                action: MessageComposeAction::ReplyAll,
                original_message_label: "Forwarded message",
                wrote_label: "wrote:",
                from_label: "From",
                to_label: "To",
                subject_label: "Subject",
            })
            .await
            .unwrap();
        assert_eq!(draft.subject, "Re: Topic");
        assert_eq!(draft.recipients.to.len(), 1);
        assert_eq!(draft.recipients.to[0].email, "sender@example.com");
        assert_eq!(draft.recipients.cc.len(), 1);
        assert_eq!(draft.recipients.cc[0].email, "other@example.com");
        assert!(draft.content.plain_text.contains("> Original body"));
        assert!(draft.content.editor_json.contains("hardBreak"));
        let threading = repository
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
        let empty = repository.create_draft("account", "slot").await.unwrap();
        repository.discard_empty_draft("slot", &empty.id).await;
        assert_eq!(
            repository
                .get_draft("account", "slot", &empty.id)
                .await
                .unwrap_err()
                .code,
            "draft.not_found"
        );

        let retained = repository.create_draft("account", "slot").await.unwrap();
        repository
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
        repository.discard_empty_draft("slot", &retained.id).await;
        assert!(repository
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
        let draft = repository.create_draft("account", "slot").await.unwrap();
        repository
            .delete_editing_draft("slot", &draft.id)
            .await
            .unwrap();
        assert_eq!(
            repository
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
        let draft = repository.create_draft("account", "slot").await.unwrap();
        let saved = repository
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
            .list_editing_drafts("account", "slot")
            .await
            .unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, draft.id);
        repository
            .add_draft_attachment(&draft.id, "报告.txt", "text/plain", b"file")
            .await
            .unwrap();
        let mime_hash = repository
            .write_send_mime(b"From: a@example.com\r\n\r\nbody")
            .await
            .unwrap();
        let job = repository
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
        let claimed = repository.claim_next_send_job().await.unwrap().unwrap();
        assert_eq!(claimed.id, job.id);
        repository.recover_interrupted_send_jobs().await.unwrap();
        let reclaimed = repository.claim_next_send_job().await.unwrap().unwrap();
        assert_eq!(reclaimed.id, job.id);
        repository.complete_send_job(&job.id).await.unwrap();
        assert_eq!(
            repository
                .get_send_job("account", "slot", &job.id)
                .await
                .unwrap()
                .status,
            SendJobStatus::Sent
        );
    }
}

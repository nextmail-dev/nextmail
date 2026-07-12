use nextmail_core::{
    CommandError, CommandResult, DraftAttachmentSummary, DraftContent, DraftDetail, DraftListItem,
    DraftRecipientFields, DraftStatus, MessageAddress, SendJobStatus, SendJobSummary,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{repository::now, MailRepository};

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
    use crate::{create_account_slot, initialize_content_database};

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

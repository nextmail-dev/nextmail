use super::{json_error, now, read_error, send_status, ClaimedSendJob, SendJobRepository};
use crate::core::{CommandError, CommandResult, SendJobSummary};
use sqlx::Row;
use uuid::Uuid;
impl SendJobRepository {
    pub async fn queue_send_job(
        &self,
        account_id: &str,
        account_slot_id: &str,
        draft_id: &str,
        mime_hash: &str,
        envelope_recipients: &[String],
    ) -> CommandResult<SendJobSummary> {
        let mut transaction = super::super::begin_write(&self.pool)
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
        let mut transaction = super::super::begin_write(&self.pool)
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
        let mut transaction = super::super::begin_write(&self.pool)
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
        let mut transaction = super::super::begin_write(&self.pool)
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

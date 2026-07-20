use crate::core::{
    CommandError, CommandResult, PendingOperationKind, PendingOperationStatus,
    PendingOperationSummary,
};
use serde_json::{json, Value};
use sqlx::{FromRow, Row, SqlitePool};
use uuid::Uuid;

use super::now;

#[derive(Clone)]
pub struct OperationRepository {
    pub(crate) pool: SqlitePool,
}

#[derive(Clone, Debug)]
pub struct PendingOperationWork {
    pub id: String,
    pub kind: PendingOperationKind,
    pub message_id: Option<String>,
    pub source_mailbox_id: Option<String>,
    pub source_mailbox_name: Option<String>,
    pub destination_mailbox_name: Option<String>,
    pub uid: Option<u32>,
    pub uid_validity: Option<u32>,
    pub base_modseq: Option<u64>,
    pub payload: Value,
    pub attempt_count: u32,
}

struct NewOperation<'a> {
    id: &'a str,
    account_slot_id: &'a str,
    kind: &'a PendingOperationKind,
    message_id: Option<&'a str>,
    source_mailbox_id: Option<&'a str>,
    destination_mailbox_id: Option<&'a str>,
    uid: Option<i64>,
    uid_validity: Option<i64>,
    base_modseq: Option<i64>,
    payload: &'a Value,
}

#[derive(FromRow)]
struct FlagOperationRow {
    uid: i64,
    uid_validity: i64,
    modseq: Option<i64>,
    unread: i64,
    flagged: i64,
}

#[derive(FromRow)]
struct PendingOperationRow {
    id: String,
    kind: String,
    message_id: Option<String>,
    source_mailbox_id: Option<String>,
    source_name: Option<String>,
    destination_name: Option<String>,
    uid: Option<i64>,
    uid_validity: Option<i64>,
    base_modseq: Option<i64>,
    payload_json: String,
    attempt_count: i64,
}

impl OperationRepository {
    pub async fn queue_set_read(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        read: bool,
    ) -> CommandResult<Vec<String>> {
        self.queue_flag_operations(
            account_slot_id,
            mailbox_id,
            message_ids,
            PendingOperationKind::SetRead,
            read,
        )
        .await
    }

    pub async fn queue_set_flagged(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        flagged: bool,
    ) -> CommandResult<Vec<String>> {
        self.queue_flag_operations(
            account_slot_id,
            mailbox_id,
            message_ids,
            PendingOperationKind::SetFlagged,
            flagged,
        )
        .await
    }

    async fn queue_flag_operations(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        kind: PendingOperationKind,
        value: bool,
    ) -> CommandResult<Vec<String>> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        let mut ids = Vec::with_capacity(message_ids.len());
        for message_id in message_ids {
            let id = queue_flag_operation(
                &mut transaction,
                account_slot_id,
                mailbox_id,
                message_id,
                &kind,
                value,
            )
            .await?;
            ids.push(id);
        }
        refresh_mailbox_counts(&mut transaction, mailbox_id).await?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        Ok(ids)
    }

    pub async fn queue_transfer(
        &self,
        account_slot_id: &str,
        source_mailbox_id: &str,
        destination_mailbox_id: &str,
        message_ids: &[String],
        copy: bool,
    ) -> CommandResult<Vec<String>> {
        if source_mailbox_id == destination_mailbox_id {
            return Err(CommandError::new("operation.same_mailbox"));
        }
        self.ensure_mailbox(account_slot_id, destination_mailbox_id)
            .await?;
        let kind = if copy {
            PendingOperationKind::Copy
        } else {
            PendingOperationKind::Move
        };
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        let mut ids = Vec::with_capacity(message_ids.len());
        for message_id in message_ids {
            let row = operation_location(
                &mut transaction,
                account_slot_id,
                source_mailbox_id,
                message_id,
            )
            .await?;
            if !copy {
                sqlx::query("UPDATE message_locations SET local_hidden = 1 WHERE message_id = ? AND mailbox_id = ?")
                    .bind(message_id)
                    .bind(source_mailbox_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|_| CommandError::new("operation.queue_failed"))?;
            }
            let id = Uuid::new_v4().to_string();
            let payload = json!({});
            insert_operation(
                &mut transaction,
                NewOperation {
                    id: &id,
                    account_slot_id,
                    kind: &kind,
                    message_id: Some(message_id),
                    source_mailbox_id: Some(source_mailbox_id),
                    destination_mailbox_id: Some(destination_mailbox_id),
                    uid: Some(row.try_get::<i64, _>("uid").unwrap_or_default()),
                    uid_validity: Some(row.try_get::<i64, _>("uid_validity").unwrap_or_default()),
                    base_modseq: row.try_get::<Option<i64>, _>("modseq").unwrap_or_default(),
                    payload: &payload,
                },
            )
            .await?;
            ids.push(id);
        }
        refresh_mailbox_counts(&mut transaction, source_mailbox_id).await?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        Ok(ids)
    }

    pub async fn queue_permanent_delete(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
    ) -> CommandResult<Vec<String>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        let mut ids = Vec::with_capacity(message_ids.len());
        for message_id in message_ids {
            let row = operation_location(&mut transaction, account_slot_id, mailbox_id, message_id)
                .await?;
            sqlx::query("UPDATE message_locations SET local_hidden = 1 WHERE message_id = ? AND mailbox_id = ?")
                .bind(message_id)
                .bind(mailbox_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| CommandError::new("operation.queue_failed"))?;
            let id = Uuid::new_v4().to_string();
            let payload = json!({});
            insert_operation(
                &mut transaction,
                NewOperation {
                    id: &id,
                    account_slot_id,
                    kind: &PendingOperationKind::Delete,
                    message_id: Some(message_id),
                    source_mailbox_id: Some(mailbox_id),
                    destination_mailbox_id: None,
                    uid: Some(row.try_get::<i64, _>("uid").unwrap_or_default()),
                    uid_validity: Some(row.try_get::<i64, _>("uid_validity").unwrap_or_default()),
                    base_modseq: row.try_get::<Option<i64>, _>("modseq").unwrap_or_default(),
                    payload: &payload,
                },
            )
            .await?;
            ids.push(id);
        }
        refresh_mailbox_counts(&mut transaction, mailbox_id).await?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        Ok(ids)
    }

    pub async fn queue_draft_append(
        &self,
        account_slot_id: &str,
        drafts_mailbox_id: &str,
        draft_id: &str,
        mime_hash: &str,
        revision: u64,
    ) -> CommandResult<()> {
        self.ensure_mailbox(account_slot_id, drafts_mailbox_id)
            .await?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        sqlx::query(
            "DELETE FROM pending_operations WHERE account_slot_id = ? AND kind = 'append_draft' \
             AND json_extract(payload_json, '$.draftId') = ? AND status IN ('queued','retry_wait','failed')",
        )
        .bind(account_slot_id)
        .bind(draft_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("operation.queue_failed"))?;
        let id = Uuid::new_v4().to_string();
        let payload = json!({ "mimeHash": mime_hash, "draftId": draft_id, "revision": revision });
        insert_operation(
            &mut transaction,
            NewOperation {
                id: &id,
                account_slot_id,
                kind: &PendingOperationKind::AppendDraft,
                message_id: None,
                source_mailbox_id: None,
                destination_mailbox_id: Some(drafts_mailbox_id),
                uid: None,
                uid_validity: None,
                base_modseq: None,
                payload: &payload,
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        Ok(())
    }
}

impl OperationRepository {
    pub async fn recover_pending_operations(&self) -> CommandResult<()> {
        sqlx::query(
            "UPDATE pending_operations SET status = 'queued', updated_at = ? WHERE status = 'running'",
        )
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("operation.recovery_failed"))?;
        Ok(())
    }

    pub async fn claim_pending_operation(
        &self,
        account_slot_id: &str,
    ) -> CommandResult<Option<PendingOperationWork>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.claim_failed"))?;
        let row: Option<PendingOperationRow> = sqlx::query_as(
            "SELECT o.id, o.kind, o.message_id, o.source_mailbox_id, sb.remote_name AS source_name, \
                    db.remote_name AS destination_name, o.uid, o.uid_validity, o.base_modseq, \
                    o.payload_json, o.attempt_count \
             FROM pending_operations o \
             LEFT JOIN mailboxes sb ON sb.id = o.source_mailbox_id \
             LEFT JOIN mailboxes db ON db.id = o.destination_mailbox_id \
             WHERE o.account_slot_id = ? AND \
               (o.status = 'queued' OR (o.status = 'retry_wait' AND COALESCE(o.next_attempt_at, 0) <= ?)) \
             ORDER BY o.created_at, o.id LIMIT 1",
        )
        .bind(account_slot_id)
        .bind(now())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("operation.claim_failed"))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|_| CommandError::new("operation.claim_failed"))?;
            return Ok(None);
        };
        if !mark_operation_running(&mut transaction, &row.id).await? {
            transaction
                .rollback()
                .await
                .map_err(|_| CommandError::new("operation.claim_failed"))?;
            return Ok(None);
        }
        let work = pending_operation_work(row)?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.claim_failed"))?;
        Ok(Some(work))
    }

    pub async fn complete_pending_operation(
        &self,
        work: &PendingOperationWork,
        cleanup_pending: bool,
    ) -> CommandResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.complete_failed"))?;
        if matches!(
            work.kind,
            PendingOperationKind::Move | PendingOperationKind::Delete
        ) {
            if let (Some(message_id), Some(mailbox_id)) = (
                work.message_id.as_deref(),
                work.source_mailbox_id.as_deref(),
            ) {
                sqlx::query(
                    "DELETE FROM message_locations WHERE message_id = ? AND mailbox_id = ?",
                )
                .bind(message_id)
                .bind(mailbox_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| CommandError::new("operation.complete_failed"))?;
                refresh_mailbox_counts(&mut transaction, mailbox_id).await?;
            }
        }
        sqlx::query(
            "UPDATE pending_operations SET status = 'succeeded', cleanup_pending = ?, \
             error_code = NULL, next_attempt_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(i64::from(cleanup_pending))
        .bind(now())
        .bind(&work.id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("operation.complete_failed"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.complete_failed"))?;
        Ok(())
    }

    pub async fn fail_pending_operation(
        &self,
        work: &PendingOperationWork,
        error_code: &str,
        retryable: bool,
    ) -> CommandResult<()> {
        let retry = retryable && work.attempt_count < 8;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::new("operation.fail_failed"))?;
        if !retry {
            self.rollback_projection(&mut transaction, work).await?;
        }
        let delay = (2_i64.pow(work.attempt_count.min(7))).min(300);
        sqlx::query(
            "UPDATE pending_operations SET status = ?, next_attempt_at = ?, error_code = ?, updated_at = ? WHERE id = ?",
        )
        .bind(if retry { "retry_wait" } else { "failed" })
        .bind(if retry { Some(now() + delay) } else { None })
        .bind(error_code)
        .bind(now())
        .bind(&work.id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("operation.fail_failed"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("operation.fail_failed"))?;
        Ok(())
    }

    async fn rollback_projection(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        work: &PendingOperationWork,
    ) -> CommandResult<()> {
        let (Some(message_id), Some(mailbox_id)) = (
            work.message_id.as_deref(),
            work.source_mailbox_id.as_deref(),
        ) else {
            return Ok(());
        };
        match work.kind {
            PendingOperationKind::SetRead => {
                let previous = work.payload["previous"].as_bool().unwrap_or(false);
                sqlx::query("UPDATE message_locations SET unread = ? WHERE message_id = ? AND mailbox_id = ?")
                    .bind(i64::from(!previous))
                    .bind(message_id)
                    .bind(mailbox_id)
                    .execute(&mut **transaction)
                    .await
                    .map_err(|_| CommandError::new("operation.fail_failed"))?;
            }
            PendingOperationKind::SetFlagged => {
                let previous = work.payload["previous"].as_bool().unwrap_or(false);
                sqlx::query("UPDATE message_locations SET flagged = ? WHERE message_id = ? AND mailbox_id = ?")
                    .bind(i64::from(previous))
                    .bind(message_id)
                    .bind(mailbox_id)
                    .execute(&mut **transaction)
                    .await
                    .map_err(|_| CommandError::new("operation.fail_failed"))?;
            }
            PendingOperationKind::Move | PendingOperationKind::Delete => {
                sqlx::query("UPDATE message_locations SET local_hidden = 0 WHERE message_id = ? AND mailbox_id = ?")
                    .bind(message_id)
                    .bind(mailbox_id)
                    .execute(&mut **transaction)
                    .await
                    .map_err(|_| CommandError::new("operation.fail_failed"))?;
            }
            _ => {}
        }
        refresh_mailbox_counts(transaction, mailbox_id).await
    }

    pub async fn list_pending_operation_status(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<Vec<PendingOperationSummary>> {
        let rows = sqlx::query(
            "SELECT id, message_id, kind, status, attempt_count, error_code, cleanup_pending \
             FROM pending_operations WHERE account_slot_id = ? AND (status != 'succeeded' OR cleanup_pending = 1) \
             ORDER BY created_at DESC LIMIT 100",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("operation.status_read_failed"))?;
        rows.into_iter()
            .map(|row| {
                Ok(PendingOperationSummary {
                    id: row.try_get("id").map_err(storage_read_error)?,
                    account_id: account_id.to_owned(),
                    message_id: row.try_get("message_id").map_err(storage_read_error)?,
                    kind: operation_kind_from_db(row.try_get("kind").map_err(storage_read_error)?),
                    status: operation_status_from_db(
                        row.try_get("status").map_err(storage_read_error)?,
                    ),
                    attempt_count: row
                        .try_get::<i64, _>("attempt_count")
                        .map_err(storage_read_error)? as u32,
                    error_code: row.try_get("error_code").map_err(storage_read_error)?,
                    cleanup_pending: row
                        .try_get::<i64, _>("cleanup_pending")
                        .map_err(storage_read_error)?
                        != 0,
                })
            })
            .collect()
    }

    pub async fn retry_pending_operation(
        &self,
        account_slot_id: &str,
        operation_id: &str,
    ) -> CommandResult<()> {
        let changed = sqlx::query(
            "UPDATE pending_operations SET status = 'queued', error_code = NULL, next_attempt_at = NULL, \
             updated_at = ? WHERE id = ? AND account_slot_id = ? AND status IN ('failed','needs_reconcile')",
        )
        .bind(now())
        .bind(operation_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("operation.retry_failed"))?;
        if changed.rows_affected() == 1 {
            Ok(())
        } else {
            Err(CommandError::new("operation.not_retryable"))
        }
    }

    async fn ensure_mailbox(&self, account_slot_id: &str, mailbox_id: &str) -> CommandResult<()> {
        ensure_mailbox(&self.pool, account_slot_id, mailbox_id).await
    }
}

async fn ensure_mailbox(
    pool: &SqlitePool,
    account_slot_id: &str,
    mailbox_id: &str,
) -> CommandResult<()> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mailboxes WHERE id = ? AND account_slot_id = ? AND selectable = 1",
    )
    .bind(mailbox_id)
    .bind(account_slot_id)
    .fetch_one(pool)
    .await
    .map_err(|_| CommandError::new("storage.mailboxes_read_failed"))?;
    if exists == 1 {
        Ok(())
    } else {
        Err(CommandError::new("mailbox.not_found"))
    }
}

async fn mark_operation_running(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_id: &str,
) -> CommandResult<bool> {
    let changed = sqlx::query(
        "UPDATE pending_operations SET status = 'running', attempt_count = attempt_count + 1, \
         updated_at = ? WHERE id = ? AND status IN ('queued', 'retry_wait')",
    )
    .bind(now())
    .bind(operation_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| CommandError::new("operation.claim_failed"))?;
    Ok(changed.rows_affected() == 1)
}

fn pending_operation_work(row: PendingOperationRow) -> CommandResult<PendingOperationWork> {
    Ok(PendingOperationWork {
        id: row.id,
        kind: operation_kind_from_db(row.kind),
        message_id: row.message_id,
        source_mailbox_id: row.source_mailbox_id,
        source_mailbox_name: row.source_name,
        destination_mailbox_name: row.destination_name,
        uid: row.uid.map(|value| value as u32),
        uid_validity: row.uid_validity.map(|value| value as u32),
        base_modseq: row.base_modseq.map(|value| value as u64),
        payload: serde_json::from_str(&row.payload_json)
            .map_err(|_| CommandError::new("storage.json_decode_failed"))?,
        attempt_count: row.attempt_count as u32 + 1,
    })
}

async fn queue_flag_operation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    account_slot_id: &str,
    mailbox_id: &str,
    message_id: &str,
    kind: &PendingOperationKind,
    value: bool,
) -> CommandResult<String> {
    let row = flag_operation_row(transaction, account_slot_id, mailbox_id, message_id).await?;
    apply_local_flag_value(transaction, mailbox_id, message_id, kind, value).await?;

    let id = Uuid::new_v4().to_string();
    let payload = json!({
        "value": value,
        "previous": previous_flag_value(&row, kind),
    });
    insert_operation(
        transaction,
        NewOperation {
            id: &id,
            account_slot_id,
            kind,
            message_id: Some(message_id),
            source_mailbox_id: Some(mailbox_id),
            destination_mailbox_id: None,
            uid: Some(row.uid),
            uid_validity: Some(row.uid_validity),
            base_modseq: row.modseq,
            payload: &payload,
        },
    )
    .await?;
    Ok(id)
}

async fn flag_operation_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    account_slot_id: &str,
    mailbox_id: &str,
    message_id: &str,
) -> CommandResult<FlagOperationRow> {
    sqlx::query_as(
        "SELECT l.uid, l.uid_validity, l.modseq, l.unread, l.flagged \
         FROM message_locations l JOIN mailboxes b ON b.id = l.mailbox_id \
         JOIN messages m ON m.id = l.message_id \
         WHERE l.message_id = ? AND l.mailbox_id = ? AND b.account_slot_id = ? \
         AND m.account_slot_id = ? AND l.local_hidden = 0",
    )
    .bind(message_id)
    .bind(mailbox_id)
    .bind(account_slot_id)
    .bind(account_slot_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| CommandError::new("operation.queue_failed"))?
    .ok_or_else(|| CommandError::new("message.remote_location_missing"))
}

fn previous_flag_value(row: &FlagOperationRow, kind: &PendingOperationKind) -> bool {
    match kind {
        PendingOperationKind::SetRead => row.unread == 0,
        PendingOperationKind::SetFlagged => row.flagged != 0,
        _ => unreachable!("only flag operations use this helper"),
    }
}

async fn apply_local_flag_value(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mailbox_id: &str,
    message_id: &str,
    kind: &PendingOperationKind,
    value: bool,
) -> CommandResult<()> {
    match kind {
        PendingOperationKind::SetRead => {
            sqlx::query(
                "UPDATE message_locations SET unread = ? WHERE message_id = ? AND mailbox_id = ?",
            )
            .bind(i64::from(!value))
            .bind(message_id)
            .bind(mailbox_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        }
        PendingOperationKind::SetFlagged => {
            sqlx::query(
                "UPDATE message_locations SET flagged = ? WHERE message_id = ? AND mailbox_id = ?",
            )
            .bind(i64::from(value))
            .bind(message_id)
            .bind(mailbox_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| CommandError::new("operation.queue_failed"))?;
        }
        _ => unreachable!("only flag operations use this helper"),
    }
    Ok(())
}

async fn insert_operation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation: NewOperation<'_>,
) -> CommandResult<()> {
    sqlx::query(
        "INSERT INTO pending_operations(id, account_slot_id, kind, message_id, source_mailbox_id, \
         destination_mailbox_id, uid, uid_validity, payload_json, base_modseq, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(operation.id)
    .bind(operation.account_slot_id)
    .bind(operation_kind_to_db(operation.kind))
    .bind(operation.message_id)
    .bind(operation.source_mailbox_id)
    .bind(operation.destination_mailbox_id)
    .bind(operation.uid)
    .bind(operation.uid_validity)
    .bind(operation.payload.to_string())
    .bind(operation.base_modseq)
    .bind(now())
    .bind(now())
    .execute(&mut **transaction)
    .await
    .map_err(|_| CommandError::new("operation.queue_failed"))?;
    Ok(())
}

async fn operation_location(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    account_slot_id: &str,
    mailbox_id: &str,
    message_id: &str,
) -> CommandResult<sqlx::sqlite::SqliteRow> {
    sqlx::query(
        "SELECT l.uid, l.uid_validity, l.modseq FROM message_locations l \
         JOIN mailboxes b ON b.id = l.mailbox_id JOIN messages m ON m.id = l.message_id \
         WHERE l.message_id = ? AND l.mailbox_id = ? AND b.account_slot_id = ? \
         AND m.account_slot_id = ? AND l.local_hidden = 0",
    )
    .bind(message_id)
    .bind(mailbox_id)
    .bind(account_slot_id)
    .bind(account_slot_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| CommandError::new("operation.queue_failed"))?
    .ok_or_else(|| CommandError::new("message.remote_location_missing"))
}

async fn refresh_mailbox_counts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mailbox_id: &str,
) -> CommandResult<()> {
    sqlx::query(
        "UPDATE mailboxes SET total_count = (SELECT COUNT(*) FROM message_locations \
         WHERE mailbox_id = ? AND local_hidden = 0), unread_count = (SELECT COUNT(*) \
         FROM message_locations WHERE mailbox_id = ? AND local_hidden = 0 AND unread = 1), \
         revision = revision + 1 WHERE id = ?",
    )
    .bind(mailbox_id)
    .bind(mailbox_id)
    .bind(mailbox_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| CommandError::new("storage.mailbox_write_failed"))?;
    Ok(())
}

fn operation_kind_to_db(kind: &PendingOperationKind) -> &'static str {
    match kind {
        PendingOperationKind::SetRead => "set_read",
        PendingOperationKind::SetFlagged => "set_flagged",
        PendingOperationKind::Copy => "copy",
        PendingOperationKind::Move => "move",
        PendingOperationKind::Delete => "delete",
        PendingOperationKind::AppendSent => "append_sent",
        PendingOperationKind::AppendDraft => "append_draft",
    }
}

fn operation_kind_from_db(value: String) -> PendingOperationKind {
    match value.as_str() {
        "set_read" => PendingOperationKind::SetRead,
        "set_flagged" => PendingOperationKind::SetFlagged,
        "copy" => PendingOperationKind::Copy,
        "move" => PendingOperationKind::Move,
        "delete" => PendingOperationKind::Delete,
        "append_sent" => PendingOperationKind::AppendSent,
        "append_draft" => PendingOperationKind::AppendDraft,
        _ => PendingOperationKind::SetRead,
    }
}

fn operation_status_from_db(value: String) -> PendingOperationStatus {
    match value.as_str() {
        "running" => PendingOperationStatus::Running,
        "retry_wait" => PendingOperationStatus::RetryWait,
        "needs_reconcile" => PendingOperationStatus::NeedsReconcile,
        "succeeded" => PendingOperationStatus::Succeeded,
        "failed" => PendingOperationStatus::Failed,
        _ => PendingOperationStatus::Queued,
    }
}

fn storage_read_error(_: sqlx::Error) -> CommandError {
    CommandError::new("storage.read_failed")
}

#[cfg(test)]
mod tests {
    use crate::core::{MailSyncSink, MailboxRole, MessageAddress, RemoteMailbox, RemoteMessage};

    use super::*;
    use crate::storage::{create_account_slot, initialize_content_database, MailRepository};

    async fn seeded_repository() -> (tempfile::TempDir, MailRepository, String, String, String) {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let inbox = repository
            .sync_sink()
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "INBOX".into(),
                    display_name: "INBOX".into(),
                    delimiter: Some("/".into()),
                    role: MailboxRole::Inbox,
                    selectable: true,
                    uid_validity: 7,
                    uid_next: 2,
                    total_count: 1,
                    unread_count: 1,
                    highest_modseq: Some(10),
                },
            )
            .await
            .unwrap();
        let archive = repository
            .sync_sink()
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "Archive".into(),
                    display_name: "Archive".into(),
                    delimiter: Some("/".into()),
                    role: MailboxRole::Archive,
                    selectable: true,
                    uid_validity: 8,
                    uid_next: 1,
                    total_count: 0,
                    unread_count: 0,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message(
                "slot",
                &inbox.id,
                &RemoteMessage {
                    uid: 1,
                    uid_validity: 7,
                    subject: "Queued".into(),
                    from: vec![MessageAddress {
                        name: None,
                        email: "sender@example.com".into(),
                    }],
                    to: vec![],
                    cc: vec![],
                    received_at: 10,
                    preview: "body".into(),
                    unread: true,
                    flagged: false,
                    size: 20,
                    message_id: Some("queued@example.com".into()),
                    references: vec![],
                    in_reply_to: None,
                    plain_text: Some("body".into()),
                    safe_html: None,
                    raw: Some(b"Subject: Queued\r\n\r\nbody".to_vec()),
                    attachments: vec![],
                    remote_images_blocked: false,
                    modseq: Some(10),
                },
            )
            .await
            .unwrap();
        let message_id = repository
            .read()
            .list_messages("slot", &inbox.id, None, 10)
            .await
            .unwrap()
            .items[0]
            .id
            .clone();
        (directory, repository, inbox.id, archive.id, message_id)
    }

    #[tokio::test]
    async fn flag_operation_updates_projection_and_is_claimed_durably() {
        let (_directory, repository, inbox_id, _, message_id) = seeded_repository().await;
        repository
            .operations()
            .queue_set_read("slot", &inbox_id, std::slice::from_ref(&message_id), true)
            .await
            .unwrap();
        let page = repository
            .read()
            .list_messages("slot", &inbox_id, None, 10)
            .await
            .unwrap();
        assert!(!page.items[0].unread);
        assert!(page.items[0].pending_operation);

        let work = repository
            .operations()
            .claim_pending_operation("slot")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(work.kind, PendingOperationKind::SetRead);
        assert_eq!(work.payload["value"], true);
        repository
            .operations()
            .complete_pending_operation(&work, false)
            .await
            .unwrap();
        assert!(repository
            .operations()
            .claim_pending_operation("slot")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn failed_move_restores_hidden_source_projection() {
        let (_directory, repository, inbox_id, archive_id, message_id) = seeded_repository().await;
        repository
            .operations()
            .queue_transfer(
                "slot",
                &inbox_id,
                &archive_id,
                std::slice::from_ref(&message_id),
                false,
            )
            .await
            .unwrap();
        assert!(repository
            .read()
            .list_messages("slot", &inbox_id, None, 10)
            .await
            .unwrap()
            .items
            .is_empty());
        let work = repository
            .operations()
            .claim_pending_operation("slot")
            .await
            .unwrap()
            .unwrap();
        repository
            .operations()
            .fail_pending_operation(&work, "operation.move_failed", false)
            .await
            .unwrap();
        assert_eq!(
            repository
                .read()
                .list_messages("slot", &inbox_id, None, 10)
                .await
                .unwrap()
                .items
                .len(),
            1
        );
    }
}

use async_trait::async_trait;
use sqlx::{QueryBuilder, Row, Sqlite};
use uuid::Uuid;

use crate::core::{
    CommandError, CommandResult, MailSyncSink, MessageUpsertOutcome, RemoteMailbox, RemoteMessage,
    RemoteMessageState, StoredMailbox, StoredMessageLocation,
};

use super::repository::{
    encode_json, map_storage_err, now, role_to_db, storage_read_error, SyncSinkRepository,
};
use super::upsert_remote_message_contacts;

const ATTACHMENT_INSERT_BATCH_SIZE: usize = 100;
const RECONCILE_UID_INSERT_BATCH_SIZE: usize = 500;

#[async_trait]
impl MailSyncSink for SyncSinkRepository {
    async fn upsert_mailbox(
        &self,
        account_slot_id: &str,
        mailbox: &RemoteMailbox,
    ) -> CommandResult<StoredMailbox> {
        let existing = sqlx::query("SELECT id, uid_validity, last_uid FROM mailboxes WHERE account_slot_id = ? AND remote_name = ?")
            .bind(account_slot_id)
            .bind(&mailbox.name)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_storage_err("storage.mailbox_write_failed"))?;
        let (id, last_uid, reset_locations, notification_baseline_required) = if let Some(row) =
            existing
        {
            let previous_validity: i64 = row.try_get("uid_validity").map_err(storage_read_error)?;
            let previous_uid: i64 = row.try_get("last_uid").map_err(storage_read_error)?;
            let validity_changed =
                previous_validity != 0 && previous_validity as u32 != mailbox.uid_validity;
            (
                row.try_get("id").map_err(storage_read_error)?,
                if !validity_changed {
                    previous_uid as u32
                } else {
                    0
                },
                validity_changed,
                previous_validity == 0 || validity_changed,
            )
        } else {
            (Uuid::new_v4().to_string(), 0, false, true)
        };

        if reset_locations {
            sqlx::query("DELETE FROM message_locations WHERE mailbox_id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .map_err(map_storage_err("storage.mailbox_reset_failed"))?;
        }
        sqlx::query(
            "INSERT INTO mailboxes(id, account_slot_id, remote_name, display_name, delimiter, role, selectable, \
                    uid_validity, uid_next, highest_modseq, total_count, unread_count, revision) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1) \
             ON CONFLICT(account_slot_id, remote_name) DO UPDATE SET \
             display_name = excluded.display_name, delimiter = excluded.delimiter, \
             role = excluded.role, selectable = excluded.selectable, \
             uid_validity = excluded.uid_validity, uid_next = excluded.uid_next, \
             highest_modseq = COALESCE(excluded.highest_modseq, mailboxes.highest_modseq), \
             total_count = excluded.total_count, unread_count = excluded.unread_count, revision = revision + 1",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(&mailbox.name)
        .bind(&mailbox.display_name)
        .bind(&mailbox.delimiter)
        .bind(role_to_db(&mailbox.role))
        .bind(i64::from(mailbox.selectable))
        .bind(i64::from(mailbox.uid_validity))
        .bind(i64::from(mailbox.uid_next))
        .bind(mailbox.highest_modseq.map(|value| value as i64))
        .bind(i64::from(mailbox.total_count))
        .bind(i64::from(mailbox.unread_count))
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.mailbox_write_failed"))?;
        Ok(StoredMailbox {
            id,
            last_uid,
            highest_modseq: mailbox.highest_modseq,
            notification_baseline_required,
        })
    }

    async fn upsert_message(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message: &RemoteMessage,
    ) -> CommandResult<MessageUpsertOutcome> {
        let from_json = encode_json(&message.from)?;
        let to_json = encode_json(&message.to)?;
        let cc_json = encode_json(&message.cc)?;
        let references_json = encode_json(&message.references)?;
        let raw_hash = match message.raw.as_deref() {
            Some(raw) => Some(self.content.write_raw(raw).await?),
            None => None,
        };
        let body_available = message.plain_text.is_some() || message.safe_html.is_some();
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("storage.message_write_failed"))?;
        let existing_location = sqlx::query_scalar::<_, String>(
            "SELECT message_id FROM message_locations WHERE mailbox_id = ? AND uid_validity = ? AND uid = ?",
        )
        .bind(mailbox_id)
        .bind(i64::from(message.uid_validity))
        .bind(i64::from(message.uid))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_write_failed"))?;

        let is_new_location = existing_location.is_none();
        let message_id = if let Some(id) = existing_location {
            id
        } else if let Some(remote_id) = message.message_id.as_deref() {
            sqlx::query_scalar::<_, String>(
                "SELECT id FROM messages WHERE account_slot_id = ? AND message_id = ? \
                 AND rfc822_size = ? AND received_at = ? LIMIT 1",
            )
            .bind(account_slot_id)
            .bind(remote_id)
            .bind(message.size as i64)
            .bind(message.received_at)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.message_write_failed"))?
            .unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            Uuid::new_v4().to_string()
        };

        sqlx::query(
            "INSERT INTO messages(id, account_slot_id, subject, from_json, to_json, cc_json, \
                    received_at, preview, rfc822_size, message_id, references_json, in_reply_to, \
                    has_attachments, raw_content_hash, body_availability, remote_images_blocked, revision) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1) \
             ON CONFLICT(id) DO UPDATE SET subject = excluded.subject, from_json = excluded.from_json, \
             to_json = excluded.to_json, cc_json = excluded.cc_json, received_at = excluded.received_at, \
             preview = CASE WHEN excluded.preview = '' THEN messages.preview ELSE excluded.preview END, \
             rfc822_size = excluded.rfc822_size, message_id = COALESCE(excluded.message_id, messages.message_id), \
             references_json = excluded.references_json, in_reply_to = excluded.in_reply_to, \
             has_attachments = MAX(messages.has_attachments, excluded.has_attachments), \
             raw_content_hash = COALESCE(excluded.raw_content_hash, messages.raw_content_hash), \
             body_availability = CASE WHEN excluded.body_availability = 'available' THEN 'available' ELSE messages.body_availability END, \
             remote_images_blocked = excluded.remote_images_blocked, revision = messages.revision + 1",
        )
        .bind(&message_id)
        .bind(account_slot_id)
        .bind(&message.subject)
        .bind(from_json)
        .bind(to_json)
        .bind(cc_json)
        .bind(message.received_at)
        .bind(&message.preview)
        .bind(message.size as i64)
        .bind(&message.message_id)
        .bind(references_json)
        .bind(&message.in_reply_to)
        .bind(i64::from(!message.attachments.is_empty()))
        .bind(raw_hash)
        .bind(if body_available { "available" } else { "missing" })
        .bind(i64::from(message.remote_images_blocked))
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_write_failed"))?;

        sqlx::query(
            "INSERT INTO message_locations(id, message_id, mailbox_id, uid, uid_validity, flags_json, \
                    unread, flagged, internal_date, modseq) VALUES (?, ?, ?, ?, ?, '[]', ?, ?, ?, ?) \
             ON CONFLICT(mailbox_id, uid_validity, uid) DO UPDATE SET \
             unread = CASE WHEN EXISTS (SELECT 1 FROM pending_operations o WHERE \
               o.message_id = message_locations.message_id AND o.source_mailbox_id = message_locations.mailbox_id \
               AND o.kind IN ('set_read','set_flagged') AND o.status IN ('queued','running','retry_wait')) \
               THEN message_locations.unread ELSE excluded.unread END, \
             flagged = CASE WHEN EXISTS (SELECT 1 FROM pending_operations o WHERE \
               o.message_id = message_locations.message_id AND o.source_mailbox_id = message_locations.mailbox_id \
               AND o.kind IN ('set_read','set_flagged') AND o.status IN ('queued','running','retry_wait')) \
               THEN message_locations.flagged ELSE excluded.flagged END, \
             internal_date = excluded.internal_date, modseq = COALESCE(excluded.modseq, message_locations.modseq)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&message_id)
        .bind(mailbox_id)
        .bind(i64::from(message.uid))
        .bind(i64::from(message.uid_validity))
        .bind(i64::from(message.unread))
        .bind(i64::from(message.flagged))
        .bind(message.received_at)
        .bind(message.modseq.map(|value| value as i64))
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_location_write_failed"))?;

        if body_available {
            sqlx::query(
                "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) VALUES (?, ?, ?, ?) \
                 ON CONFLICT(message_id) DO UPDATE SET plain_text = excluded.plain_text, \
                 safe_html = excluded.safe_html, updated_at = excluded.updated_at",
            )
            .bind(&message_id)
            .bind(&message.plain_text)
            .bind(&message.safe_html)
            .bind(now())
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.message_body_write_failed"))?;
        }

        for attachments in message.attachments.chunks(ATTACHMENT_INSERT_BATCH_SIZE) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "INSERT INTO attachments(id, message_id, part_index, file_name, content_type, size, content_id) ",
            );
            query.push_values(attachments, |mut row, attachment| {
                row.push_bind(Uuid::new_v4().to_string())
                    .push_bind(&message_id)
                    .push_bind(i64::from(attachment.part_index))
                    .push_bind(&attachment.file_name)
                    .push_bind(&attachment.content_type)
                    .push_bind(attachment.size as i64)
                    .push_bind(&attachment.content_id);
            });
            query.push(
                " ON CONFLICT(message_id, part_index) DO UPDATE SET \
                 file_name = excluded.file_name, content_type = excluded.content_type, size = excluded.size, \
                 content_id = excluded.content_id",
            );
            query
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.attachment_write_failed"))?;
        }

        let contacts_changed = upsert_remote_message_contacts(
            &mut transaction,
            account_slot_id,
            &message_id,
            &message.contact_addresses,
        )
        .await?;

        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.message_write_failed"))?;
        Ok(MessageUpsertOutcome {
            message_id,
            is_new_location,
            contacts_changed,
        })
    }

    async fn complete_notification_baseline(&self, account_slot_id: &str) -> CommandResult<()> {
        let result =
            sqlx::query("UPDATE account_slots SET notification_baseline_at = ? WHERE id = ?")
                .bind(now())
                .bind(account_slot_id)
                .execute(&self.pool)
                .await
                .map_err(map_storage_err(
                    "storage.notification_baseline_write_failed",
                ))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("account.not_found"));
        }
        Ok(())
    }

    async fn complete_mailbox(&self, mailbox_id: &str, last_uid: u32) -> CommandResult<()> {
        sqlx::query(
            "UPDATE mailboxes SET last_uid = ?, last_synced_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(i64::from(last_uid))
        .bind(now())
        .bind(mailbox_id)
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.mailbox_write_failed"))?;
        Ok(())
    }

    async fn stored_uids(&self, mailbox_id: &str, uid_validity: u32) -> CommandResult<Vec<u32>> {
        let uids = sqlx::query_scalar::<_, i64>(
            "SELECT uid FROM message_locations WHERE mailbox_id = ? AND uid_validity = ?",
        )
        .bind(mailbox_id)
        .bind(i64::from(uid_validity))
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.stored_uids_read_failed"))?;
        Ok(uids.into_iter().map(|uid| uid as u32).collect())
    }

    async fn pending_body_locations(
        &self,
        mailbox_id: &str,
        received_after: Option<i64>,
    ) -> CommandResult<Vec<StoredMessageLocation>> {
        let rows = sqlx::query(
            "SELECT l.uid, l.uid_validity FROM message_locations l \
             JOIN messages m ON m.id = l.message_id \
             WHERE l.mailbox_id = ? AND m.body_availability != 'available' \
               AND (? IS NULL OR l.internal_date >= ?) ORDER BY l.uid",
        )
        .bind(mailbox_id)
        .bind(received_after)
        .bind(received_after)
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.pending_bodies_read_failed"))?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredMessageLocation {
                    uid: row.try_get::<i64, _>("uid").map_err(storage_read_error)? as u32,
                    uid_validity: row
                        .try_get::<i64, _>("uid_validity")
                        .map_err(storage_read_error)? as u32,
                })
            })
            .collect()
    }

    async fn reconcile_mailbox(
        &self,
        mailbox_id: &str,
        uid_validity: u32,
        highest_modseq: Option<u64>,
        states: &[RemoteMessageState],
    ) -> CommandResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        for state in states {
            sqlx::query(
                "UPDATE message_locations SET unread = ?, flagged = ?, modseq = ? \
                 WHERE mailbox_id = ? AND uid_validity = ? AND uid = ? AND local_hidden = 0 \
                 AND NOT EXISTS (SELECT 1 FROM pending_operations o WHERE \
                   o.message_id = message_locations.message_id AND o.source_mailbox_id = message_locations.mailbox_id \
                   AND o.kind IN ('set_read','set_flagged') AND o.status IN ('queued','running','retry_wait'))",
            )
            .bind(i64::from(state.unread))
            .bind(i64::from(state.flagged))
            .bind(state.modseq.map(|value| value as i64))
            .bind(mailbox_id)
            .bind(i64::from(uid_validity))
            .bind(i64::from(state.uid))
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        }
        sqlx::query(
            "CREATE TEMP TABLE IF NOT EXISTS nextmail_reconcile_remote_uids(\
             uid INTEGER PRIMARY KEY) WITHOUT ROWID",
        )
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        sqlx::query("DELETE FROM nextmail_reconcile_remote_uids")
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        for state_batch in states.chunks(RECONCILE_UID_INSERT_BATCH_SIZE) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "INSERT OR IGNORE INTO nextmail_reconcile_remote_uids(uid) ",
            );
            query.push_values(state_batch, |mut row, state| {
                row.push_bind(i64::from(state.uid));
            });
            query
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        }
        sqlx::query(
            "DELETE FROM message_locations WHERE mailbox_id = ? AND uid_validity = ? \
             AND NOT EXISTS (SELECT 1 FROM nextmail_reconcile_remote_uids remote WHERE remote.uid = message_locations.uid) \
             AND NOT EXISTS (SELECT 1 FROM pending_operations operation WHERE \
               operation.message_id = message_locations.message_id \
               AND operation.source_mailbox_id = message_locations.mailbox_id \
               AND operation.status IN ('queued','running','retry_wait'))",
        )
        .bind(mailbox_id)
        .bind(i64::from(uid_validity))
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        sqlx::query(
            "UPDATE mailboxes SET highest_modseq = ?, total_count = (SELECT COUNT(*) FROM message_locations \
             WHERE mailbox_id = ? AND local_hidden = 0), unread_count = (SELECT COUNT(*) FROM message_locations \
             WHERE mailbox_id = ? AND local_hidden = 0 AND unread = 1), last_synced_at = ?, revision = revision + 1 \
             WHERE id = ?",
        )
        .bind(highest_modseq.map(|value| value as i64))
        .bind(mailbox_id)
        .bind(mailbox_id)
        .bind(now())
        .bind(mailbox_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.mailbox_reconcile_failed"))?;
        Ok(())
    }
}

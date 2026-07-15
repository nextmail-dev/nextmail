use std::{
    path::Path,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use nextmail_core::{
    AttachmentSummary, CommandError, CommandResult, ContentAvailability, MailSyncSink, MailboxRole,
    MailboxSummary, MessageAddress, MessageDetail, MessageListItem, MessageListPage, RemoteMailbox,
    RemoteMessage, RemoteMessageState, StoredMailbox, StoredMessageLocation, SyncPolicy,
};
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    ConnectOptions, Row, SqlitePool,
};
use uuid::Uuid;

use crate::ContentStore;

pub const CONTENT_DATABASE_FILENAME: &str = "content.sqlite";

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct MailRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone, Debug)]
pub struct RemoteMessageContext {
    pub mailbox_id: String,
    pub mailbox_name: String,
    pub uid: u32,
    pub uid_validity: u32,
}

impl MailRepository {
    pub async fn open(data_dir: &Path) -> CommandResult<Self> {
        let pool = open_pool(data_dir, false).await?;
        MIGRATOR
            .run(&pool)
            .await
            .map_err(|_| CommandError::new("data_directory.database_migration_failed"))?;
        Ok(Self {
            pool,
            content: ContentStore::new(data_dir),
        })
    }

    pub async fn list_mailboxes(
        &self,
        account_id: &str,
        account_slot_id: &str,
    ) -> CommandResult<Vec<MailboxSummary>> {
        let rows = sqlx::query(
            "SELECT b.id, b.display_name, b.delimiter, CASE WHEN o.role IS NOT NULL THEN o.role \
                      WHEN EXISTS(SELECT 1 FROM mailbox_role_overrides x WHERE x.account_slot_id = b.account_slot_id AND x.role = b.role) \
                      THEN 'other' ELSE b.role END AS role, b.selectable, \
                    b.total_count, b.unread_count, b.revision \
             FROM mailboxes b LEFT JOIN mailbox_role_overrides o ON o.mailbox_id = b.id \
               AND o.account_slot_id = b.account_slot_id WHERE b.account_slot_id = ? ORDER BY \
             CASE WHEN o.role = 'sent' THEN 1 WHEN o.role = 'drafts' THEN 2 WHEN o.role = 'archive' THEN 3 \
             WHEN o.role = 'trash' THEN 5 WHEN b.role = 'inbox' THEN 0 \
             WHEN EXISTS(SELECT 1 FROM mailbox_role_overrides x WHERE x.account_slot_id = b.account_slot_id AND x.role = b.role) THEN 6 \
             WHEN b.role = 'sent' THEN 1 WHEN b.role = 'drafts' THEN 2 WHEN b.role = 'archive' THEN 3 \
             WHEN b.role = 'junk' THEN 4 WHEN b.role = 'trash' THEN 5 ELSE 6 END, \
             b.remote_name COLLATE NOCASE",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.mailboxes_read_failed"))?;

        rows.into_iter()
            .map(|row| {
                Ok(MailboxSummary {
                    id: row.try_get("id").map_err(storage_read_error)?,
                    account_id: account_id.to_owned(),
                    name: row.try_get("display_name").map_err(storage_read_error)?,
                    delimiter: row.try_get("delimiter").map_err(storage_read_error)?,
                    role: role_from_db(row.try_get("role").map_err(storage_read_error)?),
                    selectable: row
                        .try_get::<i64, _>("selectable")
                        .map_err(storage_read_error)?
                        != 0,
                    total_count: row
                        .try_get::<i64, _>("total_count")
                        .map_err(storage_read_error)? as u32,
                    unread_count: row
                        .try_get::<i64, _>("unread_count")
                        .map_err(storage_read_error)? as u32,
                    revision: row
                        .try_get::<i64, _>("revision")
                        .map_err(storage_read_error)? as u64,
                })
            })
            .collect()
    }

    pub async fn list_messages(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        let limit = limit.clamp(1, 100);
        let (cursor_date, cursor_id) = cursor.and_then(parse_cursor).unzip();
        let rows = sqlx::query(
            "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, \
                    l.unread, l.flagged, m.has_attachments, m.body_availability, \
                    EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                      AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
             FROM message_locations l JOIN messages m ON m.id = l.message_id \
             JOIN mailboxes b ON b.id = l.mailbox_id \
             WHERE l.mailbox_id = ? AND b.account_slot_id = ? AND l.local_hidden = 0 AND \
               (? IS NULL OR l.internal_date < ? OR (l.internal_date = ? AND m.id < ?)) \
             ORDER BY l.internal_date DESC, m.id DESC LIMIT ?",
        )
        .bind(mailbox_id)
        .bind(account_slot_id)
        .bind(cursor_date)
        .bind(cursor_date)
        .bind(cursor_date)
        .bind(cursor_id.as_deref())
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.messages_read_failed"))?;

        let has_more = rows.len() > limit as usize;
        let mut items = rows
            .into_iter()
            .take(limit as usize)
            .map(message_list_item_from_row)
            .collect::<CommandResult<Vec<_>>>()?;
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| format!("{}:{}", item.received_at, item.id))
        } else {
            None
        };
        Ok(MessageListPage {
            items: std::mem::take(&mut items),
            next_cursor,
        })
    }

    pub async fn get_message_detail(
        &self,
        account_slot_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<MessageDetail> {
        let row = sqlx::query(
            "SELECT id, subject, from_json, to_json, cc_json, received_at, body_availability, \
                    remote_images_blocked, revision \
             FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_read_failed"))?
        .ok_or_else(|| CommandError::new("message.not_found"))?;

        let location = sqlx::query(
            "SELECT l.mailbox_id, l.unread, l.flagged, EXISTS(SELECT 1 FROM pending_operations o \
               WHERE o.message_id = l.message_id AND o.source_mailbox_id = l.mailbox_id \
               AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
             FROM message_locations l JOIN mailboxes b ON b.id = l.mailbox_id \
             WHERE l.message_id = ? AND b.account_slot_id = ? AND l.local_hidden = 0 \
               AND (? IS NULL OR l.mailbox_id = ?) \
             ORDER BY CASE b.role WHEN 'inbox' THEN 0 ELSE 1 END LIMIT 1",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .bind(mailbox_id)
        .bind(mailbox_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_location_read_failed"))?
        .ok_or_else(|| CommandError::new("message.remote_location_missing"))?;

        let body =
            sqlx::query("SELECT plain_text, safe_html FROM message_bodies WHERE message_id = ?")
                .bind(message_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| CommandError::new("storage.message_read_failed"))?;
        let attachment_rows = sqlx::query(
            "SELECT id, file_name, content_type, size, availability FROM attachments \
             WHERE message_id = ? ORDER BY part_index",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.attachments_read_failed"))?;
        let attachments = attachment_rows
            .into_iter()
            .map(|attachment| {
                Ok(AttachmentSummary {
                    id: attachment.try_get("id").map_err(storage_read_error)?,
                    file_name: attachment
                        .try_get("file_name")
                        .map_err(storage_read_error)?,
                    content_type: attachment
                        .try_get("content_type")
                        .map_err(storage_read_error)?,
                    size: attachment
                        .try_get::<i64, _>("size")
                        .map_err(storage_read_error)? as u64,
                    availability: availability_from_db(
                        attachment
                            .try_get("availability")
                            .map_err(storage_read_error)?,
                    ),
                })
            })
            .collect::<CommandResult<Vec<_>>>()?;

        Ok(MessageDetail {
            id: row.try_get("id").map_err(storage_read_error)?,
            mailbox_id: location.try_get("mailbox_id").map_err(storage_read_error)?,
            subject: row.try_get("subject").map_err(storage_read_error)?,
            from: decode_addresses(row.try_get("from_json").map_err(storage_read_error)?)?,
            to: decode_addresses(row.try_get("to_json").map_err(storage_read_error)?)?,
            cc: decode_addresses(row.try_get("cc_json").map_err(storage_read_error)?)?,
            received_at: row.try_get("received_at").map_err(storage_read_error)?,
            plain_text: body
                .as_ref()
                .and_then(|value| value.try_get("plain_text").ok()),
            safe_html: body
                .as_ref()
                .and_then(|value| value.try_get("safe_html").ok()),
            body_availability: availability_from_db(
                row.try_get("body_availability")
                    .map_err(storage_read_error)?,
            ),
            attachments,
            remote_images_blocked: row
                .try_get::<i64, _>("remote_images_blocked")
                .map_err(storage_read_error)?
                != 0,
            revision: row
                .try_get::<i64, _>("revision")
                .map_err(storage_read_error)? as u64,
            unread: location
                .try_get::<i64, _>("unread")
                .map_err(storage_read_error)?
                != 0,
            flagged: location
                .try_get::<i64, _>("flagged")
                .map_err(storage_read_error)?
                != 0,
            pending_operation: location
                .try_get::<i64, _>("pending_operation")
                .map_err(storage_read_error)?
                != 0,
        })
    }

    pub async fn remote_message_context(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<RemoteMessageContext> {
        let row = sqlx::query(
            "SELECT l.mailbox_id, b.remote_name, l.uid, l.uid_validity \
             FROM message_locations l \
             JOIN mailboxes b ON b.id = l.mailbox_id \
             JOIN messages m ON m.id = l.message_id \
             WHERE l.message_id = ? AND m.account_slot_id = ? AND b.selectable = 1 \
             ORDER BY CASE b.role WHEN 'inbox' THEN 0 ELSE 1 END, b.remote_name LIMIT 1",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_location_read_failed"))?
        .ok_or_else(|| CommandError::new("message.remote_location_missing"))?;
        Ok(RemoteMessageContext {
            mailbox_id: row.try_get("mailbox_id").map_err(storage_read_error)?,
            mailbox_name: row.try_get("remote_name").map_err(storage_read_error)?,
            uid: row.try_get::<i64, _>("uid").map_err(storage_read_error)? as u32,
            uid_validity: row
                .try_get::<i64, _>("uid_validity")
                .map_err(storage_read_error)? as u32,
        })
    }

    pub async fn get_sync_policy(&self, account_slot_id: &str) -> CommandResult<SyncPolicy> {
        let value = sqlx::query_scalar::<_, String>(
            "SELECT sync_policy FROM account_sync_settings WHERE account_slot_id = ?",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.sync_settings_read_failed"))?;
        Ok(value.as_deref().map(policy_from_db).unwrap_or_default())
    }

    pub async fn set_sync_policy(
        &self,
        account_slot_id: &str,
        policy: SyncPolicy,
    ) -> CommandResult<SyncPolicy> {
        sqlx::query(
            "INSERT INTO account_sync_settings(account_slot_id, sync_policy, updated_at) \
             VALUES (?, ?, ?) ON CONFLICT(account_slot_id) DO UPDATE SET \
             sync_policy = excluded.sync_policy, updated_at = excluded.updated_at",
        )
        .bind(account_slot_id)
        .bind(policy_to_db(&policy))
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.sync_settings_write_failed"))?;
        Ok(policy)
    }

    pub async fn raw_message(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<Option<Vec<u8>>> {
        let hash = sqlx::query_scalar::<_, Option<String>>(
            "SELECT raw_content_hash FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_read_failed"))?
        .flatten();
        match hash {
            Some(value) => self.content.read_raw(&value).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn attachment_context(
        &self,
        account_slot_id: &str,
        attachment_id: &str,
    ) -> CommandResult<(String, u32)> {
        let row = sqlx::query(
            "SELECT a.message_id, a.part_index FROM attachments a \
             JOIN messages m ON m.id = a.message_id \
             WHERE a.id = ? AND m.account_slot_id = ?",
        )
        .bind(attachment_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.attachment_read_failed"))?
        .ok_or_else(|| CommandError::new("attachment.not_found"))?;
        Ok((
            row.try_get("message_id").map_err(storage_read_error)?,
            row.try_get::<i64, _>("part_index")
                .map_err(storage_read_error)? as u32,
        ))
    }

    pub async fn store_attachment_content(
        &self,
        attachment_id: &str,
        content: &[u8],
    ) -> CommandResult<AttachmentSummary> {
        let hash = self.content.write_attachment(content).await?;
        sqlx::query(
            "UPDATE attachments SET availability = 'available', content_hash = ? WHERE id = ?",
        )
        .bind(hash)
        .bind(attachment_id)
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.attachment_write_failed"))?;
        let row = sqlx::query(
            "SELECT id, file_name, content_type, size, availability FROM attachments WHERE id = ?",
        )
        .bind(attachment_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.attachment_read_failed"))?;
        Ok(AttachmentSummary {
            id: row.try_get("id").map_err(storage_read_error)?,
            file_name: row.try_get("file_name").map_err(storage_read_error)?,
            content_type: row.try_get("content_type").map_err(storage_read_error)?,
            size: row.try_get::<i64, _>("size").map_err(storage_read_error)? as u64,
            availability: availability_from_db(
                row.try_get("availability").map_err(storage_read_error)?,
            ),
        })
    }
}

#[async_trait]
impl MailSyncSink for MailRepository {
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
            .map_err(|_| CommandError::new("storage.mailbox_write_failed"))?;
        let (id, last_uid, reset_locations) = if let Some(row) = existing {
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
            )
        } else {
            (Uuid::new_v4().to_string(), 0, false)
        };

        if reset_locations {
            sqlx::query("DELETE FROM message_locations WHERE mailbox_id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .map_err(|_| CommandError::new("storage.mailbox_reset_failed"))?;
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
        .map_err(|_| CommandError::new("storage.mailbox_write_failed"))?;
        Ok(StoredMailbox {
            id,
            last_uid,
            highest_modseq: mailbox.highest_modseq,
        })
    }

    async fn upsert_message(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message: &RemoteMessage,
    ) -> CommandResult<()> {
        let existing_location = sqlx::query_scalar::<_, String>(
            "SELECT message_id FROM message_locations WHERE mailbox_id = ? AND uid_validity = ? AND uid = ?",
        )
        .bind(mailbox_id)
        .bind(i64::from(message.uid_validity))
        .bind(i64::from(message.uid))
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_write_failed"))?;

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
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| CommandError::new("storage.message_write_failed"))?
            .unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            Uuid::new_v4().to_string()
        };

        let raw_hash = match message.raw.as_deref() {
            Some(raw) => Some(self.content.write_raw(raw).await?),
            None => None,
        };
        let body_available = message.plain_text.is_some() || message.safe_html.is_some();
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
        .bind(encode_json(&message.from)?)
        .bind(encode_json(&message.to)?)
        .bind(encode_json(&message.cc)?)
        .bind(message.received_at)
        .bind(&message.preview)
        .bind(message.size as i64)
        .bind(&message.message_id)
        .bind(encode_json(&message.references)?)
        .bind(&message.in_reply_to)
        .bind(i64::from(!message.attachments.is_empty()))
        .bind(raw_hash)
        .bind(if body_available { "available" } else { "missing" })
        .bind(i64::from(message.remote_images_blocked))
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_write_failed"))?;

        sqlx::query(
            "INSERT INTO message_locations(id, message_id, mailbox_id, uid, uid_validity, flags_json, \
                    unread, flagged, internal_date, modseq) VALUES (?, ?, ?, ?, ?, '[]', ?, ?, ?, ?) \
             ON CONFLICT(mailbox_id, uid_validity, uid) DO UPDATE SET \
             unread = excluded.unread, flagged = excluded.flagged, internal_date = excluded.internal_date, \
             modseq = excluded.modseq",
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
        .execute(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.message_location_write_failed"))?;

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
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("storage.message_body_write_failed"))?;
        }

        for attachment in &message.attachments {
            sqlx::query(
                "INSERT INTO attachments(id, message_id, part_index, file_name, content_type, size, content_id) \
                 VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(message_id, part_index) DO UPDATE SET \
                 file_name = excluded.file_name, content_type = excluded.content_type, size = excluded.size, \
                 content_id = excluded.content_id",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&message_id)
            .bind(i64::from(attachment.part_index))
            .bind(&attachment.file_name)
            .bind(&attachment.content_type)
            .bind(attachment.size as i64)
            .bind(&attachment.content_id)
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("storage.attachment_write_failed"))?;
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
        .map_err(|_| CommandError::new("storage.mailbox_write_failed"))?;
        Ok(())
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
               AND (? IS NULL OR l.internal_date >= ?) \
             ORDER BY l.uid",
        )
        .bind(mailbox_id)
        .bind(received_after)
        .bind(received_after)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.pending_bodies_read_failed"))?;
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
            .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
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
            .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
        }
        let remote_uids = states
            .iter()
            .map(|state| state.uid)
            .collect::<std::collections::HashSet<_>>();
        let local_rows = sqlx::query(
            "SELECT id, message_id, uid FROM message_locations WHERE mailbox_id = ? AND uid_validity = ?",
        )
        .bind(mailbox_id)
        .bind(i64::from(uid_validity))
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
        for row in local_rows {
            let uid = row.try_get::<i64, _>("uid").map_err(storage_read_error)? as u32;
            if remote_uids.contains(&uid) {
                continue;
            }
            let message_id: String = row.try_get("message_id").map_err(storage_read_error)?;
            let pending = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pending_operations WHERE message_id = ? AND source_mailbox_id = ? \
                 AND status IN ('queued','running','retry_wait')",
            )
            .bind(&message_id)
            .bind(mailbox_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
            if pending == 0 {
                let id: String = row.try_get("id").map_err(storage_read_error)?;
                sqlx::query("DELETE FROM message_locations WHERE id = ?")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
            }
        }
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
        .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CommandError::new("storage.mailbox_reconcile_failed"))?;
        Ok(())
    }
}

pub async fn initialize_content_database(data_dir: &Path) -> CommandResult<()> {
    let pool = open_pool(data_dir, true).await?;
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|_| CommandError::new("data_directory.database_migration_failed"))?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .map_err(|_| CommandError::new("data_directory.database_checkpoint_failed"))?;
    pool.close().await;
    Ok(())
}

pub async fn create_account_slot(
    data_dir: &Path,
    slot_id: &str,
    created_at: i64,
) -> CommandResult<()> {
    let pool = open_pool(data_dir, false).await?;
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|_| CommandError::new("data_directory.database_migration_failed"))?;
    sqlx::query("INSERT INTO account_slots (id, created_at) VALUES (?, ?)")
        .bind(slot_id)
        .bind(created_at)
        .execute(&pool)
        .await
        .map_err(|_| CommandError::new("account.slot_create_failed"))?;
    pool.close().await;
    Ok(())
}

pub async fn delete_account_slot(data_dir: &Path, slot_id: &str) {
    if let Ok(pool) = open_pool(data_dir, false).await {
        let _ = sqlx::query("DELETE FROM account_slots WHERE id = ?")
            .bind(slot_id)
            .execute(&pool)
            .await;
        pool.close().await;
    }
}

async fn open_pool(data_dir: &Path, create: bool) -> CommandResult<SqlitePool> {
    let database_path = data_dir.join(CONTENT_DATABASE_FILENAME);
    if !create && !database_path.is_file() {
        return Err(CommandError::new("data_directory.database_missing"));
    }
    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", database_path.display()))
        .map_err(|_| CommandError::new("data_directory.database_open_failed"))?
        .create_if_missing(create)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .disable_statement_logging();
    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|_| CommandError::new("data_directory.database_open_failed"))
}

fn message_list_item_from_row(row: sqlx::sqlite::SqliteRow) -> CommandResult<MessageListItem> {
    Ok(MessageListItem {
        id: row.try_get("id").map_err(storage_read_error)?,
        mailbox_id: row.try_get("mailbox_id").map_err(storage_read_error)?,
        subject: row.try_get("subject").map_err(storage_read_error)?,
        from: decode_addresses(row.try_get("from_json").map_err(storage_read_error)?)?,
        received_at: row.try_get("internal_date").map_err(storage_read_error)?,
        preview: row.try_get("preview").map_err(storage_read_error)?,
        unread: row
            .try_get::<i64, _>("unread")
            .map_err(storage_read_error)?
            != 0,
        flagged: row
            .try_get::<i64, _>("flagged")
            .map_err(storage_read_error)?
            != 0,
        has_attachments: row
            .try_get::<i64, _>("has_attachments")
            .map_err(storage_read_error)?
            != 0,
        body_availability: availability_from_db(
            row.try_get("body_availability")
                .map_err(storage_read_error)?,
        ),
        pending_operation: row
            .try_get::<i64, _>("pending_operation")
            .map_err(storage_read_error)?
            != 0,
    })
}

fn encode_json<T: serde::Serialize>(value: &T) -> CommandResult<String> {
    serde_json::to_string(value).map_err(|_| CommandError::new("storage.json_encode_failed"))
}

fn decode_addresses(value: String) -> CommandResult<Vec<MessageAddress>> {
    serde_json::from_str(&value).map_err(|_| CommandError::new("storage.json_decode_failed"))
}

fn parse_cursor(value: &str) -> Option<(i64, String)> {
    let (date, id) = value.split_once(':')?;
    Some((date.parse().ok()?, id.to_owned()))
}

fn role_to_db(role: &MailboxRole) -> &'static str {
    match role {
        MailboxRole::Inbox => "inbox",
        MailboxRole::Sent => "sent",
        MailboxRole::Drafts => "drafts",
        MailboxRole::Trash => "trash",
        MailboxRole::Junk => "junk",
        MailboxRole::Archive => "archive",
        MailboxRole::Other => "other",
    }
}

fn role_from_db(value: String) -> MailboxRole {
    match value.as_str() {
        "inbox" => MailboxRole::Inbox,
        "sent" => MailboxRole::Sent,
        "drafts" => MailboxRole::Drafts,
        "trash" => MailboxRole::Trash,
        "junk" => MailboxRole::Junk,
        "archive" => MailboxRole::Archive,
        _ => MailboxRole::Other,
    }
}

fn policy_to_db(policy: &SyncPolicy) -> &'static str {
    match policy {
        SyncPolicy::Days30 => "days30",
        SyncPolicy::Days90 => "days90",
        SyncPolicy::Days365 => "days365",
        SyncPolicy::All => "all",
    }
}

fn policy_from_db(value: &str) -> SyncPolicy {
    match value {
        "days30" => SyncPolicy::Days30,
        "days365" => SyncPolicy::Days365,
        "all" => SyncPolicy::All,
        _ => SyncPolicy::Days90,
    }
}

fn availability_from_db(value: String) -> ContentAvailability {
    match value.as_str() {
        "queued" => ContentAvailability::Queued,
        "available" => ContentAvailability::Available,
        "failed" => ContentAvailability::Failed,
        _ => ContentAvailability::Missing,
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn storage_read_error(_: sqlx::Error) -> CommandError {
    CommandError::new("storage.read_failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migration_and_mailbox_round_trip_work() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let mailbox = repository
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "INBOX".to_owned(),
                    display_name: "INBOX".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Inbox,
                    selectable: true,
                    uid_validity: 1,
                    uid_next: 2,
                    total_count: 1,
                    unread_count: 1,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();
        repository
            .upsert_message(
                "slot",
                &mailbox.id,
                &RemoteMessage {
                    uid: 1,
                    uid_validity: 1,
                    subject: "Stored locally".to_owned(),
                    from: vec![MessageAddress {
                        name: Some("Alice".to_owned()),
                        email: "alice@example.com".to_owned(),
                    }],
                    to: vec![],
                    cc: vec![],
                    received_at: 10,
                    preview: "Hello".to_owned(),
                    unread: true,
                    flagged: false,
                    size: 28,
                    message_id: Some("message@example.com".to_owned()),
                    references: vec![],
                    in_reply_to: None,
                    plain_text: Some("Hello from disk".to_owned()),
                    safe_html: None,
                    raw: Some(b"Subject: Stored locally\r\n\r\nHello".to_vec()),
                    attachments: vec![],
                    remote_images_blocked: false,
                    modseq: None,
                },
            )
            .await
            .unwrap();
        let mailboxes = repository.list_mailboxes("account", "slot").await.unwrap();
        assert_eq!(mailboxes.len(), 1);
        assert_eq!(mailboxes[0].role, MailboxRole::Inbox);

        let page = repository
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let detail = repository
            .get_message_detail("slot", &page.items[0].id, Some(&mailbox.id))
            .await
            .unwrap();
        assert_eq!(detail.plain_text.as_deref(), Some("Hello from disk"));
        assert!(repository
            .raw_message("slot", &detail.id)
            .await
            .unwrap()
            .is_some());
        let context = repository
            .remote_message_context("slot", &detail.id)
            .await
            .unwrap();
        assert_eq!(context.mailbox_name, "INBOX");
        assert_eq!(context.uid, 1);

        repository
            .upsert_message(
                "slot",
                &mailbox.id,
                &RemoteMessage {
                    uid: 2,
                    uid_validity: 1,
                    subject: "Header only".to_owned(),
                    from: vec![],
                    to: vec![],
                    cc: vec![],
                    received_at: 20,
                    preview: String::new(),
                    unread: false,
                    flagged: false,
                    size: 100,
                    message_id: None,
                    references: vec![],
                    in_reply_to: None,
                    plain_text: None,
                    safe_html: None,
                    raw: None,
                    attachments: vec![],
                    remote_images_blocked: false,
                    modseq: None,
                },
            )
            .await
            .unwrap();
        let pending = repository
            .pending_body_locations(&mailbox.id, Some(15))
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].uid, 2);
        let header_only = repository
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.subject == "Header only")
            .unwrap();
        assert!(repository
            .raw_message("slot", &header_only.id)
            .await
            .unwrap()
            .is_none());
    }
}

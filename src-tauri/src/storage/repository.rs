use std::{
    path::Path,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::core::{
    AddressPresentation, AttachmentSummary, CommandError, CommandResult, ContentAvailability,
    MailboxRole, MailboxSummary, MessageAddress, MessageDetail, MessageListItem, MessageListPage,
    RemoteMessageBody, SyncInterval,
};
use async_trait::async_trait;
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    ConnectOptions, FromRow, Row, SqlitePool,
};

use super::{
    normalize_email, CompositionDefinitionRepository, ContactIdentity, ContactRepository,
    ContentStore, DraftRepository, MailboxRepository, MailboxRoleRepository, OperationRepository,
    PreparedAttachmentFile, SendJobRepository,
};

pub const CONTENT_DATABASE_FILENAME: &str = "content.sqlite";

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct MailRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone)]
pub struct MailReadRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone)]
pub struct SyncSinkRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(FromRow)]
struct MessageDetailRow {
    id: String,
    subject: String,
    from_json: String,
    to_json: String,
    cc_json: String,
    received_at: i64,
    body_availability: String,
    remote_images_blocked: i64,
    revision: i64,
}

#[derive(FromRow)]
struct MessageLocationRow {
    mailbox_id: String,
    unread: i64,
    flagged: i64,
    pending_operation: i64,
}

#[derive(FromRow)]
struct MessageBodyRow {
    plain_text: Option<String>,
    safe_html: Option<String>,
}

#[derive(FromRow)]
struct AttachmentSummaryRow {
    id: String,
    file_name: String,
    content_type: String,
    size: i64,
    availability: String,
}

fn message_detail_from_rows(
    message: MessageDetailRow,
    location: MessageLocationRow,
    body: Option<MessageBodyRow>,
    attachments: Vec<AttachmentSummary>,
) -> CommandResult<MessageDetail> {
    Ok(MessageDetail {
        id: message.id,
        mailbox_id: location.mailbox_id,
        subject: message.subject,
        from: decode_address_presentations(message.from_json)?,
        to: decode_address_presentations(message.to_json)?,
        cc: decode_address_presentations(message.cc_json)?,
        received_at: message.received_at,
        plain_text: body.as_ref().and_then(|value| value.plain_text.clone()),
        safe_html: body.and_then(|value| value.safe_html),
        body_availability: availability_from_db(message.body_availability),
        attachments,
        remote_images_blocked: message.remote_images_blocked != 0,
        revision: message.revision as u64,
        unread: location.unread != 0,
        flagged: location.flagged != 0,
        pending_operation: location.pending_operation != 0,
    })
}

#[async_trait]
pub trait MailRepositoryProvider: Send + Sync {
    async fn open(&self, data_dir: &Path) -> CommandResult<MailRepository>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteMailRepositoryProvider;

#[async_trait]
impl MailRepositoryProvider for SqliteMailRepositoryProvider {
    async fn open(&self, data_dir: &Path) -> CommandResult<MailRepository> {
        MailRepository::open(data_dir).await
    }
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

    pub fn read(&self) -> MailReadRepository {
        MailReadRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn sync_sink(&self) -> SyncSinkRepository {
        SyncSinkRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn contacts(&self) -> ContactRepository {
        ContactRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn drafts(&self) -> DraftRepository {
        DraftRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn send_jobs(&self) -> SendJobRepository {
        SendJobRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn operations(&self) -> OperationRepository {
        OperationRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn mailbox_roles(&self) -> MailboxRoleRepository {
        MailboxRoleRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn mailboxes(&self) -> MailboxRepository {
        MailboxRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn composition_definitions(&self) -> CompositionDefinitionRepository {
        CompositionDefinitionRepository {
            pool: self.pool.clone(),
        }
    }
}

impl SyncSinkRepository {
    pub async fn replace_message_body(
        &self,
        account_slot_id: &str,
        message_id: &str,
        body: &RemoteMessageBody,
    ) -> CommandResult<()> {
        let mut transaction = begin_write(&self.pool)
            .await
            .map_err(map_storage_err("storage.message_body_write_failed"))?;
        let result = sqlx::query(
            "UPDATE messages SET body_availability = 'available', remote_images_blocked = ?, \
             preview = COALESCE(?, preview), \
             revision = revision + 1 WHERE id = ? AND account_slot_id = ?",
        )
        .bind(i64::from(body.remote_images_blocked))
        .bind(&body.preview)
        .bind(message_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("message.not_found"));
        }
        sqlx::query(
            "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
             VALUES (?, ?, ?, ?) ON CONFLICT(message_id) DO UPDATE SET \
             plain_text = excluded.plain_text, safe_html = excluded.safe_html, \
             updated_at = excluded.updated_at",
        )
        .bind(message_id)
        .bind(&body.plain_text)
        .bind(&body.safe_html)
        .bind(now())
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        for attachment in &body.attachments {
            sqlx::query(
                "INSERT INTO attachments(id, message_id, part_index, imap_section, file_name, content_type, size, content_id) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(message_id, part_index) DO UPDATE SET \
                 imap_section = COALESCE(excluded.imap_section, attachments.imap_section), \
                 file_name = excluded.file_name, content_type = excluded.content_type, \
                 size = CASE WHEN attachments.availability = 'available' THEN attachments.size ELSE excluded.size END, \
                 content_id = excluded.content_id",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(message_id)
            .bind(i64::from(attachment.part_index))
            .bind(&attachment.imap_section)
            .bind(&attachment.file_name)
            .bind(&attachment.content_type)
            .bind(attachment.size as i64)
            .bind(&attachment.content_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.attachment_write_failed"))?;
        }
        for content_id in &body.inline_content_ids {
            sqlx::query("DELETE FROM attachments WHERE message_id = ? AND lower(content_id) = ?")
                .bind(message_id)
                .bind(content_id)
                .execute(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.message_body_write_failed"))?;
        }
        sqlx::query(
            "UPDATE messages SET has_attachments = EXISTS(SELECT 1 FROM attachments WHERE message_id = ?) \
             WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(message_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.message_body_write_failed"))
    }
}

impl MailReadRepository {
    fn contacts(&self) -> ContactRepository {
        ContactRepository {
            pool: self.pool.clone(),
        }
    }

    pub async fn notification_baseline_ready(&self, account_slot_id: &str) -> CommandResult<bool> {
        let ready = sqlx::query_scalar::<_, i64>(
            "SELECT notification_baseline_at IS NOT NULL FROM account_slots WHERE id = ?",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.notification_baseline_read_failed"))?
        .ok_or_else(|| CommandError::new("account.not_found"))?;
        Ok(ready != 0)
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
             CASE WHEN b.local_sort_order IS NULL THEN 1 ELSE 0 END, b.local_sort_order, \
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
        .map_err(map_storage_err("storage.mailboxes_read_failed"))?;

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
        .map_err(map_storage_err("storage.messages_read_failed"))?;

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
        self.resolve_message_items(account_slot_id, &mut items)
            .await?;
        Ok(MessageListPage {
            items: std::mem::take(&mut items),
            next_cursor,
        })
    }

    pub async fn search_messages(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        let query = query.trim();
        if query.is_empty() {
            return self
                .list_messages(account_slot_id, mailbox_id, cursor, limit)
                .await;
        }

        let limit = limit.clamp(1, 100);
        let (cursor_date, cursor_id) = cursor.and_then(parse_cursor).unzip();
        let rows = if query.chars().count() < 3 {
            sqlx::query(
                "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, \
                        l.unread, l.flagged, m.has_attachments, m.body_availability, \
                        EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                          AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
                 FROM message_search JOIN messages m ON m.id = message_search.message_id \
                 JOIN message_locations l ON l.message_id = m.id \
                 JOIN mailboxes b ON b.id = l.mailbox_id \
                 WHERE message_search.account_slot_id = ? AND m.account_slot_id = ? \
                   AND l.mailbox_id = ? AND b.account_slot_id = ? AND l.local_hidden = 0 \
                   AND (instr(lower(message_search.subject), lower(?)) > 0 \
                     OR instr(lower(message_search.addresses), lower(?)) > 0 \
                     OR instr(lower(message_search.preview), lower(?)) > 0 \
                     OR instr(lower(message_search.body), lower(?)) > 0 \
                     OR instr(lower(message_search.attachment_names), lower(?)) > 0) \
                   AND (? IS NULL OR l.internal_date < ? OR (l.internal_date = ? AND m.id < ?)) \
                 ORDER BY l.internal_date DESC, m.id DESC LIMIT ?",
            )
            .bind(account_slot_id)
            .bind(account_slot_id)
            .bind(mailbox_id)
            .bind(account_slot_id)
            .bind(query)
            .bind(query)
            .bind(query)
            .bind(query)
            .bind(query)
            .bind(cursor_date)
            .bind(cursor_date)
            .bind(cursor_date)
            .bind(cursor_id.as_deref())
            .bind(i64::from(limit) + 1)
            .fetch_all(&self.pool)
            .await
        } else {
            let literal_query = format!("\"{}\"", query.replace('"', "\"\""));
            sqlx::query(
                "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, \
                        l.unread, l.flagged, m.has_attachments, m.body_availability, \
                        EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                          AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
                 FROM message_search JOIN messages m ON m.id = message_search.message_id \
                 JOIN message_locations l ON l.message_id = m.id \
                 JOIN mailboxes b ON b.id = l.mailbox_id \
                 WHERE message_search MATCH ? AND message_search.account_slot_id = ? \
                   AND m.account_slot_id = ? AND l.mailbox_id = ? AND b.account_slot_id = ? \
                   AND l.local_hidden = 0 \
                   AND (? IS NULL OR l.internal_date < ? OR (l.internal_date = ? AND m.id < ?)) \
                 ORDER BY l.internal_date DESC, m.id DESC LIMIT ?",
            )
            .bind(literal_query)
            .bind(account_slot_id)
            .bind(account_slot_id)
            .bind(mailbox_id)
            .bind(account_slot_id)
            .bind(cursor_date)
            .bind(cursor_date)
            .bind(cursor_date)
            .bind(cursor_id.as_deref())
            .bind(i64::from(limit) + 1)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(map_storage_err("storage.messages_read_failed"))?;

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
        self.resolve_message_items(account_slot_id, &mut items)
            .await?;
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
        let message = self.message_detail_row(account_slot_id, message_id).await?;
        let location = self
            .message_location_row(account_slot_id, message_id, mailbox_id)
            .await?;
        let body = self.message_body_row(message_id).await?;
        let attachments = self.attachment_summaries(message_id).await?;

        let mut detail = message_detail_from_rows(message, location, body, attachments)?;
        self.resolve_message_detail(account_slot_id, &mut detail)
            .await?;
        Ok(detail)
    }

    async fn resolve_message_items(
        &self,
        account_slot_id: &str,
        items: &mut [MessageListItem],
    ) -> CommandResult<()> {
        let emails = items
            .iter()
            .flat_map(|item| item.from.iter().map(|address| address.email.clone()))
            .collect::<Vec<_>>();
        let identities = self
            .contacts()
            .identities_for_emails(account_slot_id, &emails)
            .await?;
        for address in items.iter_mut().flat_map(|item| item.from.iter_mut()) {
            apply_contact_identity(address, &identities);
        }
        Ok(())
    }

    async fn resolve_message_detail(
        &self,
        account_slot_id: &str,
        detail: &mut MessageDetail,
    ) -> CommandResult<()> {
        let emails = detail
            .from
            .iter()
            .chain(detail.to.iter())
            .chain(detail.cc.iter())
            .map(|address| address.email.clone())
            .collect::<Vec<_>>();
        let identities = self
            .contacts()
            .identities_for_emails(account_slot_id, &emails)
            .await?;
        for address in detail
            .from
            .iter_mut()
            .chain(detail.to.iter_mut())
            .chain(detail.cc.iter_mut())
        {
            apply_contact_identity(address, &identities);
        }
        Ok(())
    }

    async fn message_detail_row(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<MessageDetailRow> {
        sqlx::query_as(
            "SELECT id, subject, from_json, to_json, cc_json, received_at, body_availability, \
                    remote_images_blocked, revision \
             FROM messages WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.message_read_failed"))?
        .ok_or_else(|| CommandError::new("message.not_found"))
    }

    async fn message_location_row(
        &self,
        account_slot_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<MessageLocationRow> {
        sqlx::query_as(
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
        .map_err(map_storage_err("storage.message_location_read_failed"))?
        .ok_or_else(|| CommandError::new("message.remote_location_missing"))
    }

    async fn message_body_row(&self, message_id: &str) -> CommandResult<Option<MessageBodyRow>> {
        sqlx::query_as("SELECT plain_text, safe_html FROM message_bodies WHERE message_id = ?")
            .bind(message_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_storage_err("storage.message_read_failed"))
    }

    async fn attachment_summaries(
        &self,
        message_id: &str,
    ) -> CommandResult<Vec<AttachmentSummary>> {
        let rows: Vec<AttachmentSummaryRow> = sqlx::query_as(
            "SELECT id, file_name, content_type, size, availability FROM attachments \
             WHERE message_id = ? ORDER BY part_index",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachments_read_failed"))?;

        Ok(rows
            .into_iter()
            .map(|attachment| AttachmentSummary {
                id: attachment.id,
                file_name: attachment.file_name,
                content_type: attachment.content_type,
                size: attachment.size as u64,
                availability: availability_from_db(attachment.availability),
            })
            .collect())
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
        .map_err(map_storage_err("storage.message_location_read_failed"))?
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

    pub async fn get_sync_interval(&self, account_slot_id: &str) -> CommandResult<SyncInterval> {
        let value = sqlx::query_scalar::<_, i64>(
            "SELECT sync_interval_minutes FROM account_sync_settings WHERE account_slot_id = ?",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.sync_settings_read_failed"))?;
        Ok(value.map(sync_interval_from_db).unwrap_or_default())
    }

    pub async fn set_sync_interval(
        &self,
        account_slot_id: &str,
        interval: SyncInterval,
    ) -> CommandResult<SyncInterval> {
        sqlx::query(
            "INSERT INTO account_sync_settings(account_slot_id, sync_interval_minutes, updated_at) \
             VALUES (?, ?, ?) ON CONFLICT(account_slot_id) DO UPDATE SET \
             sync_interval_minutes = excluded.sync_interval_minutes, updated_at = excluded.updated_at",
        )
        .bind(account_slot_id)
        .bind(sync_interval_to_db(&interval))
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.sync_settings_write_failed"))?;
        Ok(interval)
    }

    pub async fn get_download_full_messages(&self, account_slot_id: &str) -> CommandResult<bool> {
        let value = sqlx::query_scalar::<_, i64>(
            "SELECT download_full_messages FROM account_sync_settings WHERE account_slot_id = ?",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.sync_settings_read_failed"))?;
        Ok(value.unwrap_or_default() != 0)
    }

    pub async fn set_download_full_messages(
        &self,
        account_slot_id: &str,
        enabled: bool,
    ) -> CommandResult<bool> {
        sqlx::query(
            "INSERT INTO account_sync_settings(account_slot_id, download_full_messages, updated_at) \
             VALUES (?, ?, ?) ON CONFLICT(account_slot_id) DO UPDATE SET \
             download_full_messages = excluded.download_full_messages, updated_at = excluded.updated_at",
        )
        .bind(account_slot_id)
        .bind(i64::from(enabled))
        .bind(now())
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.sync_settings_write_failed"))?;
        Ok(enabled)
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
        .map_err(map_storage_err("storage.message_read_failed"))?
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
    ) -> CommandResult<(String, u32, Option<String>)> {
        let row = sqlx::query(
            "SELECT a.message_id, a.part_index, a.imap_section FROM attachments a \
             JOIN messages m ON m.id = a.message_id \
             WHERE a.id = ? AND m.account_slot_id = ?",
        )
        .bind(attachment_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachment_read_failed"))?
        .ok_or_else(|| CommandError::new("attachment.not_found"))?;
        Ok((
            row.try_get("message_id").map_err(storage_read_error)?,
            row.try_get::<i64, _>("part_index")
                .map_err(storage_read_error)? as u32,
            row.try_get("imap_section").map_err(storage_read_error)?,
        ))
    }

    pub async fn attachment_summary(
        &self,
        account_slot_id: &str,
        attachment_id: &str,
    ) -> CommandResult<AttachmentSummary> {
        let row = sqlx::query(
            "SELECT a.id, a.file_name, a.content_type, a.size, a.availability FROM attachments a \
             JOIN messages m ON m.id = a.message_id WHERE a.id = ? AND m.account_slot_id = ?",
        )
        .bind(attachment_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachment_read_failed"))?
        .ok_or_else(|| CommandError::new("attachment.not_found"))?;
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

    pub async fn prepare_attachment_file(
        &self,
        account_slot_id: &str,
        attachment_id: &str,
    ) -> CommandResult<PreparedAttachmentFile> {
        let row = sqlx::query(
            "SELECT a.file_name, a.content_hash FROM attachments a \
             JOIN messages m ON m.id = a.message_id WHERE a.id = ? AND m.account_slot_id = ?",
        )
        .bind(attachment_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachment_read_failed"))?
        .ok_or_else(|| CommandError::new("attachment.not_found"))?;
        let file_name = row
            .try_get::<String, _>("file_name")
            .map_err(storage_read_error)?;
        let hash = row
            .try_get::<Option<String>, _>("content_hash")
            .map_err(storage_read_error)?
            .ok_or_else(|| CommandError::new("attachment.content_unavailable"))?;
        self.content
            .materialize_attachment(attachment_id, &file_name, &hash)
            .await
    }

    pub async fn store_attachment_content(
        &self,
        account_slot_id: &str,
        attachment_id: &str,
        content: &[u8],
    ) -> CommandResult<AttachmentSummary> {
        // Validate ownership before writing into the content-addressed store so a caller
        // cannot create orphaned content by presenting another account's attachment ID.
        self.attachment_context(account_slot_id, attachment_id)
            .await?;
        let hash = self.content.write_attachment(content).await?;
        sqlx::query(
            "UPDATE attachments SET availability = 'available', content_hash = ?, size = ? WHERE id = ? \
             AND EXISTS(SELECT 1 FROM messages m WHERE m.id = attachments.message_id AND m.account_slot_id = ?)",
        )
        .bind(hash)
        .bind(content.len() as i64)
        .bind(attachment_id)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachment_write_failed"))?;
        let row = sqlx::query(
            "SELECT a.id, a.file_name, a.content_type, a.size, a.availability FROM attachments a \
             JOIN messages m ON m.id = a.message_id WHERE a.id = ? AND m.account_slot_id = ?",
        )
        .bind(attachment_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.attachment_read_failed"))?
        .ok_or_else(|| CommandError::new("attachment.not_found"))?;
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
        // SQLite serializes all writers through a single lock even in WAL mode.
        // The sync runtime fans out up to 3 worker sessions that each upsert in
        // their own transaction; without an explicit busy timeout a worker that
        // loses the write lock fails instantly with SQLITE_BUSY instead of
        // waiting for the in-progress write to finish. 15s is far beyond the
        // per-message upsert cost, so this turns contention into brief waits.
        .busy_timeout(Duration::from_secs(15))
        .disable_statement_logging();
    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|_| CommandError::new("data_directory.database_open_failed"))
}

pub(super) async fn begin_write(
    pool: &SqlitePool,
) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, sqlx::Error> {
    // BEGIN IMMEDIATE obtains SQLite's single writer slot before any reads in
    // the transaction, so busy_timeout can wait instead of a later read-to-write
    // upgrade failing immediately with SQLITE_BUSY.
    pool.begin_with("BEGIN IMMEDIATE").await
}

fn message_list_item_from_row(row: sqlx::sqlite::SqliteRow) -> CommandResult<MessageListItem> {
    Ok(MessageListItem {
        id: row.try_get("id").map_err(storage_read_error)?,
        mailbox_id: row.try_get("mailbox_id").map_err(storage_read_error)?,
        subject: row.try_get("subject").map_err(storage_read_error)?,
        from: decode_address_presentations(row.try_get("from_json").map_err(storage_read_error)?)?,
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

pub(super) fn encode_json<T: serde::Serialize>(value: &T) -> CommandResult<String> {
    serde_json::to_string(value).map_err(map_storage_err("storage.json_encode_failed"))
}

fn decode_addresses(value: String) -> CommandResult<Vec<MessageAddress>> {
    serde_json::from_str(&value).map_err(map_storage_err("storage.json_decode_failed"))
}

fn decode_address_presentations(value: String) -> CommandResult<Vec<AddressPresentation>> {
    Ok(decode_addresses(value)?
        .iter()
        .map(AddressPresentation::from_header)
        .collect())
}

fn apply_contact_identity(
    address: &mut AddressPresentation,
    identities: &std::collections::HashMap<String, ContactIdentity>,
) {
    let identity =
        normalize_email(&address.email).and_then(|(_, normalized)| identities.get(&normalized));
    address.contact_id = identity.map(|value| value.id.clone());
    address.name = identity
        .map(|value| value.name.clone())
        .or_else(|| address.header_name.clone());
}

fn parse_cursor(value: &str) -> Option<(i64, String)> {
    let (date, id) = value.split_once(':')?;
    Some((date.parse().ok()?, id.to_owned()))
}

pub(super) fn role_to_db(role: &MailboxRole) -> &'static str {
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

fn sync_interval_to_db(interval: &SyncInterval) -> i64 {
    interval.minutes().map_or(0, |minutes| minutes as i64)
}

fn sync_interval_from_db(value: i64) -> SyncInterval {
    match value {
        0 => SyncInterval::Manual,
        5 => SyncInterval::Minutes5,
        10 => SyncInterval::Minutes10,
        _ => SyncInterval::Minutes1,
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

pub(super) fn storage_read_error(error: sqlx::Error) -> CommandError {
    tracing::warn!(?error, "storage read failed");
    CommandError::new("storage.read_failed")
}

// Mirrors `map_imap_err`: preserves the underlying storage error in the log
// instead of discarding it via `.map_err(|_| ...)`. Without this every storage
// write failure surfaces only a generic code (e.g. "storage.message_write_failed")
// and the real cause - SQLITE_BUSY, disk I/O, a trigger fault - is lost.
pub(super) fn map_storage_err<E: std::fmt::Debug>(
    code: &'static str,
) -> impl FnOnce(E) -> CommandError {
    move |error| {
        tracing::warn!(%code, ?error, "storage operation failed");
        CommandError::new(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ContactAddressRole, ContactDraft, MailSyncSink, RemoteContactAddress, RemoteMailbox,
        RemoteMessage, StoredMailbox,
    };

    #[tokio::test]
    async fn write_transactions_wait_for_the_active_writer() {
        let directory = tempfile::tempdir().unwrap();
        let pool = open_pool(directory.path(), true).await.unwrap();
        let first = begin_write(&pool).await.unwrap();
        let waiting_pool = pool.clone();
        let mut waiting = tokio::spawn(async move {
            begin_write(&waiting_pool)
                .await
                .unwrap()
                .rollback()
                .await
                .unwrap();
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(30), &mut waiting)
                .await
                .is_err()
        );
        first.rollback().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("waiting writer should acquire the released slot")
            .unwrap();
    }

    #[tokio::test]
    async fn stylesheet_policy_migration_invalidates_only_cached_html_bodies() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                remote_images_blocked INTEGER NOT NULL,
                revision INTEGER NOT NULL
             );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT,
                safe_html TEXT
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '9');
             INSERT INTO messages VALUES ('html', 'available', 1, 4);
             INSERT INTO messages VALUES ('plain', 'available', 0, 2);
             INSERT INTO message_bodies VALUES ('html', 'HTML fallback', '<p>Cached</p>');
             INSERT INTO message_bodies VALUES ('plain', 'Plain only', NULL);",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0010_html_stylesheet_and_theme_fidelity.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let html_message: (String, i64, i64) = sqlx::query_as(
            "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(html_message, ("missing".to_owned(), 0, 5));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );

        let plain_message: (String, i64) =
            sqlx::query_as("SELECT body_availability, revision FROM messages WHERE id = 'plain'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(plain_message, ("available".to_owned(), 2));
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "Plain only"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "10"
        );
    }

    #[tokio::test]
    async fn transient_controlled_link_schema_is_removed_by_direct_link_migration() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                remote_images_blocked INTEGER NOT NULL,
                revision INTEGER NOT NULL
             );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT,
                safe_html TEXT
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '10');
             INSERT INTO messages VALUES ('html', 'available', 0, 3);
             INSERT INTO message_bodies VALUES ('html', 'fallback', '<a>old linkless cache</a>');",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0011_controlled_mail_links.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let message: (String, i64) =
            sqlx::query_as("SELECT body_availability, revision FROM messages WHERE id = 'html'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(message, ("missing".to_owned(), 4));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message_bodies")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "11"
        );

        sqlx::raw_sql(
            "UPDATE messages SET body_availability = 'available';
             INSERT INTO message_bodies(message_id, plain_text, safe_html)
             VALUES ('html', 'fallback', '<a>cached bridge</a>');",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/0012_direct_mail_links_and_layout_fidelity.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "12"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'message_links'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message_bodies")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn functional_selector_policy_migration_invalidates_cached_html() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                remote_images_blocked INTEGER NOT NULL,
                revision INTEGER NOT NULL
             );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT,
                safe_html TEXT
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '13');
             INSERT INTO messages VALUES ('html', 'available', 1, 7);
             INSERT INTO messages VALUES ('plain', 'available', 0, 3);
             INSERT INTO message_bodies VALUES ('html', 'fallback', '<table>stale</table>');
             INSERT INTO message_bodies VALUES ('plain', 'plain only', NULL);",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0014_functional_selector_fidelity.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("missing".to_owned(), 0, 8)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "plain only"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "14"
        );
    }

    async fn assert_html_cache_invalidation_migration(
        migration: &'static str,
        initial_version: &str,
        expected_version: &str,
    ) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                remote_images_blocked INTEGER NOT NULL,
                revision INTEGER NOT NULL
             );
                 CREATE TABLE message_bodies(
                    message_id TEXT PRIMARY KEY,
                    plain_text TEXT,
                    safe_html TEXT
                 );
                 CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO messages VALUES ('html', 'available', 1, 9);
                 INSERT INTO messages VALUES ('plain', 'available', 0, 4);
                 INSERT INTO message_bodies VALUES ('html', 'fallback', '<img>');
                 INSERT INTO message_bodies VALUES ('plain', 'plain only', NULL);",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', ?)")
            .bind(initial_version)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::raw_sql(migration).execute(&pool).await.unwrap();

        assert_eq!(
            sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("missing".to_owned(), 0, 10)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "plain only"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            expected_version
        );
    }

    #[tokio::test]
    async fn inline_image_policy_migration_invalidates_only_cached_html() {
        assert_html_cache_invalidation_migration(
            include_str!("../../migrations/0021_inline_image_fidelity.sql"),
            "20",
            "21",
        )
        .await;
    }

    #[tokio::test]
    async fn octet_stream_cid_policy_migration_invalidates_only_cached_html() {
        assert_html_cache_invalidation_migration(
            include_str!("../../migrations/0022_octet_stream_cid_fidelity.sql"),
            "21",
            "22",
        )
        .await;
    }

    #[tokio::test]
    async fn bmp_inline_image_policy_migration_invalidates_only_cached_html() {
        assert_html_cache_invalidation_migration(
            include_str!("../../migrations/0023_bmp_inline_image_fidelity.sql"),
            "22",
            "23",
        )
        .await;
    }

    #[tokio::test]
    async fn selective_cid_refresh_invalidates_only_mislabeled_image_candidates() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                remote_images_blocked INTEGER NOT NULL,
                revision INTEGER NOT NULL
             );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT,
                safe_html TEXT
             );
             CREATE TABLE attachments(
                message_id TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content_id TEXT
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata VALUES ('data_format_version', '27');
             INSERT INTO messages VALUES ('mislabeled', 'available', 1, 4);
             INSERT INTO messages VALUES ('ordinary', 'available', 1, 7);
             INSERT INTO message_bodies VALUES ('mislabeled', NULL, '<img>');
             INSERT INTO message_bodies VALUES ('ordinary', NULL, '<p>body</p>');
             INSERT INTO attachments VALUES (
                'mislabeled', 'application/octet-stream', 'logo@example.test'
             );
             INSERT INTO attachments VALUES (
                'ordinary', 'application/pdf', 'report@example.test'
             );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0028_selective_cid_body_refresh.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT body_availability, remote_images_blocked, revision \
                 FROM messages WHERE id = 'mislabeled'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("missing".to_owned(), 0, 5)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'mislabeled'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_as::<_, (String, i64)>(
                "SELECT body_availability, revision FROM messages WHERE id = 'ordinary'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("available".to_owned(), 7)
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "28"
        );
    }

    #[tokio::test]
    async fn attachment_filename_refresh_invalidates_only_encoded_name_candidates() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                body_availability TEXT NOT NULL,
                revision INTEGER NOT NULL
             );
             CREATE TABLE message_bodies(message_id TEXT PRIMARY KEY, safe_html TEXT);
             CREATE TABLE attachments(message_id TEXT NOT NULL, file_name TEXT NOT NULL);
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata VALUES ('data_format_version', '28');
             INSERT INTO messages VALUES ('encoded', 'available', 4);
             INSERT INTO messages VALUES ('ordinary', 'available', 7);
             INSERT INTO message_bodies VALUES ('encoded', '<p>body</p>');
             INSERT INTO message_bodies VALUES ('ordinary', '<p>body</p>');
             INSERT INTO attachments VALUES ('encoded', '=?utf-8?B?5rWZ5rGfLmRvY3g=?=');
             INSERT INTO attachments VALUES ('ordinary', 'report.docx');",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0029_attachment_filename_refresh.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_as::<_, (String, i64)>(
                "SELECT body_availability, revision FROM messages WHERE id = 'encoded'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("missing".to_owned(), 5)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'encoded'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_as::<_, (String, i64)>(
                "SELECT body_availability, revision FROM messages WHERE id = 'ordinary'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("available".to_owned(), 7)
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "29"
        );
    }

    #[tokio::test]
    async fn local_search_migration_backfills_existing_searchable_content() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE messages(
                id TEXT PRIMARY KEY,
                account_slot_id TEXT NOT NULL,
                subject TEXT NOT NULL,
                from_json TEXT NOT NULL,
                to_json TEXT NOT NULL,
                cc_json TEXT NOT NULL,
                preview TEXT NOT NULL
             );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT
             );
             CREATE TABLE attachments(
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                file_name TEXT NOT NULL
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '14');
             INSERT INTO messages VALUES (
                'legacy-message', 'slot', 'Legacy subject',
                '[{\"name\":\"Alice\",\"email\":\"alice@example.com\"}]', '[]', '[]',
                'Legacy preview'
             );
             INSERT INTO message_bodies VALUES ('legacy-message', 'Legacy offline body');
             INSERT INTO attachments VALUES (
                'legacy-attachment', 'legacy-message', 'legacy-report.pdf'
             );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../../migrations/0015_local_message_search.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        for query in [
            "\"Legacy subject\"",
            "\"Alice\"",
            "\"offline body\"",
            "\"report.pdf\"",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM message_search WHERE message_search MATCH ?"
                )
                .bind(query)
                .fetch_one(&pool)
                .await
                .unwrap(),
                1
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "15"
        );
    }

    #[tokio::test]
    async fn rebuilt_message_bodies_are_written_atomically_with_account_isolation() {
        let (_directory, repository, mailbox) = repository_with_mailbox(1).await;
        let mut remote = remote_message(1, 1, "Cached");
        remote.attachments = vec![crate::core::RemoteAttachment {
            part_index: 0,
            imap_section: None,
            file_name: "inline.png".to_owned(),
            content_type: "image/png".to_owned(),
            size: 128,
            content_id: Some("Logo@Example.Test".to_owned()),
        }];
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote)
            .await
            .unwrap();
        let message = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 20)
            .await
            .unwrap()
            .items
            .remove(0);

        let error = repository
            .sync_sink()
            .replace_message_body(
                "another-slot",
                &message.id,
                &crate::core::RemoteMessageBody {
                    plain_text: Some("wrong account".to_owned()),
                    safe_html: Some("<p>wrong account</p>".to_owned()),
                    preview: None,
                    attachments: Vec::new(),
                    remote_images_blocked: false,
                    inline_content_ids: Vec::new(),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "message.not_found");

        repository
            .sync_sink()
            .replace_message_body(
                "slot",
                &message.id,
                &crate::core::RemoteMessageBody {
                    plain_text: Some("offline body".to_owned()),
                    safe_html: Some("<p>offline body</p>".to_owned()),
                    preview: None,
                    attachments: Vec::new(),
                    remote_images_blocked: true,
                    inline_content_ids: vec!["logo@example.test".to_owned()],
                },
            )
            .await
            .unwrap();
        let detail = repository
            .read()
            .get_message_detail("slot", &message.id, Some(&mailbox.id))
            .await
            .unwrap();
        assert_eq!(detail.plain_text.as_deref(), Some("offline body"));
        assert!(detail.attachments.is_empty());
        assert_eq!(detail.safe_html.as_deref(), Some("<p>offline body</p>"));
        assert!(detail.remote_images_blocked);
        assert_eq!(detail.body_availability, ContentAvailability::Available);
    }

    #[tokio::test]
    async fn local_search_indexes_message_content_with_mailbox_and_account_isolation() {
        let (directory, repository, inbox) = repository_with_mailbox(7).await;
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let archive = repository
            .sync_sink()
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "Archive".to_owned(),
                    display_name: "Archive".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Archive,
                    selectable: true,
                    uid_validity: 8,
                    uid_next: 2,
                    total_count: 1,
                    unread_count: 0,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();
        let private_inbox = repository
            .sync_sink()
            .upsert_mailbox(
                "slot-b",
                &RemoteMailbox {
                    name: "INBOX".to_owned(),
                    display_name: "INBOX".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Inbox,
                    selectable: true,
                    uid_validity: 9,
                    uid_next: 2,
                    total_count: 1,
                    unread_count: 1,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();

        let mut first = remote_message(1, 7, "Quarterly roadmap");
        first.received_at = 100;
        first.from = vec![MessageAddress {
            name: Some("Alice Example".to_owned()),
            email: "alice@example.com".to_owned(),
        }];
        first.to = vec![MessageAddress {
            name: Some("Bob".to_owned()),
            email: "bob@example.com".to_owned(),
        }];
        first.preview = "Finance update".to_owned();
        first.plain_text = Some("请核对电子发票和本地正文索引".to_owned());
        first.attachments = vec![crate::core::RemoteAttachment {
            part_index: 1,
            imap_section: None,
            file_name: "financial-report.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            size: 42,
            content_id: None,
        }];
        repository
            .sync_sink()
            .upsert_message("slot", &inbox.id, &first)
            .await
            .unwrap();

        let mut second = remote_message(2, 7, "Quarterly follow-up");
        second.received_at = 200;
        repository
            .sync_sink()
            .upsert_message("slot", &inbox.id, &second)
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message("slot", &archive.id, &remote_message(1, 8, "Archive secret"))
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message(
                "slot-b",
                &private_inbox.id,
                &remote_message(1, 9, "Private account message"),
            )
            .await
            .unwrap();

        for query in [
            "Alice Example",
            "alice@example.com",
            "电子发票",
            "发票",
            "report.pdf",
        ] {
            let page = repository
                .read()
                .search_messages("slot", &inbox.id, query, None, 20)
                .await
                .unwrap();
            assert_eq!(page.items.len(), 1, "query {query:?} must find the message");
            assert_eq!(page.items[0].subject, "Quarterly roadmap");
        }

        let first_page = repository
            .read()
            .search_messages("slot", &inbox.id, "Quarterly", None, 1)
            .await
            .unwrap();
        assert_eq!(first_page.items[0].subject, "Quarterly follow-up");
        let second_page = repository
            .read()
            .search_messages(
                "slot",
                &inbox.id,
                "Quarterly",
                first_page.next_cursor.as_deref(),
                1,
            )
            .await
            .unwrap();
        assert_eq!(second_page.items[0].subject, "Quarterly roadmap");
        assert!(second_page.next_cursor.is_none());

        assert!(repository
            .read()
            .search_messages("slot", &inbox.id, "Archive secret", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        assert!(repository
            .read()
            .search_messages("slot", &inbox.id, "Alice OR Private", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        assert!(repository
            .read()
            .search_messages("slot", &inbox.id, "Alice\"", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        assert!(repository
            .read()
            .search_messages("slot", &private_inbox.id, "Private account", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        assert_eq!(
            repository
                .read()
                .search_messages("slot-b", &private_inbox.id, "Private account", None, 20)
                .await
                .unwrap()
                .items
                .len(),
            1
        );

        let first_id = repository
            .read()
            .search_messages("slot", &inbox.id, "Alice Example", None, 20)
            .await
            .unwrap()
            .items
            .remove(0)
            .id;
        repository
            .sync_sink()
            .replace_message_body(
                "slot",
                &first_id,
                &crate::core::RemoteMessageBody {
                    plain_text: Some("replacement searchable content".to_owned()),
                    safe_html: None,
                    preview: None,
                    attachments: Vec::new(),
                    remote_images_blocked: false,
                    inline_content_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert!(repository
            .read()
            .search_messages("slot", &inbox.id, "电子发票", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        assert_eq!(
            repository
                .read()
                .search_messages("slot", &inbox.id, "searchable content", None, 20)
                .await
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn account_sync_interval_defaults_to_one_minute_and_round_trips() {
        let (_directory, repository, _mailbox) = repository_with_mailbox(1).await;
        let read = repository.read();

        assert_eq!(
            read.get_sync_interval("slot").await.unwrap(),
            SyncInterval::Minutes1
        );
        for interval in [
            SyncInterval::Manual,
            SyncInterval::Minutes1,
            SyncInterval::Minutes5,
            SyncInterval::Minutes10,
        ] {
            assert_eq!(
                read.set_sync_interval("slot", interval.clone())
                    .await
                    .unwrap(),
                interval
            );
            assert_eq!(read.get_sync_interval("slot").await.unwrap(), interval);
        }
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
            )
            .fetch_one(&repository.pool)
            .await
            .unwrap(),
            "29"
        );
    }

    #[tokio::test]
    async fn full_message_sync_defaults_off_and_round_trips() {
        let (_directory, repository, _mailbox) = repository_with_mailbox(1).await;
        let read = repository.read();

        assert!(!read.get_download_full_messages("slot").await.unwrap());
        assert!(read.set_download_full_messages("slot", true).await.unwrap());
        assert!(read.get_download_full_messages("slot").await.unwrap());
        assert!(!read
            .set_download_full_messages("slot", false)
            .await
            .unwrap());
        assert!(!read.get_download_full_messages("slot").await.unwrap());
    }

    async fn repository_with_mailbox(
        uid_validity: u32,
    ) -> (tempfile::TempDir, MailRepository, StoredMailbox) {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let mailbox = repository
            .sync_sink()
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "INBOX".to_owned(),
                    display_name: "INBOX".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Inbox,
                    selectable: true,
                    uid_validity,
                    uid_next: 3,
                    total_count: 2,
                    unread_count: 2,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();
        (directory, repository, mailbox)
    }

    #[tokio::test]
    async fn notification_baseline_and_message_upsert_are_durable() {
        let (_directory, repository, mailbox) = repository_with_mailbox(11).await;
        assert!(mailbox.notification_baseline_required);
        assert!(!repository
            .read()
            .notification_baseline_ready("slot")
            .await
            .unwrap());

        let first = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 11, "First"))
            .await
            .unwrap();
        let duplicate = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 11, "First"))
            .await
            .unwrap();
        assert!(first.is_new_location);
        assert!(!duplicate.is_new_location);
        assert_eq!(first.message_id, duplicate.message_id);

        repository
            .sync_sink()
            .complete_notification_baseline("slot")
            .await
            .unwrap();
        assert!(repository
            .read()
            .notification_baseline_ready("slot")
            .await
            .unwrap());
        let existing_mailbox = repository
            .sync_sink()
            .upsert_mailbox(
                "slot",
                &RemoteMailbox {
                    name: "INBOX".to_owned(),
                    display_name: "INBOX".to_owned(),
                    delimiter: Some("/".to_owned()),
                    role: MailboxRole::Inbox,
                    selectable: true,
                    uid_validity: 11,
                    uid_next: 3,
                    total_count: 2,
                    unread_count: 1,
                    highest_modseq: None,
                },
            )
            .await
            .unwrap();
        assert!(!existing_mailbox.notification_baseline_required);
    }

    #[tokio::test]
    async fn synchronized_contacts_are_account_scoped_and_override_header_names() {
        let (directory, repository, mailbox) = repository_with_mailbox(11).await;
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let mut message = remote_message(1, 11, "Contact identity");
        message.from = vec![MessageAddress {
            name: Some("Header Alias".to_owned()),
            email: "Alice@Example.COM".to_owned(),
        }];
        message.to = vec![MessageAddress {
            name: None,
            email: "bob@example.com".to_owned(),
        }];
        message.contact_addresses = vec![
            RemoteContactAddress {
                role: ContactAddressRole::From,
                address: message.from[0].clone(),
            },
            RemoteContactAddress {
                role: ContactAddressRole::To,
                address: message.to[0].clone(),
            },
        ];

        let inserted = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap();
        assert!(inserted.contacts_changed);
        let duplicate = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap();
        assert!(!duplicate.contacts_changed);

        let contacts = repository
            .contacts()
            .list_contacts("slot", "", None, 20)
            .await
            .unwrap();
        assert_eq!(contacts.total, 2);
        assert!(repository
            .contacts()
            .list_contacts("slot-b", "", None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
        let alice = contacts
            .items
            .into_iter()
            .find(|contact| contact.email.eq_ignore_ascii_case("alice@example.com"))
            .unwrap();
        assert_eq!(alice.name, "Header Alias");
        let mut renamed_message = message.clone();
        renamed_message.from[0].name = Some("Changed Header".to_owned());
        renamed_message.contact_addresses[0].address.name = Some("Changed Header".to_owned());
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &renamed_message)
            .await
            .unwrap();
        assert_eq!(
            repository
                .contacts()
                .get_contact_summary("slot", &alice.id)
                .await
                .unwrap()
                .name,
            "Header Alias"
        );
        let alice = repository
            .contacts()
            .update_contact_name("slot", &alice.id, "Alice Local", alice.revision)
            .await
            .unwrap();
        assert_eq!(alice.name, "Alice Local");
        assert_eq!(
            repository
                .contacts()
                .create_contact(
                    "slot",
                    &ContactDraft {
                        name: "Duplicate".to_owned(),
                        email: "alice@example.com".to_owned(),
                    },
                )
                .await
                .unwrap_err()
                .code,
            "contact.already_exists"
        );

        let listed_message = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 20)
            .await
            .unwrap()
            .items
            .remove(0);
        assert_eq!(listed_message.from[0].name.as_deref(), Some("Alice Local"));
        assert_eq!(
            listed_message.from[0].header_name.as_deref(),
            Some("Changed Header")
        );
        assert_eq!(
            listed_message.from[0].contact_id.as_deref(),
            Some(alice.id.as_str())
        );

        let detail = repository
            .contacts()
            .get_contact_detail("slot", &alice.id, 20)
            .await
            .unwrap();
        assert_eq!(detail.recent_messages.len(), 1);
        assert_eq!(detail.recent_messages[0].subject, "Contact identity");

        assert_eq!(
            repository
                .contacts()
                .delete_contacts("slot-b", std::slice::from_ref(&alice.id))
                .await
                .unwrap_err()
                .code,
            "contact.not_found"
        );
        repository
            .contacts()
            .delete_contacts("slot", std::slice::from_ref(&alice.id))
            .await
            .unwrap();
        assert!(repository
            .contacts()
            .get_contact_summary("slot", &alice.id)
            .await
            .is_err());
        let recreated = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &renamed_message)
            .await
            .unwrap();
        assert!(recreated.contacts_changed);
        assert_eq!(
            repository
                .contacts()
                .list_contacts("slot", "alice@example.com", None, 20)
                .await
                .unwrap()
                .items[0]
                .name,
            "Changed Header"
        );
    }

    #[tokio::test]
    async fn contact_backfill_indexes_messages_stored_before_contact_support() {
        let (_directory, repository, mailbox) = repository_with_mailbox(12).await;
        let mut message = remote_message(1, 12, "Historical contact");
        message.from = vec![MessageAddress {
            name: Some("Historical Header".to_owned()),
            email: "history@example.com".to_owned(),
        }];
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap();
        assert_eq!(
            repository
                .contacts()
                .list_contacts("slot", "", None, 20)
                .await
                .unwrap()
                .total,
            0
        );

        let batch = repository
            .contacts()
            .backfill_next_batch("slot")
            .await
            .unwrap();
        assert_eq!(batch.processed, 1);
        assert!(batch.changed);
        assert!(batch.complete);
        let contacts = repository
            .contacts()
            .list_contacts("slot", "history", None, 20)
            .await
            .unwrap();
        assert_eq!(contacts.total, 1);
        assert_eq!(contacts.items[0].name, "Historical Header");
        assert!(
            repository
                .contacts()
                .backfill_next_batch("slot")
                .await
                .unwrap()
                .complete
        );
    }

    #[tokio::test]
    async fn contact_list_search_and_cursor_stay_bounded_with_ten_thousand_rows() {
        let (_directory, repository, _mailbox) = repository_with_mailbox(13).await;
        sqlx::raw_sql(
            "WITH digits(value) AS (
                VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
             ), numbered(value) AS (
                SELECT a.value * 1000 + b.value * 100 + c.value * 10 + d.value
                FROM digits a CROSS JOIN digits b CROSS JOIN digits c CROSS JOIN digits d
             )
             INSERT INTO contacts(
                id, account_slot_id, normalized_email, email, name, name_source,
                created_at, updated_at, revision
             )
             SELECT printf('contact-%05d', value), 'slot',
                    printf('person-%05d@example.com', value),
                    printf('person-%05d@example.com', value),
                    printf('Person %05d', value), 'auto', 1, 1, 1
             FROM numbered;",
        )
        .execute(&repository.pool)
        .await
        .unwrap();

        let first = repository
            .contacts()
            .list_contacts("slot", "", None, 100)
            .await
            .unwrap();
        assert_eq!(first.total, 10_000);
        assert_eq!(first.items.len(), 100);
        assert_eq!(first.items[0].name, "Person 00000");
        let second = repository
            .contacts()
            .list_contacts("slot", "", first.next_cursor.as_deref(), 100)
            .await
            .unwrap();
        assert_eq!(second.items.len(), 100);
        assert_eq!(second.items[0].name, "Person 00100");
        let search = repository
            .contacts()
            .list_contacts("slot", "09999", None, 20)
            .await
            .unwrap();
        assert_eq!(search.total, 1);
        assert_eq!(search.items[0].email, "person-09999@example.com");
    }

    fn remote_message(uid: u32, uid_validity: u32, subject: &str) -> RemoteMessage {
        RemoteMessage {
            uid,
            uid_validity,
            subject: subject.to_owned(),
            from: vec![],
            to: vec![],
            cc: vec![],
            contact_addresses: vec![],
            received_at: i64::from(uid),
            preview: "body".to_owned(),
            unread: true,
            flagged: false,
            size: 20,
            message_id: Some(format!("message-{uid}@example.com")),
            references: vec![],
            in_reply_to: None,
            plain_text: Some("body".to_owned()),
            safe_html: None,
            raw: None,
            attachments: vec![],
            remote_images_blocked: false,
            modseq: None,
        }
    }

    #[tokio::test]
    async fn migration_and_mailbox_round_trip_work() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot", 1)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let mailbox = repository
            .sync_sink()
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
            .sync_sink()
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
                    contact_addresses: vec![],
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
        let mailboxes = repository
            .read()
            .list_mailboxes("account", "slot")
            .await
            .unwrap();
        assert_eq!(mailboxes.len(), 1);
        assert_eq!(mailboxes[0].role, MailboxRole::Inbox);

        let page = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        let detail = repository
            .read()
            .get_message_detail("slot", &page.items[0].id, Some(&mailbox.id))
            .await
            .unwrap();
        assert_eq!(detail.plain_text.as_deref(), Some("Hello from disk"));
        assert!(repository
            .read()
            .raw_message("slot", &detail.id)
            .await
            .unwrap()
            .is_some());
        let context = repository
            .read()
            .remote_message_context("slot", &detail.id)
            .await
            .unwrap();
        assert_eq!(context.mailbox_name, "INBOX");
        assert_eq!(context.uid, 1);

        repository
            .sync_sink()
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
                    contact_addresses: vec![],
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
            .sync_sink()
            .pending_body_locations(&mailbox.id, Some(15))
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].uid, 2);
        let header_only = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.subject == "Header only")
            .unwrap();
        assert!(repository
            .read()
            .raw_message("slot", &header_only.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn attachment_content_cannot_be_written_through_another_account_slot() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot-a", 1)
            .await
            .unwrap();
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        sqlx::query(
            "INSERT INTO messages(id, account_slot_id, subject, received_at) \
             VALUES ('message-a', 'slot-a', 'Private', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO attachments(id, message_id, part_index, file_name, content_type, size) \
             VALUES ('attachment-a', 'message-a', 1, 'private.txt', 'text/plain', 6)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();

        let error = repository
            .read()
            .store_attachment_content("slot-b", "attachment-a", b"secret")
            .await
            .unwrap_err();
        assert_eq!(error.code, "attachment.not_found");
        repository
            .read()
            .store_attachment_content("slot-a", "attachment-a", b"secret")
            .await
            .unwrap();
        let error = repository
            .read()
            .prepare_attachment_file("slot-b", "attachment-a")
            .await
            .unwrap_err();
        assert_eq!(error.code, "attachment.not_found");
        let prepared = repository
            .read()
            .prepare_attachment_file("slot-a", "attachment-a")
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(prepared.path).await.unwrap(), b"secret");
        let availability: String =
            sqlx::query_scalar("SELECT availability FROM attachments WHERE id = 'attachment-a'")
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(availability, "available");
    }

    #[tokio::test]
    async fn upsert_message_writes_multiple_attachments_in_one_atomic_unit() {
        let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
        let mut message = remote_message(1, 7, "Attachments");
        message.attachments = vec![
            crate::core::RemoteAttachment {
                part_index: 1,
                imap_section: None,
                file_name: "one.txt".to_owned(),
                content_type: "text/plain".to_owned(),
                size: 3,
                content_id: None,
            },
            crate::core::RemoteAttachment {
                part_index: 2,
                imap_section: Some("2".to_owned()),
                file_name: "two.txt".to_owned(),
                content_type: "text/plain".to_owned(),
                size: 3,
                content_id: Some("part-two".to_owned()),
            },
        ];

        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap();

        let attachment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachments")
            .fetch_one(&repository.pool)
            .await
            .unwrap();
        assert_eq!(attachment_count, 2);
        let section: Option<String> =
            sqlx::query_scalar("SELECT imap_section FROM attachments WHERE part_index = 2")
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(section.as_deref(), Some("2"));

        sqlx::query(
            "UPDATE attachments SET availability = 'available', size = 2 WHERE part_index = 2",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        message.attachments[1].size = 4;
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap();
        let size: i64 = sqlx::query_scalar("SELECT size FROM attachments WHERE part_index = 2")
            .fetch_one(&repository.pool)
            .await
            .unwrap();
        assert_eq!(size, 2);
    }

    #[tokio::test]
    async fn stored_uids_returns_every_stored_uid_for_the_mailbox_validity() {
        let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "First"))
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(3, 7, "Third"))
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(5, 7, "Fifth"))
            .await
            .unwrap();

        let mut uids = repository
            .sync_sink()
            .stored_uids(&mailbox.id, 7)
            .await
            .unwrap();
        uids.sort_unstable();
        assert_eq!(uids, vec![1, 3, 5]);

        // Locations are uid_validity-scoped: a different validity yields nothing,
        // which is what makes the resumable-sync diff correct after a reset.
        assert!(repository
            .sync_sink()
            .stored_uids(&mailbox.id, 99)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn upsert_message_preserves_pending_read_flag_against_stale_server_state() {
        let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
        let outcome = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
            .await
            .unwrap();
        assert!(outcome.is_new_location);

        // Opening the message locally marks it read and queues a pending set_read.
        repository
            .operations()
            .queue_set_read(
                "slot",
                &mailbox.id,
                std::slice::from_ref(&outcome.message_id),
                true,
            )
            .await
            .unwrap();

        // A later body fetch re-upserts the message carrying the server's stale
        // (still-unread) flags. This must not clobber the pending read.
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
            .await
            .unwrap();
        let item = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap()
            .items
            .remove(0);
        assert!(!item.unread, "pending set_read must survive a stale upsert");

        // With no pending op in flight, upsert flags apply normally again.
        sqlx::query("DELETE FROM pending_operations")
            .execute(&repository.pool)
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
            .await
            .unwrap();
        let item = repository
            .read()
            .list_messages("slot", &mailbox.id, None, 50)
            .await
            .unwrap()
            .items
            .remove(0);
        assert!(
            item.unread,
            "without a pending op the upsert flag is applied"
        );
    }

    #[tokio::test]
    async fn upsert_message_rolls_back_all_database_rows_when_attachment_write_fails() {
        let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
        sqlx::query(
            "CREATE TRIGGER fail_attachment_insert BEFORE INSERT ON attachments \
             BEGIN SELECT RAISE(FAIL, 'forced attachment failure'); END",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        let mut message = remote_message(1, 7, "Atomic");
        message.attachments = vec![crate::core::RemoteAttachment {
            part_index: 1,
            imap_section: None,
            file_name: "failure.txt".to_owned(),
            content_type: "text/plain".to_owned(),
            size: 7,
            content_id: None,
        }];

        let error = repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &message)
            .await
            .unwrap_err();
        assert_eq!(error.code, "storage.attachment_write_failed");

        for (table, query) in [
            ("messages", "SELECT COUNT(*) FROM messages"),
            (
                "message_locations",
                "SELECT COUNT(*) FROM message_locations",
            ),
            ("message_bodies", "SELECT COUNT(*) FROM message_bodies"),
            ("attachments", "SELECT COUNT(*) FROM attachments"),
        ] {
            let count: i64 = sqlx::query_scalar(query)
                .fetch_one(&repository.pool)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table} must not retain a partial upsert");
        }
    }

    #[tokio::test]
    async fn reconcile_mailbox_deletes_missing_locations_but_preserves_pending_work() {
        let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Pending"))
            .await
            .unwrap();
        repository
            .sync_sink()
            .upsert_message("slot", &mailbox.id, &remote_message(2, 7, "Removed"))
            .await
            .unwrap();
        let pending_message_id: String = sqlx::query_scalar(
            "SELECT message_id FROM message_locations WHERE mailbox_id = ? AND uid = 1",
        )
        .bind(&mailbox.id)
        .fetch_one(&repository.pool)
        .await
        .unwrap();
        repository
            .operations()
            .queue_set_read(
                "slot",
                &mailbox.id,
                std::slice::from_ref(&pending_message_id),
                true,
            )
            .await
            .unwrap();

        repository
            .sync_sink()
            .reconcile_mailbox(&mailbox.id, 7, Some(12), &[])
            .await
            .unwrap();

        let remaining_uids = sqlx::query_scalar::<_, i64>(
            "SELECT uid FROM message_locations WHERE mailbox_id = ? ORDER BY uid",
        )
        .bind(&mailbox.id)
        .fetch_all(&repository.pool)
        .await
        .unwrap();
        assert_eq!(remaining_uids, vec![1]);
    }
}

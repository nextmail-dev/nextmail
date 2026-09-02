use crate::core::{
    AddressPresentation, AttachmentSummary, CommandError, CommandResult, ContentAvailability,
    MailboxRole, MailboxSummary, MessageAddress, MessageDetail, MessageListItem, MessageListPage,
    SyncInterval,
};
use sqlx::{FromRow, Row};

use super::{
    map_storage_err, normalize_email, now,
    repository::{MailReadRepository, RemoteMessageContext},
    storage_read_error, ContactIdentity, ContactRepository, PreparedAttachmentFile,
};

#[derive(FromRow)]
struct MessageDetailRow {
    id: String,
    subject: String,
    from_json: String,
    to_json: String,
    cc_json: String,
    received_at: i64,
    high_priority: i64,
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
        high_priority: message.high_priority != 0,
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
        high_priority: row
            .try_get::<i64, _>("high_priority")
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
                    b.total_count, b.unread_count, b.is_favorite, b.revision \
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
                    is_favorite: row
                        .try_get::<i64, _>("is_favorite")
                        .map_err(storage_read_error)?
                        != 0,
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
            "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, m.high_priority, \
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

    pub async fn list_unread_messages(
        &self,
        account_slot_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        self.list_marked_messages(account_slot_id, cursor, limit, false)
            .await
    }

    pub async fn list_starred_messages(
        &self,
        account_slot_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        self.list_marked_messages(account_slot_id, cursor, limit, true)
            .await
    }

    async fn list_marked_messages(
        &self,
        account_slot_id: &str,
        cursor: Option<&str>,
        limit: u32,
        starred: bool,
    ) -> CommandResult<MessageListPage> {
        let limit = limit.clamp(1, 100);
        let (cursor_date, cursor_id) = cursor.and_then(parse_cursor).unzip();
        let rows = sqlx::query(
            "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, m.high_priority, \
                    l.unread, l.flagged, m.has_attachments, m.body_availability, \
                    EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                      AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
             FROM message_locations l JOIN messages m ON m.id = l.message_id \
             JOIN mailboxes b ON b.id = l.mailbox_id \
             WHERE b.account_slot_id = ? AND m.account_slot_id = ? \
               AND l.local_hidden = 0 AND CASE WHEN ? THEN l.flagged ELSE l.unread END = 1 AND \
               (? IS NULL OR l.internal_date < ? OR (l.internal_date = ? AND m.id < ?)) \
             ORDER BY l.internal_date DESC, m.id DESC LIMIT ?",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(starred)
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
        let next_cursor = has_more
            .then(|| {
                items
                    .last()
                    .map(|item| format!("{}:{}", item.received_at, item.id))
            })
            .flatten();
        self.resolve_message_items(account_slot_id, &mut items)
            .await?;
        Ok(MessageListPage { items, next_cursor })
    }

    pub async fn search_messages(
        &self,
        account_slot_id: &str,
        mailbox_id: Option<&str>,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        let query = query.trim();
        if query.is_empty() {
            return match mailbox_id {
                Some(mailbox_id) => {
                    self.list_messages(account_slot_id, mailbox_id, cursor, limit)
                        .await
                }
                None => Ok(MessageListPage {
                    items: Vec::new(),
                    next_cursor: None,
                }),
            };
        }

        let limit = limit.clamp(1, 100);
        let (cursor_date, cursor_id) = cursor.and_then(parse_cursor).unzip();
        let rows = match (mailbox_id, query.chars().count() < 3) {
            (Some(mailbox_id), true) => {
                let query = query.to_ascii_lowercase();
                sqlx::query(
                "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, m.high_priority, \
                        l.unread, l.flagged, m.has_attachments, m.body_availability, \
                        EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                          AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
                 FROM message_locations l INDEXED BY idx_locations_mailbox_date \
                 JOIN messages m ON m.id = l.message_id \
                 JOIN mailboxes b ON b.id = l.mailbox_id \
                 LEFT JOIN message_bodies body ON body.message_id = m.id \
                 WHERE l.mailbox_id = ? AND b.account_slot_id = ? AND m.account_slot_id = ? \
                   AND l.local_hidden = 0 \
                   AND (instr(lower(m.subject), ?) > 0 \
                     OR instr(lower(m.from_json || ' ' || m.to_json || ' ' || m.cc_json), ?) > 0 \
                     OR instr(lower(COALESCE(body.plain_text, '')), ?) > 0) \
                   AND (? IS NULL OR l.internal_date < ? OR (l.internal_date = ? AND l.message_id < ?)) \
                 ORDER BY l.internal_date DESC, l.message_id DESC LIMIT ?",
                )
                .bind(mailbox_id)
                .bind(account_slot_id)
                .bind(account_slot_id)
                .bind(&query)
                .bind(&query)
                .bind(&query)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_id.as_deref())
                .bind(i64::from(limit) + 1)
                .fetch_all(&self.pool)
                .await
            }
            (None, true) => {
                let query = query.to_ascii_lowercase();
                sqlx::query(
                    "SELECT m.id, l.mailbox_id, m.subject, m.from_json, m.received_at AS internal_date, m.preview, m.high_priority, \
                            l.unread, l.flagged, m.has_attachments, m.body_availability, \
                            EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                              AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
                     FROM messages m INDEXED BY idx_messages_account_received \
                     LEFT JOIN message_bodies body ON body.message_id = m.id \
                     JOIN message_locations l ON l.id = ( \
                       SELECT l2.id FROM message_locations l2 INDEXED BY idx_locations_message_date \
                       JOIN mailboxes b2 ON b2.id = l2.mailbox_id \
                       WHERE l2.message_id = m.id AND b2.account_slot_id = ? AND b2.selectable = 1 \
                         AND l2.local_hidden = 0 \
                       ORDER BY l2.internal_date DESC, l2.mailbox_id DESC LIMIT 1 \
                     ) \
                     WHERE m.account_slot_id = ? \
                       AND (instr(lower(m.subject), ?) > 0 \
                         OR instr(lower(m.from_json || ' ' || m.to_json || ' ' || m.cc_json), ?) > 0 \
                         OR instr(lower(COALESCE(body.plain_text, '')), ?) > 0) \
                       AND (? IS NULL OR m.received_at < ? OR (m.received_at = ? AND m.id < ?)) \
                     ORDER BY m.received_at DESC, m.id DESC LIMIT ?",
                )
                .bind(account_slot_id)
                .bind(account_slot_id)
                .bind(&query)
                .bind(&query)
                .bind(&query)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_id.as_deref())
                .bind(i64::from(limit) + 1)
                .fetch_all(&self.pool)
                .await
            }
            (Some(mailbox_id), false) => {
                let escaped = query.replace('"', "\"\"");
                let literal_query = format!(
                    "subject : \"{escaped}\" OR addresses : \"{escaped}\" OR body : \"{escaped}\""
                );
                sqlx::query(
                "SELECT m.id, l.mailbox_id, m.subject, m.from_json, l.internal_date, m.preview, m.high_priority, \
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
            (None, false) => {
                let escaped = query.replace('"', "\"\"");
                let literal_query = format!(
                    "subject : \"{escaped}\" OR addresses : \"{escaped}\" OR body : \"{escaped}\""
                );
                sqlx::query(
                    "SELECT m.id, l.mailbox_id, m.subject, m.from_json, m.received_at AS internal_date, m.preview, m.high_priority, \
                            l.unread, l.flagged, m.has_attachments, m.body_availability, \
                            EXISTS(SELECT 1 FROM pending_operations o WHERE o.message_id = m.id \
                              AND o.source_mailbox_id = l.mailbox_id AND o.status IN ('queued','running','retry_wait')) AS pending_operation \
                     FROM message_search JOIN messages m ON m.id = message_search.message_id \
                     JOIN message_locations l ON l.id = ( \
                       SELECT l2.id FROM message_locations l2 INDEXED BY idx_locations_message_date \
                       JOIN mailboxes b2 ON b2.id = l2.mailbox_id \
                       WHERE l2.message_id = m.id AND b2.account_slot_id = ? AND b2.selectable = 1 \
                         AND l2.local_hidden = 0 \
                       ORDER BY l2.internal_date DESC, l2.mailbox_id DESC LIMIT 1 \
                     ) \
                     WHERE message_search MATCH ? AND message_search.account_slot_id = ? \
                       AND m.account_slot_id = ? \
                       AND (? IS NULL OR m.received_at < ? OR (m.received_at = ? AND m.id < ?)) \
                     ORDER BY m.received_at DESC, m.id DESC LIMIT ?",
                )
                .bind(account_slot_id)
                .bind(literal_query)
                .bind(account_slot_id)
                .bind(account_slot_id)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_date)
                .bind(cursor_id.as_deref())
                .bind(i64::from(limit) + 1)
                .fetch_all(&self.pool)
                .await
            }
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

    pub async fn message_subject(
        &self,
        account_slot_id: &str,
        message_id: &str,
    ) -> CommandResult<String> {
        Ok(self
            .message_detail_row(account_slot_id, message_id)
            .await?
            .subject)
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
            "SELECT id, subject, from_json, to_json, cc_json, received_at, high_priority, body_availability, \
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

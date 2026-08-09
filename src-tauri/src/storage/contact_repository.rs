use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use lettre::Address;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::core::{
    AddressPresentation, CommandError, CommandResult, ContactAddressRole, ContactDetail,
    ContactDraft, ContactListPage, ContactRecentMessage, ContactSummary, MessageAddress,
    RemoteContactAddress,
};

use super::{map_storage_err, storage_read_error};

const CONTACT_NAME_MAX_CHARS: usize = 160;
const CONTACT_BACKFILL_BATCH_SIZE: u32 = 200;
const CONTACT_IDENTITY_QUERY_BATCH_SIZE: usize = 400;

#[derive(Clone)]
pub struct ContactRepository {
    pub(crate) pool: SqlitePool,
}

#[derive(Clone, Debug)]
pub(crate) struct ContactIdentity {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ContactBackfillBatch {
    pub processed: u32,
    pub changed: bool,
    pub complete: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct ContactCursor {
    name: String,
    email: String,
    id: String,
}

impl ContactRepository {
    pub async fn list_contacts(
        &self,
        account_slot_id: &str,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<ContactListPage> {
        let query = query.trim();
        let limit = limit.clamp(1, 100);
        let cursor = cursor.and_then(parse_contact_cursor);
        let cursor_name = cursor.as_ref().map(|value| value.name.as_str());
        let cursor_email = cursor.as_ref().map(|value| value.email.as_str());
        let cursor_id = cursor.as_ref().map(|value| value.id.as_str());

        let rows = sqlx::query(
            "SELECT id, name, email, revision, created_at, updated_at FROM contacts \
             WHERE account_slot_id = ? \
               AND (? = '' OR instr(lower(name), lower(?)) > 0 OR instr(normalized_email, lower(?)) > 0) \
               AND (? IS NULL OR name COLLATE NOCASE > ? COLLATE NOCASE \
                 OR (name COLLATE NOCASE = ? COLLATE NOCASE AND normalized_email > ?) \
                 OR (name COLLATE NOCASE = ? COLLATE NOCASE AND normalized_email = ? AND id > ?)) \
             ORDER BY name COLLATE NOCASE, normalized_email, id LIMIT ?",
        )
        .bind(account_slot_id)
        .bind(query)
        .bind(query)
        .bind(query)
        .bind(cursor_name)
        .bind(cursor_name)
        .bind(cursor_name)
        .bind(cursor_email)
        .bind(cursor_name)
        .bind(cursor_email)
        .bind(cursor_id)
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.contacts_read_failed"))?;

        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM contacts WHERE account_slot_id = ? \
             AND (? = '' OR instr(lower(name), lower(?)) > 0 OR instr(normalized_email, lower(?)) > 0)",
        )
        .bind(account_slot_id)
        .bind(query)
        .bind(query)
        .bind(query)
        .fetch_one(&self.pool)
        .await
        .map_err(map_storage_err("storage.contacts_read_failed"))? as u64;

        let has_more = rows.len() > limit as usize;
        let items = rows
            .into_iter()
            .take(limit as usize)
            .map(contact_summary_from_row)
            .collect::<CommandResult<Vec<_>>>()?;
        let next_cursor = if has_more {
            items.last().and_then(|contact| {
                serde_json::to_string(&ContactCursor {
                    name: contact.name.clone(),
                    email: normalize_email(&contact.email)?.1,
                    id: contact.id.clone(),
                })
                .ok()
            })
        } else {
            None
        };

        Ok(ContactListPage {
            items,
            next_cursor,
            total,
        })
    }

    pub async fn list_suggestions(
        &self,
        account_slot_id: &str,
        query: &str,
        limit: u32,
    ) -> CommandResult<Vec<ContactSummary>> {
        Ok(self
            .list_contacts(account_slot_id, query, None, limit.clamp(1, 20))
            .await?
            .items)
    }

    pub async fn get_contact_detail(
        &self,
        account_slot_id: &str,
        contact_id: &str,
        recent_limit: u32,
    ) -> CommandResult<ContactDetail> {
        let contact = self
            .get_contact_summary(account_slot_id, contact_id)
            .await?;
        let rows = sqlx::query(
            "SELECT m.id AS message_id, \
                (SELECT l.mailbox_id FROM message_locations l JOIN mailboxes b ON b.id = l.mailbox_id \
                 WHERE l.message_id = m.id AND b.account_slot_id = ? AND l.local_hidden = 0 \
                 ORDER BY l.internal_date DESC, l.id LIMIT 1) AS mailbox_id, \
                m.subject, \
                (SELECT MAX(l.internal_date) FROM message_locations l JOIN mailboxes b ON b.id = l.mailbox_id \
                 WHERE l.message_id = m.id AND b.account_slot_id = ? AND l.local_hidden = 0) AS received_at \
             FROM message_contacts mc JOIN messages m ON m.id = mc.message_id \
             WHERE mc.contact_id = ? AND m.account_slot_id = ? \
               AND EXISTS(SELECT 1 FROM message_locations l JOIN mailboxes b ON b.id = l.mailbox_id \
                          WHERE l.message_id = m.id AND b.account_slot_id = ? AND l.local_hidden = 0) \
             GROUP BY m.id ORDER BY received_at DESC, m.id DESC LIMIT ?",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(contact_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(i64::from(recent_limit.clamp(1, 50)))
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.contact_recent_read_failed"))?;
        let recent_messages = rows
            .into_iter()
            .map(|row| {
                Ok(ContactRecentMessage {
                    message_id: row.try_get("message_id").map_err(storage_read_error)?,
                    mailbox_id: row.try_get("mailbox_id").map_err(storage_read_error)?,
                    subject: row.try_get("subject").map_err(storage_read_error)?,
                    received_at: row.try_get("received_at").map_err(storage_read_error)?,
                })
            })
            .collect::<CommandResult<Vec<_>>>()?;
        Ok(ContactDetail {
            contact,
            recent_messages,
        })
    }

    pub async fn create_contact(
        &self,
        account_slot_id: &str,
        draft: &ContactDraft,
    ) -> CommandResult<ContactSummary> {
        let name = validate_contact_name(&draft.name)?;
        let (email, normalized_email) = normalize_email(&draft.email)
            .ok_or_else(|| CommandError::new("contact.email_invalid"))?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let result = sqlx::query(
            "INSERT INTO contacts(id, account_slot_id, normalized_email, email, name, name_source, created_at, updated_at, revision) \
             VALUES (?, ?, ?, ?, ?, 'manual', ?, ?, 1)",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(normalized_email)
        .bind(email)
        .bind(name)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await;
        match result {
            Ok(_) => self.get_contact_summary(account_slot_id, &id).await,
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                Err(CommandError::new("contact.already_exists"))
            }
            Err(error) => Err(map_storage_err("storage.contact_write_failed")(error)),
        }
    }

    pub async fn update_contact_name(
        &self,
        account_slot_id: &str,
        contact_id: &str,
        name: &str,
        expected_revision: u64,
    ) -> CommandResult<ContactSummary> {
        let name = validate_contact_name(name)?;
        let result = sqlx::query(
            "UPDATE contacts SET name = ?, name_source = 'manual', updated_at = ?, revision = revision + 1 \
             WHERE id = ? AND account_slot_id = ? AND revision = ?",
        )
        .bind(name)
        .bind(now())
        .bind(contact_id)
        .bind(account_slot_id)
        .bind(expected_revision as i64)
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("storage.contact_write_failed"))?;
        if result.rows_affected() == 0 {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM contacts WHERE id = ? AND account_slot_id = ?",
            )
            .bind(contact_id)
            .bind(account_slot_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_storage_err("storage.contacts_read_failed"))?;
            return Err(CommandError::new(if exists == 0 {
                "contact.not_found"
            } else {
                "contact.conflict"
            }));
        }
        self.get_contact_summary(account_slot_id, contact_id).await
    }

    pub async fn delete_contacts(
        &self,
        account_slot_id: &str,
        contact_ids: &[String],
    ) -> CommandResult<()> {
        let contact_ids = contact_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if contact_ids.is_empty() {
            return Ok(());
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("storage.contact_write_failed"))?;
        let mut found = 0_i64;
        for batch in contact_ids.chunks(CONTACT_IDENTITY_QUERY_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Sqlite>::new(
                "SELECT COUNT(*) FROM contacts WHERE account_slot_id = ",
            );
            builder.push_bind(account_slot_id).push(" AND id IN (");
            let mut separated = builder.separated(", ");
            for contact_id in batch {
                separated.push_bind(contact_id);
            }
            separated.push_unseparated(")");
            found += builder
                .build_query_scalar::<i64>()
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.contacts_read_failed"))?;
        }
        if found != contact_ids.len() as i64 {
            return Err(CommandError::new("contact.not_found"));
        }
        for batch in contact_ids.chunks(CONTACT_IDENTITY_QUERY_BATCH_SIZE) {
            let mut builder =
                QueryBuilder::<Sqlite>::new("DELETE FROM contacts WHERE account_slot_id = ");
            builder.push_bind(account_slot_id).push(" AND id IN (");
            let mut separated = builder.separated(", ");
            for contact_id in batch {
                separated.push_bind(contact_id);
            }
            separated.push_unseparated(")");
            builder
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.contact_write_failed"))?;
        }
        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.contact_write_failed"))?;
        Ok(())
    }

    pub async fn get_contact_summary(
        &self,
        account_slot_id: &str,
        contact_id: &str,
    ) -> CommandResult<ContactSummary> {
        let row = sqlx::query(
            "SELECT id, name, email, revision, created_at, updated_at FROM contacts \
             WHERE id = ? AND account_slot_id = ?",
        )
        .bind(contact_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.contacts_read_failed"))?
        .ok_or_else(|| CommandError::new("contact.not_found"))?;
        contact_summary_from_row(row)
    }

    pub(crate) async fn identities_for_emails(
        &self,
        account_slot_id: &str,
        emails: &[String],
    ) -> CommandResult<HashMap<String, ContactIdentity>> {
        let normalized = emails
            .iter()
            .filter_map(|email| normalize_email(email).map(|(_, normalized)| normalized))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut identities = HashMap::new();
        for batch in normalized.chunks(CONTACT_IDENTITY_QUERY_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Sqlite>::new(
                "SELECT id, normalized_email, name FROM contacts WHERE account_slot_id = ",
            );
            builder
                .push_bind(account_slot_id)
                .push(" AND normalized_email IN (");
            let mut separated = builder.separated(", ");
            for email in batch {
                separated.push_bind(email);
            }
            separated.push_unseparated(")");
            let rows = builder
                .build()
                .fetch_all(&self.pool)
                .await
                .map_err(map_storage_err("storage.contacts_read_failed"))?;
            for row in rows {
                identities.insert(
                    row.try_get("normalized_email")
                        .map_err(storage_read_error)?,
                    ContactIdentity {
                        id: row.try_get("id").map_err(storage_read_error)?,
                        name: row.try_get("name").map_err(storage_read_error)?,
                    },
                );
            }
        }
        Ok(identities)
    }

    pub async fn resolve_addresses(
        &self,
        account_slot_id: &str,
        addresses: &[MessageAddress],
    ) -> CommandResult<Vec<AddressPresentation>> {
        let emails = addresses
            .iter()
            .map(|address| address.email.clone())
            .collect::<Vec<_>>();
        let identities = self.identities_for_emails(account_slot_id, &emails).await?;
        Ok(addresses
            .iter()
            .map(|address| address_presentation(address, &identities))
            .collect())
    }

    pub async fn backfill_next_batch(
        &self,
        account_slot_id: &str,
    ) -> CommandResult<ContactBackfillBatch> {
        let state = sqlx::query(
            "SELECT last_message_rowid, completed_at FROM contact_backfill_state WHERE account_slot_id = ?",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.contact_backfill_failed"))?;
        if state
            .as_ref()
            .and_then(|row| row.try_get::<Option<i64>, _>("completed_at").ok().flatten())
            .is_some()
        {
            return Ok(ContactBackfillBatch {
                complete: true,
                ..Default::default()
            });
        }
        let last_rowid = state
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("last_message_rowid").ok())
            .unwrap_or(0);
        let rows = sqlx::query(
            "SELECT rowid AS message_rowid, id, from_json, to_json, cc_json FROM messages \
             WHERE account_slot_id = ? AND rowid > ? ORDER BY rowid LIMIT ?",
        )
        .bind(account_slot_id)
        .bind(last_rowid)
        .bind(i64::from(CONTACT_BACKFILL_BATCH_SIZE))
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("storage.contact_backfill_failed"))?;
        let complete = rows.len() < CONTACT_BACKFILL_BATCH_SIZE as usize;
        let processed = rows.len() as u32;
        let mut prepared = Vec::with_capacity(rows.len());
        let mut final_rowid = last_rowid;
        for row in rows {
            let rowid = row
                .try_get::<i64, _>("message_rowid")
                .map_err(storage_read_error)?;
            final_rowid = final_rowid.max(rowid);
            let message_id = row.try_get::<String, _>("id").map_err(storage_read_error)?;
            let mut candidates = Vec::new();
            append_stored_addresses(
                &mut candidates,
                ContactAddressRole::From,
                row.try_get("from_json").map_err(storage_read_error)?,
            );
            append_stored_addresses(
                &mut candidates,
                ContactAddressRole::To,
                row.try_get("to_json").map_err(storage_read_error)?,
            );
            append_stored_addresses(
                &mut candidates,
                ContactAddressRole::Cc,
                row.try_get("cc_json").map_err(storage_read_error)?,
            );
            prepared.push((message_id, candidates));
        }

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("storage.contact_backfill_failed"))?;
        let mut changed = false;
        for (message_id, candidates) in &prepared {
            changed |= upsert_contact_candidates(
                &mut transaction,
                account_slot_id,
                message_id,
                candidates,
            )
            .await?;
        }
        sqlx::query(
            "INSERT INTO contact_backfill_state(account_slot_id, last_message_rowid, completed_at) \
             VALUES (?, ?, ?) ON CONFLICT(account_slot_id) DO UPDATE SET \
             last_message_rowid = excluded.last_message_rowid, completed_at = excluded.completed_at",
        )
        .bind(account_slot_id)
        .bind(final_rowid)
        .bind(complete.then(now))
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.contact_backfill_failed"))?;
        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.contact_backfill_failed"))?;
        Ok(ContactBackfillBatch {
            processed,
            changed,
            complete,
        })
    }
}

pub(crate) async fn upsert_remote_message_contacts(
    transaction: &mut Transaction<'_, Sqlite>,
    account_slot_id: &str,
    message_id: &str,
    candidates: &[RemoteContactAddress],
) -> CommandResult<bool> {
    let candidates = candidates
        .iter()
        .map(|candidate| (candidate.role, candidate.address.clone()))
        .collect::<Vec<_>>();
    upsert_contact_candidates(transaction, account_slot_id, message_id, &candidates).await
}

async fn upsert_contact_candidates(
    transaction: &mut Transaction<'_, Sqlite>,
    account_slot_id: &str,
    message_id: &str,
    candidates: &[(ContactAddressRole, MessageAddress)],
) -> CommandResult<bool> {
    let mut changed = false;
    let mut seen = HashSet::new();
    let discovered_names = candidates
        .iter()
        .filter_map(|(_, address)| {
            let (_, normalized_email) = normalize_email(&address.email)?;
            let name = automatic_header_name(address.name.as_deref()?)?;
            Some((normalized_email, name))
        })
        .fold(HashMap::new(), |mut names, (email, name)| {
            names.entry(email).or_insert(name);
            names
        });
    for (role, address) in candidates {
        let Some((email, normalized_email)) = normalize_email(&address.email) else {
            continue;
        };
        if !seen.insert((role.as_str(), normalized_email.clone())) {
            continue;
        }
        let contact_id = Uuid::new_v4().to_string();
        let timestamp = now();
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO contacts(id, account_slot_id, normalized_email, email, name, name_source, created_at, updated_at, revision) \
             VALUES (?, ?, ?, ?, ?, 'auto', ?, ?, 1)",
        )
        .bind(&contact_id)
        .bind(account_slot_id)
        .bind(&normalized_email)
        .bind(&email)
        .bind(
            discovered_names
                .get(&normalized_email)
                .cloned()
                .unwrap_or_else(|| default_contact_name(&email)),
        )
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut **transaction)
        .await
        .map_err(map_storage_err("storage.contact_write_failed"))?;
        changed |= inserted.rows_affected() > 0;
        let contact_id = if inserted.rows_affected() > 0 {
            contact_id
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT id FROM contacts WHERE account_slot_id = ? AND normalized_email = ?",
            )
            .bind(account_slot_id)
            .bind(&normalized_email)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_storage_err("storage.contacts_read_failed"))?
        };
        let linked = sqlx::query(
            "INSERT OR IGNORE INTO message_contacts(message_id, contact_id, role) VALUES (?, ?, ?)",
        )
        .bind(message_id)
        .bind(contact_id)
        .bind(role.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(map_storage_err("storage.contact_write_failed"))?;
        changed |= linked.rows_affected() > 0;
    }
    Ok(changed)
}

pub(crate) fn address_presentation(
    address: &MessageAddress,
    identities: &HashMap<String, ContactIdentity>,
) -> AddressPresentation {
    let identity =
        normalize_email(&address.email).and_then(|(_, normalized)| identities.get(&normalized));
    AddressPresentation {
        contact_id: identity.map(|value| value.id.clone()),
        name: identity
            .map(|value| value.name.clone())
            .or_else(|| address.name.clone()),
        header_name: address.name.clone(),
        email: address.email.clone(),
    }
}

pub(crate) fn normalize_email(value: &str) -> Option<(String, String)> {
    let email = value.trim();
    if email.is_empty()
        || email.len() > 254
        || email
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || email.parse::<Address>().is_err()
    {
        return None;
    }
    let (local, domain) = email.rsplit_once('@')?;
    if local.is_empty() || domain.is_empty() || local.contains('@') {
        return None;
    }
    Some((email.to_owned(), email.to_ascii_lowercase()))
}

fn default_contact_name(email: &str) -> String {
    email
        .split_once('@')
        .map(|(local, _)| local)
        .filter(|local| !local.is_empty())
        .unwrap_or(email)
        .to_owned()
}

fn automatic_header_name(value: &str) -> Option<String> {
    validate_contact_name(value).ok()
}

fn validate_contact_name(value: &str) -> CommandResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CommandError::new("contact.name_required"));
    }
    if value.chars().count() > CONTACT_NAME_MAX_CHARS
        || value.chars().any(|character| character.is_control())
    {
        return Err(CommandError::new("contact.name_invalid"));
    }
    Ok(value.to_owned())
}

fn append_stored_addresses(
    target: &mut Vec<(ContactAddressRole, MessageAddress)>,
    role: ContactAddressRole,
    json: String,
) {
    let values = serde_json::from_str::<Vec<MessageAddress>>(&json).unwrap_or_default();
    target.extend(values.into_iter().map(|address| (role, address)));
}

fn contact_summary_from_row(row: sqlx::sqlite::SqliteRow) -> CommandResult<ContactSummary> {
    Ok(ContactSummary {
        id: row.try_get("id").map_err(storage_read_error)?,
        name: row.try_get("name").map_err(storage_read_error)?,
        email: row.try_get("email").map_err(storage_read_error)?,
        revision: row
            .try_get::<i64, _>("revision")
            .map_err(storage_read_error)? as u64,
        created_at: row.try_get("created_at").map_err(storage_read_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_read_error)?,
    })
}

fn parse_contact_cursor(value: &str) -> Option<ContactCursor> {
    serde_json::from_str(value).ok()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{
        automatic_header_name, default_contact_name, normalize_email, validate_contact_name,
    };

    #[test]
    fn normalizes_email_without_provider_alias_folding() {
        assert_eq!(
            normalize_email("  Alice+Work@Example.COM  "),
            Some((
                "Alice+Work@Example.COM".to_owned(),
                "alice+work@example.com".to_owned(),
            ))
        );
        assert_eq!(default_contact_name("alice+work@example.com"), "alice+work");
        assert!(normalize_email("missing-at.example.com").is_none());
        assert!(normalize_email("a b@example.com").is_none());
        assert!(normalize_email("alice@.").is_none());
    }

    #[test]
    fn validates_contact_names() {
        assert_eq!(validate_contact_name("  Alice  ").unwrap(), "Alice");
        assert_eq!(
            automatic_header_name("  Header Alice  ").as_deref(),
            Some("Header Alice")
        );
        assert!(validate_contact_name(" ").is_err());
        assert!(validate_contact_name("Alice\nAdmin").is_err());
        assert!(automatic_header_name("Alice\nAdmin").is_none());
    }
}

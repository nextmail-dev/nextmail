use crate::core::{CommandError, CommandResult, MailboxRole};
use sqlx::{Row, SqlitePool};

use super::now;

#[derive(Clone)]
pub struct MailboxRoleRepository {
    pub(crate) pool: SqlitePool,
}

impl MailboxRoleRepository {
    pub async fn mailbox_for_role(
        &self,
        account_slot_id: &str,
        role: MailboxRole,
    ) -> CommandResult<Option<(String, String)>> {
        let role = role_to_db(&role);
        let row = sqlx::query(
            "SELECT b.id, b.remote_name FROM mailboxes b \
             LEFT JOIN mailbox_role_overrides o ON o.mailbox_id = b.id AND o.account_slot_id = b.account_slot_id \
             WHERE b.account_slot_id = ? AND b.selectable = 1 \
             AND (o.role = ? OR (o.role IS NULL AND b.role = ?)) \
             ORDER BY CASE WHEN o.role IS NOT NULL THEN 0 ELSE 1 END LIMIT 1",
        )
        .bind(account_slot_id)
        .bind(role)
        .bind(role)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.mailboxes_read_failed"))?;
        row.map(|row| {
            Ok((
                row.try_get("id").map_err(storage_read_error)?,
                row.try_get("remote_name").map_err(storage_read_error)?,
            ))
        })
        .transpose()
    }

    pub async fn mailbox_role_for_id(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
    ) -> CommandResult<MailboxRole> {
        let value = sqlx::query_scalar::<_, String>(
            "SELECT CASE WHEN o.role IS NOT NULL THEN o.role \
               WHEN EXISTS(SELECT 1 FROM mailbox_role_overrides x WHERE x.account_slot_id = b.account_slot_id AND x.role = b.role) \
               THEN 'other' ELSE b.role END FROM mailboxes b \
             LEFT JOIN mailbox_role_overrides o ON o.mailbox_id = b.id AND o.account_slot_id = b.account_slot_id \
             WHERE b.id = ? AND b.account_slot_id = ?",
        )
        .bind(mailbox_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CommandError::new("storage.mailboxes_read_failed"))?
        .ok_or_else(|| CommandError::new("mailbox.not_found"))?;
        Ok(role_from_db(&value))
    }

    pub async fn set_mailbox_role_mapping(
        &self,
        account_slot_id: &str,
        role: MailboxRole,
        mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        if matches!(
            role,
            MailboxRole::Inbox | MailboxRole::Junk | MailboxRole::Other
        ) {
            return Err(CommandError::new("mailbox.role_not_mappable"));
        }
        if let Some(mailbox_id) = mailbox_id {
            ensure_mailbox(&self.pool, account_slot_id, mailbox_id).await?;
            sqlx::query(
                "DELETE FROM mailbox_role_overrides WHERE account_slot_id = ? AND mailbox_id = ? AND role != ?",
            )
            .bind(account_slot_id)
            .bind(mailbox_id)
            .bind(role_to_db(&role))
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("mailbox.role_mapping_failed"))?;
            sqlx::query(
                "INSERT INTO mailbox_role_overrides(account_slot_id, role, mailbox_id, updated_at) \
                 VALUES (?, ?, ?, ?) ON CONFLICT(account_slot_id, role) DO UPDATE SET \
                 mailbox_id = excluded.mailbox_id, updated_at = excluded.updated_at",
            )
            .bind(account_slot_id)
            .bind(role_to_db(&role))
            .bind(mailbox_id)
            .bind(now())
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("mailbox.role_mapping_failed"))?;
        } else {
            sqlx::query(
                "DELETE FROM mailbox_role_overrides WHERE account_slot_id = ? AND role = ?",
            )
            .bind(account_slot_id)
            .bind(role_to_db(&role))
            .execute(&self.pool)
            .await
            .map_err(|_| CommandError::new("mailbox.role_mapping_failed"))?;
        }
        Ok(())
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

fn role_from_db(value: &str) -> MailboxRole {
    match value {
        "inbox" => MailboxRole::Inbox,
        "sent" => MailboxRole::Sent,
        "drafts" => MailboxRole::Drafts,
        "trash" => MailboxRole::Trash,
        "junk" => MailboxRole::Junk,
        "archive" => MailboxRole::Archive,
        _ => MailboxRole::Other,
    }
}

fn storage_read_error(_: sqlx::Error) -> CommandError {
    CommandError::new("storage.read_failed")
}

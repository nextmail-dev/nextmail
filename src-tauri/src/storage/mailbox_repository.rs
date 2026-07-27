use std::collections::HashSet;

use crate::core::{CommandError, CommandResult};
use sqlx::{FromRow, Row, SqlitePool};
use uuid::Uuid;

use super::map_storage_err;

#[derive(Clone)]
pub struct MailboxRepository {
    pub(crate) pool: SqlitePool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailboxMutationContext {
    pub id: String,
    pub remote_name: String,
    pub display_name: String,
    pub delimiter: Option<String>,
    pub selectable: bool,
}

#[derive(FromRow)]
struct MailboxPathRow {
    id: String,
    remote_name: String,
    display_name: String,
}

impl MailboxRepository {
    pub async fn mutation_context(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
    ) -> CommandResult<MailboxMutationContext> {
        let row = sqlx::query(
            "SELECT id, remote_name, display_name, delimiter, selectable \
             FROM mailboxes WHERE id = ? AND account_slot_id = ?",
        )
        .bind(mailbox_id)
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.mailboxes_read_failed"))?
        .ok_or_else(|| CommandError::new("mailbox.not_found"))?;
        Ok(MailboxMutationContext {
            id: row
                .try_get("id")
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?,
            remote_name: row
                .try_get("remote_name")
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?,
            display_name: row
                .try_get("display_name")
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?,
            delimiter: row
                .try_get("delimiter")
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?,
            selectable: row
                .try_get::<i64, _>("selectable")
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?
                != 0,
        })
    }

    pub async fn default_delimiter(&self, account_slot_id: &str) -> CommandResult<Option<String>> {
        sqlx::query_scalar(
            "SELECT delimiter FROM mailboxes WHERE account_slot_id = ? \
             AND delimiter IS NOT NULL AND delimiter != '' ORDER BY local_sort_order, remote_name LIMIT 1",
        )
        .bind(account_slot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_storage_err("storage.mailboxes_read_failed"))
    }

    pub async fn insert_created_mailbox(
        &self,
        account_slot_id: &str,
        remote_name: &str,
        display_name: &str,
        delimiter: Option<&str>,
    ) -> CommandResult<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO mailboxes(\
                id, account_slot_id, remote_name, display_name, delimiter, role, selectable, \
                uid_validity, uid_next, last_uid, total_count, unread_count, revision, local_sort_order\
             ) VALUES (?, ?, ?, ?, ?, 'other', 1, 0, 0, 0, 0, 0, 1, (\
                SELECT CASE WHEN MAX(local_sort_order) IS NULL THEN NULL \
                ELSE MAX(local_sort_order) + 1 END FROM mailboxes WHERE account_slot_id = ?\
             ))",
        )
        .bind(&id)
        .bind(account_slot_id)
        .bind(remote_name)
        .bind(display_name)
        .bind(delimiter)
        .bind(account_slot_id)
        .execute(&self.pool)
        .await
        .map_err(map_storage_err("mailbox.local_write_failed"))?;
        Ok(id)
    }

    pub async fn rename_mailbox_tree(
        &self,
        account_slot_id: &str,
        source: &MailboxMutationContext,
        destination_remote_name: &str,
        destination_display_name: &str,
    ) -> CommandResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        let rows = sqlx::query_as::<_, MailboxPathRow>(
            "SELECT id, remote_name, display_name FROM mailboxes WHERE account_slot_id = ?",
        )
        .bind(account_slot_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_storage_err("mailbox.local_write_failed"))?;
        let delimiter = source.delimiter.as_deref().unwrap_or_default();
        let remote_prefix = format!("{}{}", source.remote_name, delimiter);
        let affected = rows
            .iter()
            .filter(|row| {
                row.id == source.id
                    || (!delimiter.is_empty() && row.remote_name.starts_with(&remote_prefix))
            })
            .collect::<Vec<_>>();
        let affected_ids = affected
            .iter()
            .map(|row| row.id.as_str())
            .collect::<HashSet<_>>();
        let updates = affected
            .iter()
            .map(|row| -> CommandResult<_> {
                if row.id == source.id {
                    Ok((
                        row.id.as_str(),
                        destination_remote_name.to_owned(),
                        destination_display_name.to_owned(),
                    ))
                } else {
                    let display_suffix = row
                        .display_name
                        .strip_prefix(&source.display_name)
                        .ok_or_else(|| CommandError::new("mailbox.local_write_failed"))?;
                    Ok((
                        row.id.as_str(),
                        format!(
                            "{}{}",
                            destination_remote_name,
                            &row.remote_name[source.remote_name.len()..]
                        ),
                        format!("{destination_display_name}{display_suffix}"),
                    ))
                }
            })
            .collect::<CommandResult<Vec<_>>>()?;
        if updates.iter().any(|(_, remote_name, _)| {
            rows.iter().any(|row| {
                !affected_ids.contains(row.id.as_str()) && row.remote_name == *remote_name
            })
        }) {
            return Err(CommandError::new("mailbox.already_exists"));
        }
        for (id, remote_name, display_name) in updates {
            sqlx::query(
                "UPDATE mailboxes SET remote_name = ?, display_name = ?, revision = revision + 1 \
                 WHERE id = ? AND account_slot_id = ?",
            )
            .bind(remote_name)
            .bind(display_name)
            .bind(id)
            .bind(account_slot_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        }
        transaction
            .commit()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))
    }

    pub async fn delete_mailbox(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
    ) -> CommandResult<()> {
        let result = sqlx::query("DELETE FROM mailboxes WHERE id = ? AND account_slot_id = ?")
            .bind(mailbox_id)
            .bind(account_slot_id)
            .execute(&self.pool)
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(CommandError::new("mailbox.not_found"))
        }
    }

    pub async fn mark_all_read(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
    ) -> CommandResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM mailboxes WHERE id = ? AND account_slot_id = ?",
        )
        .bind(mailbox_id)
        .bind(account_slot_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_storage_err("mailbox.local_write_failed"))?;
        if exists != 1 {
            return Err(CommandError::new("mailbox.not_found"));
        }
        sqlx::query(
            "UPDATE message_locations SET unread = 0 WHERE mailbox_id = ? AND local_hidden = 0",
        )
        .bind(mailbox_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("mailbox.local_write_failed"))?;
        sqlx::query(
            "UPDATE mailboxes SET unread_count = 0, revision = revision + 1 \
             WHERE id = ? AND account_slot_id = ?",
        )
        .bind(mailbox_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("mailbox.local_write_failed"))?;
        transaction
            .commit()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))
    }

    pub async fn reorder(
        &self,
        account_slot_id: &str,
        ordered_mailbox_ids: &[String],
    ) -> CommandResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        let existing =
            sqlx::query_scalar::<_, String>("SELECT id FROM mailboxes WHERE account_slot_id = ?")
                .bind(account_slot_id)
                .fetch_all(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.mailboxes_read_failed"))?;
        let existing_set = existing.iter().collect::<HashSet<_>>();
        let requested_set = ordered_mailbox_ids.iter().collect::<HashSet<_>>();
        if existing.len() != ordered_mailbox_ids.len()
            || requested_set.len() != ordered_mailbox_ids.len()
            || existing_set != requested_set
        {
            return Err(CommandError::new("mailbox.order_conflict"));
        }
        for (index, mailbox_id) in ordered_mailbox_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE mailboxes SET local_sort_order = ?, revision = revision + 1 \
                 WHERE id = ? AND account_slot_id = ?",
            )
            .bind(index as i64)
            .bind(mailbox_id)
            .bind(account_slot_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))?;
        }
        transaction
            .commit()
            .await
            .map_err(map_storage_err("mailbox.local_write_failed"))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::storage::{initialize_content_database, MailRepository};

    async fn repository() -> (tempfile::TempDir, MailRepository) {
        let directory = tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        sqlx::query("INSERT INTO account_slots(id, created_at) VALUES ('slot', ?)")
            .bind(0_i64)
            .execute(&repository.pool)
            .await
            .unwrap();
        (directory, repository)
    }

    async fn insert_mailbox(
        repository: &MailRepository,
        id: &str,
        remote_name: &str,
        display_name: &str,
        sort_order: Option<i64>,
    ) {
        sqlx::query(
            "INSERT INTO mailboxes(\
                id, account_slot_id, remote_name, display_name, delimiter, role, selectable, local_sort_order\
             ) VALUES (?, 'slot', ?, ?, '/', 'other', 1, ?)",
        )
        .bind(id)
        .bind(remote_name)
        .bind(display_name)
        .bind(sort_order)
        .execute(&repository.pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn rename_preserves_ids_and_updates_descendant_paths() {
        let (_directory, repository) = repository().await;
        insert_mailbox(&repository, "parent", "Projects", "Projects", Some(0)).await;
        insert_mailbox(
            &repository,
            "child",
            "Projects/2026",
            "Projects/2026",
            Some(1),
        )
        .await;
        sqlx::query("INSERT INTO account_slots(id, created_at) VALUES ('other-slot', 0)")
            .execute(&repository.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO mailboxes(\
                id, account_slot_id, remote_name, display_name, delimiter, role, selectable\
             ) VALUES ('other-parent', 'other-slot', 'Projects', 'Projects', '/', 'other', 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        let mailboxes = repository.mailboxes();
        let source = mailboxes.mutation_context("slot", "parent").await.unwrap();
        mailboxes
            .rename_mailbox_tree("slot", &source, "Work", "工作")
            .await
            .unwrap();
        let rows = sqlx::query_as::<_, MailboxPathRow>(
            "SELECT id, remote_name, display_name FROM mailboxes \
             WHERE account_slot_id = 'slot' ORDER BY local_sort_order",
        )
        .fetch_all(&repository.pool)
        .await
        .unwrap();
        assert_eq!(
            rows.into_iter()
                .map(|row| (row.id, row.remote_name, row.display_name))
                .collect::<Vec<_>>(),
            vec![
                ("parent".to_owned(), "Work".to_owned(), "工作".to_owned()),
                (
                    "child".to_owned(),
                    "Work/2026".to_owned(),
                    "工作/2026".to_owned()
                ),
            ]
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT remote_name FROM mailboxes WHERE id = 'other-parent'"
            )
            .fetch_one(&repository.pool)
            .await
            .unwrap(),
            "Projects"
        );
    }

    #[tokio::test]
    async fn reorder_requires_the_complete_account_mailbox_set() {
        let (_directory, repository) = repository().await;
        insert_mailbox(&repository, "one", "One", "One", None).await;
        insert_mailbox(&repository, "two", "Two", "Two", None).await;
        let mailboxes = repository.mailboxes();
        assert_eq!(
            mailboxes
                .reorder("slot", &["one".to_owned()])
                .await
                .unwrap_err()
                .code,
            "mailbox.order_conflict"
        );
        mailboxes
            .reorder("slot", &["two".to_owned(), "one".to_owned()])
            .await
            .unwrap();
        let order =
            sqlx::query_scalar::<_, String>("SELECT id FROM mailboxes ORDER BY local_sort_order")
                .fetch_all(&repository.pool)
                .await
                .unwrap();
        assert_eq!(order, vec!["two", "one"]);
    }

    #[tokio::test]
    async fn created_mailboxes_append_after_an_explicit_local_order() {
        let (_directory, repository) = repository().await;
        insert_mailbox(&repository, "one", "One", "One", Some(0)).await;

        let created_id = repository
            .mailboxes()
            .insert_created_mailbox("slot", "Two", "Two", Some("/"))
            .await
            .unwrap();
        let sort_order =
            sqlx::query_scalar::<_, i64>("SELECT local_sort_order FROM mailboxes WHERE id = ?")
                .bind(created_id)
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(sort_order, 1);
    }
}

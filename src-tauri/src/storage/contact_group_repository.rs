use std::collections::HashSet;

use sqlx::{QueryBuilder, Row, Sqlite};
use uuid::Uuid;

use crate::core::{
    CommandError, CommandResult, ContactGroupDetail, ContactGroupDraft, ContactGroupSummary,
};

use super::{
    begin_write, contact_repository::contact_summary_from_row, map_storage_err, storage_read_error,
    ContactRepository,
};

impl ContactRepository {
    pub async fn list_contact_groups(
        &self,
        account_slot_id: &str,
    ) -> CommandResult<Vec<ContactGroupSummary>> {
        sqlx::query(
            "SELECT g.id, g.name, g.revision, COUNT(m.contact_id) AS member_count \
             FROM contact_groups g LEFT JOIN contact_group_members m \
             ON m.account_slot_id = g.account_slot_id AND m.group_id = g.id \
             WHERE g.account_slot_id = ? GROUP BY g.id ORDER BY g.name COLLATE NOCASE, g.id",
        )
        .bind(account_slot_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_storage_err("contact_group.read_failed"))?
        .into_iter()
        .map(group_from_row)
        .collect()
    }

    pub async fn get_contact_group(
        &self,
        account_slot_id: &str,
        group_id: &str,
    ) -> CommandResult<ContactGroupDetail> {
        // Read the revision and members from the same snapshot.
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(map_storage_err("contact_group.read_failed"))?;
        let row = sqlx::query(
            "SELECT id, name, revision FROM contact_groups WHERE account_slot_id = ? AND id = ?",
        )
        .bind(account_slot_id)
        .bind(group_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_storage_err("contact_group.read_failed"))?
        .ok_or_else(|| CommandError::new("contact_group.not_found"))?;
        let members = sqlx::query(
            "SELECT c.* FROM contacts c JOIN contact_group_members m \
             ON c.account_slot_id = m.account_slot_id AND c.id = m.contact_id \
             WHERE m.account_slot_id = ? AND m.group_id = ? ORDER BY c.name COLLATE NOCASE, c.normalized_email, c.id",
        )
        .bind(account_slot_id)
        .bind(group_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(map_storage_err("contact_group.read_failed"))?
        .into_iter()
        .map(contact_summary_from_row)
        .collect::<CommandResult<Vec<_>>>()?;
        Ok(ContactGroupDetail {
            group: ContactGroupSummary {
                id: row.try_get("id").map_err(storage_read_error)?,
                name: row.try_get("name").map_err(storage_read_error)?,
                revision: row
                    .try_get::<i64, _>("revision")
                    .map_err(storage_read_error)? as u64,
                member_count: members.len() as u64,
            },
            members,
        })
    }

    pub async fn save_contact_group(
        &self,
        account_slot_id: &str,
        group_id: Option<&str>,
        draft: &ContactGroupDraft,
        expected_revision: Option<u64>,
    ) -> CommandResult<ContactGroupDetail> {
        let name = draft.name.trim();
        if name.is_empty() {
            return Err(CommandError::new("contact_group.name_required"));
        }
        if name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err(CommandError::new("contact_group.name_invalid"));
        }
        let id = group_id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let mut transaction = begin_write(&self.pool).await.map_err(group_write_error)?;
        if group_id.is_some() {
            let result = sqlx::query(
                "UPDATE contact_groups SET name = ?, normalized_name = ?, revision = revision + 1 \
                 WHERE account_slot_id = ? AND id = ? AND revision = ?",
            )
            .bind(name)
            .bind(name.to_lowercase())
            .bind(account_slot_id)
            .bind(&id)
            .bind(expected_revision.and_then(|value| i64::try_from(value).ok()))
            .execute(&mut *transaction)
            .await
            .map_err(group_write_error)?;
            if result.rows_affected() == 0 {
                return Err(CommandError::new("contact_group.conflict"));
            }
            sqlx::query(
                "DELETE FROM contact_group_members WHERE account_slot_id = ? AND group_id = ?",
            )
            .bind(account_slot_id)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(group_write_error)?;
        } else {
            sqlx::query("INSERT INTO contact_groups(id, account_slot_id, name, normalized_name) VALUES (?, ?, ?, ?)")
                .bind(&id).bind(account_slot_id).bind(name).bind(name.to_lowercase())
                .execute(&mut *transaction).await.map_err(group_write_error)?;
        }
        let contact_ids = draft
            .contact_ids
            .iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for batch in contact_ids.chunks(400) {
            let mut query = QueryBuilder::<Sqlite>::new("INSERT INTO contact_group_members(account_slot_id, group_id, contact_id) SELECT account_slot_id, ");
            query
                .push_bind(&id)
                .push(", id FROM contacts WHERE account_slot_id = ")
                .push_bind(account_slot_id)
                .push(" AND id IN (");
            let mut values = query.separated(", ");
            for contact_id in batch {
                values.push_bind(*contact_id);
            }
            values.push_unseparated(")");
            let result = query
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(group_write_error)?;
            if result.rows_affected() != batch.len() as u64 {
                return Err(CommandError::new("contact_group.member_unavailable"));
            }
        }
        transaction.commit().await.map_err(group_write_error)?;
        self.get_contact_group(account_slot_id, &id).await
    }

    pub async fn delete_contact_group(
        &self,
        account_slot_id: &str,
        group_id: &str,
        expected_revision: u64,
    ) -> CommandResult<()> {
        let result = sqlx::query(
            "DELETE FROM contact_groups WHERE account_slot_id = ? AND id = ? AND revision = ?",
        )
        .bind(account_slot_id)
        .bind(group_id)
        .bind(i64::try_from(expected_revision).ok())
        .execute(&self.pool)
        .await
        .map_err(group_write_error)?;
        if result.rows_affected() == 0 {
            return Err(CommandError::new("contact_group.conflict"));
        }
        Ok(())
    }
}

fn group_from_row(row: sqlx::sqlite::SqliteRow) -> CommandResult<ContactGroupSummary> {
    Ok(ContactGroupSummary {
        id: row.try_get("id").map_err(storage_read_error)?,
        name: row.try_get("name").map_err(storage_read_error)?,
        member_count: row
            .try_get::<i64, _>("member_count")
            .map_err(storage_read_error)? as u64,
        revision: row
            .try_get::<i64, _>("revision")
            .map_err(storage_read_error)? as u64,
    })
}

fn group_write_error(error: sqlx::Error) -> CommandError {
    if matches!(&error, sqlx::Error::Database(error) if error.is_unique_violation()) {
        CommandError::new("contact_group.already_exists")
    } else {
        map_storage_err("contact_group.write_failed")(error)
    }
}

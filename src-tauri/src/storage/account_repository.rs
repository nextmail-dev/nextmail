use crate::core::{AccountRemovalImpact, CommandError, CommandResult};
use sqlx::Row;

use super::MailRepository;

impl MailRepository {
    pub async fn account_removal_impact(
        &self,
        account_slot_id: &str,
    ) -> CommandResult<AccountRemovalImpact> {
        let row = sqlx::query(
            "SELECT \
               (SELECT COUNT(*) FROM drafts WHERE account_slot_id = ? AND status = 'editing') AS editing_drafts, \
               (SELECT COUNT(*) FROM send_jobs WHERE account_slot_id = ? AND status IN ('queued','sending')) AS queued_send_jobs, \
               (SELECT COUNT(*) FROM pending_operations WHERE account_slot_id = ? AND status IN ('queued','running','retry_wait','needs_reconcile','failed')) AS pending_operations",
        )
        .bind(account_slot_id)
        .bind(account_slot_id)
        .bind(account_slot_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| CommandError::new("account.removal_impact_failed"))?;
        let editing_drafts = row
            .try_get::<i64, _>("editing_drafts")
            .map_err(|_| CommandError::new("account.removal_impact_failed"))?
            as u64;
        let queued_send_jobs = row
            .try_get::<i64, _>("queued_send_jobs")
            .map_err(|_| CommandError::new("account.removal_impact_failed"))?
            as u64;
        let pending_operations = row
            .try_get::<i64, _>("pending_operations")
            .map_err(|_| CommandError::new("account.removal_impact_failed"))?
            as u64;
        Ok(AccountRemovalImpact {
            editing_drafts,
            queued_send_jobs,
            pending_operations,
            can_remove: queued_send_jobs == 0 && pending_operations == 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{create_account_slot, initialize_content_database};

    #[tokio::test]
    async fn ordinary_drafts_do_not_block_removal_but_send_jobs_and_operations_do() {
        let directory = tempfile::tempdir().unwrap();
        initialize_content_database(directory.path()).await.unwrap();
        create_account_slot(directory.path(), "slot-a", 1)
            .await
            .unwrap();
        create_account_slot(directory.path(), "slot-b", 2)
            .await
            .unwrap();
        let repository = MailRepository::open(directory.path()).await.unwrap();
        let draft_a = repository
            .create_draft("account-a", "slot-a")
            .await
            .unwrap();
        let draft_b = repository
            .create_draft("account-b", "slot-b")
            .await
            .unwrap();

        let draft_only = repository.account_removal_impact("slot-a").await.unwrap();
        assert_eq!(draft_only.editing_drafts, 1);
        assert!(draft_only.can_remove);

        let mime_hash = repository
            .write_send_mime(b"From: a@example.com\r\n\r\nbody")
            .await
            .unwrap();
        repository
            .queue_send_job(
                "account-b",
                "slot-b",
                &draft_b.id,
                &mime_hash,
                &["to@example.com".to_owned()],
            )
            .await
            .unwrap();
        assert!(
            repository
                .account_removal_impact("slot-a")
                .await
                .unwrap()
                .can_remove
        );

        repository
            .queue_send_job(
                "account-a",
                "slot-a",
                &draft_a.id,
                &mime_hash,
                &["to@example.com".to_owned()],
            )
            .await
            .unwrap();
        let queued = repository.account_removal_impact("slot-a").await.unwrap();
        assert_eq!(queued.queued_send_jobs, 1);
        assert!(!queued.can_remove);

        sqlx::query(
            "INSERT INTO pending_operations(id, account_slot_id, kind, status, created_at, updated_at) \
             VALUES ('operation-b', 'slot-b', 'set_read', 'queued', 1, 1)",
        )
        .execute(&repository.pool)
        .await
        .unwrap();
        assert_eq!(
            repository
                .account_removal_impact("slot-a")
                .await
                .unwrap()
                .pending_operations,
            0
        );
    }
}

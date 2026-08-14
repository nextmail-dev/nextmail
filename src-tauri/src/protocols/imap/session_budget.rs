use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use crate::core::{CommandError, CommandResult};

pub(super) const ACCOUNT_SESSION_LIMIT: usize = 3;
pub(super) const SYNC_SESSION_COUNT: usize = 2;

/// Per-account concurrency budget for active IMAP sessions.
///
/// Sessions themselves remain owned by the concrete protocol operation and
/// are logged out/closed when that operation finishes. The registry stores
/// weak references only, so an inactive account budget disappears without a
/// cleanup worker. Keeping the budget in the adapter prevents async-imap types
/// from crossing the existing core port.
#[derive(Default)]
pub(super) struct SessionBudgetRegistry {
    budgets: Mutex<HashMap<String, Weak<Semaphore>>>,
}

impl SessionBudgetRegistry {
    pub(super) async fn acquire(&self, account_id: &str) -> CommandResult<OwnedSemaphorePermit> {
        let budget = self.budget(account_id);
        match Arc::clone(&budget).try_acquire_owned() {
            Ok(permit) => Ok(permit),
            Err(TryAcquireError::NoPermits) => {
                tracing::debug!(%account_id, "waiting for an IMAP session slot");
                budget
                    .acquire_owned()
                    .await
                    .map_err(|_| CommandError::retryable("account.network_unavailable"))
            }
            Err(TryAcquireError::Closed) => {
                Err(CommandError::retryable("account.network_unavailable"))
            }
        }
    }

    fn budget(&self, account_id: &str) -> Arc<Semaphore> {
        let mut budgets = self
            .budgets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = budgets.get(account_id).and_then(Weak::upgrade) {
            return existing;
        }
        let budget = Arc::new(Semaphore::new(ACCOUNT_SESSION_LIMIT));
        budgets.insert(account_id.to_owned(), Arc::downgrade(&budget));
        budget
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::{SessionBudgetRegistry, ACCOUNT_SESSION_LIMIT, SYNC_SESSION_COUNT};

    #[tokio::test]
    async fn keeps_interactive_slots_while_sync_workers_are_active() {
        assert_eq!(ACCOUNT_SESSION_LIMIT, 3);
        assert_eq!(SYNC_SESSION_COUNT, 2);
        let interactive_slots = ACCOUNT_SESSION_LIMIT - SYNC_SESSION_COUNT;
        assert_eq!(interactive_slots, 1);
        let registry = Arc::new(SessionBudgetRegistry::default());
        let mut sync_permits = Vec::new();
        for _ in 0..SYNC_SESSION_COUNT {
            sync_permits.push(registry.acquire("account").await.unwrap());
        }
        let mut interactive_permits = Vec::new();
        for _ in 0..interactive_slots {
            interactive_permits.push(registry.acquire("account").await.unwrap());
        }

        let waiting_registry = Arc::clone(&registry);
        let waiting =
            tokio::spawn(async move { waiting_registry.acquire("account").await.unwrap() });
        assert!(
            tokio::time::timeout(Duration::from_millis(30), waiting)
                .await
                .is_err(),
            "a session beyond the account limit must wait"
        );

        drop(interactive_permits);
        drop(sync_permits);
    }

    #[tokio::test]
    async fn reuses_the_same_live_budget_for_an_account() {
        let registry = SessionBudgetRegistry::default();
        let first = registry.budget("account");
        let second = registry.budget("account");
        let other = registry.budget("other");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &other));
    }
}

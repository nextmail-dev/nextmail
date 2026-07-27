use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use tokio::sync::RwLock;

/// Coordinates operations that resolve and use mutable IMAP mailbox paths.
///
/// Ordinary reads and message mutations may continue concurrently. Folder
/// structure changes take the exclusive side so they cannot rename or delete
/// a path while a sync, body fetch, or queued message operation is using it.
/// Weak references keep the registry free of inactive account entries without
/// adding a cleanup worker.
#[derive(Default)]
pub(super) struct MailboxPathLockRegistry {
    locks: Mutex<HashMap<String, Weak<RwLock<()>>>>,
}

impl MailboxPathLockRegistry {
    pub(super) fn lock(&self, account_id: &str) -> Arc<RwLock<()>> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = locks.get(account_id).and_then(Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(RwLock::new(()));
        locks.insert(account_id.to_owned(), Arc::downgrade(&lock));
        lock
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::MailboxPathLockRegistry;

    #[tokio::test]
    async fn allows_concurrent_readers_and_serializes_a_writer() {
        let registry = MailboxPathLockRegistry::default();
        let lock = registry.lock("account");
        let first_reader = lock.read().await;
        let second_reader = lock.read().await;

        let writer_lock = Arc::clone(&lock);
        let writer = tokio::spawn(async move {
            let _guard = writer_lock.write().await;
        });
        tokio::pin!(writer);
        assert!(
            tokio::time::timeout(Duration::from_millis(30), &mut writer)
                .await
                .is_err(),
            "a folder structure mutation must wait for active path readers"
        );

        drop(second_reader);
        drop(first_reader);
        tokio::time::timeout(Duration::from_secs(1), &mut writer)
            .await
            .expect("writer should proceed after existing path readers finish")
            .expect("writer task should finish");
    }

    #[tokio::test]
    async fn keeps_account_locks_independent() {
        let registry = MailboxPathLockRegistry::default();
        let first = registry.lock("first");
        let second = registry.lock("second");
        let _first_writer = first.write().await;

        let _second_writer = tokio::time::timeout(Duration::from_millis(30), second.write())
            .await
            .expect("another account must not share the path lock");
    }

    #[test]
    fn reuses_the_same_live_lock_for_an_account() {
        let registry = MailboxPathLockRegistry::default();
        let first = registry.lock("account");
        let second = registry.lock("account");
        assert!(Arc::ptr_eq(&first, &second));
    }
}

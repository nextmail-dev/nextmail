use async_imap::Session;
use async_trait::async_trait;
use futures_util::future::try_join_all;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, RwLock, Weak},
    time::{Duration, Instant},
};

use super::{
    connection::{connect_session, BoxedImapTransport},
    path_lock::MailboxPathLockRegistry,
    session::{
        append_message_session, apply_mailbox_operation_session, apply_operation_session,
        fetch_attachment_session, fetch_message_body_session, fetch_message_session,
        replace_draft_session,
    },
    session_budget::{SessionBudgetRegistry, SYNC_SESSION_COUNT},
    sync_mailbox_session, sync_session, BodystructureMode, SyncSessionOutcome,
};
use crate::core::{
    CommandResult, ImapAccountConfig, ImapSyncProvider, MailSyncSink, MailboxSyncTarget,
    RemoteMailboxOperation, RemoteMailboxOperationOutcome, RemoteMessage, RemoteMessageBody,
    RemoteOperation, RemoteOperationOutcome, SyncObserver,
};

#[derive(Default)]
pub struct AsyncImapProvider {
    session_budgets: SessionBudgetRegistry,
    mailbox_path_locks: MailboxPathLockRegistry,
    bodystructure_incompatible_accounts: RwLock<HashSet<String>>,
    sync_sessions: Arc<Mutex<HashMap<String, CachedSyncSessions>>>,
    sync_locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

struct CachedSyncSessions {
    fingerprint: u64,
    idle_since: Instant,
    permits: Vec<tokio::sync::OwnedSemaphorePermit>,
    sessions: Vec<Session<BoxedImapTransport>>,
}

const SYNC_SESSION_IDLE_TTL: Duration = Duration::from_secs(120);

#[async_trait]
impl ImapSyncProvider for AsyncImapProvider {
    async fn synchronize(
        &self,
        account: &ImapAccountConfig,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()> {
        // Only one full sync may check out the reusable three-session pool for
        // an account. Otherwise a second sync can consume the one interactive
        // permit and wait forever for two permits held by the first pool.
        let sync_lock = self.sync_lock(&account.account_id);
        let _sync_guard = sync_lock.lock().await;
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        // A full sync leases three of the four per-account slots. The remaining
        // slot lets interactive body/attachment and pending-operation requests
        // proceed without opening a fifth connection that can cause stricter
        // servers to reset one of the existing sync sessions.
        let mut mode = self.bodystructure_mode(account);
        loop {
            let (session_permits, mut pool) = self.checkout_sync_sessions(account).await?;
            let result = sync_session(&mut pool, account, sink, observer, mode).await;
            match result? {
                SyncSessionOutcome::Complete => {
                    self.store_sync_sessions(account, session_permits, pool);
                    return Ok(());
                }
                SyncSessionOutcome::BodystructureIncompatible
                    if mode == BodystructureMode::Enabled =>
                {
                    self.remember_bodystructure_incompatible(account);
                    mode = BodystructureMode::Disabled;
                    tracing::warn!(
                        account_id = %account.account_id,
                        "reconnecting IMAP sync with BODYSTRUCTURE disabled"
                    );
                }
                SyncSessionOutcome::BodystructureIncompatible => {
                    return Err(crate::core::CommandError::retryable(
                        "sync.imap_connection_failed",
                    ));
                }
            }
        }
    }

    async fn fetch_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessage> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        fetch_message_session(session, mailbox_name, uid, expected_uid_validity).await
    }

    async fn fetch_message_body(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessageBody> {
        if self.bodystructure_mode(account) == BodystructureMode::Disabled {
            return Err(crate::core::CommandError::new(
                super::SELECTIVE_FETCH_UNSUPPORTED,
            ));
        }
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        let result =
            fetch_message_body_session(session, mailbox_name, uid, expected_uid_validity).await;
        if result
            .as_ref()
            .is_err_and(|error| error.code == super::SELECTIVE_FETCH_UNSUPPORTED)
        {
            self.remember_bodystructure_incompatible(account);
        }
        result
    }

    async fn fetch_attachment(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
        imap_section: &str,
    ) -> CommandResult<Vec<u8>> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        fetch_attachment_session(
            session,
            mailbox_name,
            uid,
            expected_uid_validity,
            imap_section,
        )
        .await
    }

    async fn synchronize_mailbox(
        &self,
        account: &ImapAccountConfig,
        mailbox: &MailboxSyncTarget,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let mut mode = self.bodystructure_mode(account);
        loop {
            let (_permit, session) = self.connect_budgeted_session(account).await?;
            match sync_mailbox_session(session, account, mailbox, sink, observer, mode).await? {
                SyncSessionOutcome::Complete => return Ok(()),
                SyncSessionOutcome::BodystructureIncompatible
                    if mode == BodystructureMode::Enabled =>
                {
                    self.remember_bodystructure_incompatible(account);
                    mode = BodystructureMode::Disabled;
                }
                SyncSessionOutcome::BodystructureIncompatible => {
                    return Err(crate::core::CommandError::retryable(
                        "sync.imap_connection_failed",
                    ));
                }
            }
        }
    }

    async fn apply_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteOperation,
    ) -> CommandResult<RemoteOperationOutcome> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        apply_operation_session(session, operation).await
    }

    async fn apply_mailbox_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteMailboxOperation,
    ) -> CommandResult<RemoteMailboxOperationOutcome> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.write().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        apply_mailbox_operation_session(session, operation).await
    }

    async fn append_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        flags: &str,
        raw: &[u8],
    ) -> CommandResult<()> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        append_message_session(session, mailbox_name, flags, raw).await
    }

    async fn replace_draft(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        draft_id: &str,
        raw: &[u8],
    ) -> CommandResult<RemoteOperationOutcome> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        replace_draft_session(session, mailbox_name, draft_id, raw).await
    }
}

impl AsyncImapProvider {
    fn sync_lock(&self, account_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .sync_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = locks.get(account_id).and_then(Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(account_id.to_owned(), Arc::downgrade(&lock));
        lock
    }

    async fn checkout_sync_sessions(
        &self,
        account: &ImapAccountConfig,
    ) -> CommandResult<(
        Vec<tokio::sync::OwnedSemaphorePermit>,
        Vec<Session<BoxedImapTransport>>,
    )> {
        let fingerprint = account_fingerprint(account);
        let cached = self
            .sync_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&account.account_id);
        if let Some(mut cached) = cached.filter(|cached| {
            cached.fingerprint == fingerprint
                && cached.idle_since.elapsed() <= SYNC_SESSION_IDLE_TTL
                && cached.permits.len() == SYNC_SESSION_COUNT
                && cached.sessions.len() == SYNC_SESSION_COUNT
        }) {
            let healthy =
                futures_util::future::join_all(cached.sessions.iter_mut().map(Session::noop))
                    .await
                    .into_iter()
                    .all(|result| result.is_ok());
            if healthy {
                tracing::debug!(
                    account_id = %account.account_id,
                    sessions = cached.sessions.len(),
                    "reusing healthy IMAP sync sessions"
                );
                return Ok((cached.permits, cached.sessions));
            }
            tracing::warn!(
                account_id = %account.account_id,
                "discarding stale IMAP sync sessions after NOOP failed"
            );
        }
        let permits = try_join_all(
            (0..SYNC_SESSION_COUNT).map(|_| self.session_budgets.acquire(&account.account_id)),
        )
        .await?;
        let sessions =
            try_join_all((0..SYNC_SESSION_COUNT).map(|_| connect_session(account))).await?;
        Ok((permits, sessions))
    }

    fn store_sync_sessions(
        &self,
        account: &ImapAccountConfig,
        permits: Vec<tokio::sync::OwnedSemaphorePermit>,
        sessions: Vec<Session<BoxedImapTransport>>,
    ) {
        let idle_since = Instant::now();
        self.sync_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                account.account_id.clone(),
                CachedSyncSessions {
                    fingerprint: account_fingerprint(account),
                    idle_since,
                    permits,
                    sessions,
                },
            );
        let pools = Arc::clone(&self.sync_sessions);
        let account_id = account.account_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(SYNC_SESSION_IDLE_TTL).await;
            let mut pools = pools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if pools
                .get(&account_id)
                .is_some_and(|cached| cached.idle_since == idle_since)
            {
                pools.remove(&account_id);
                tracing::debug!(%account_id, "closed idle IMAP sync sessions");
            }
        });
    }

    fn bodystructure_mode(&self, account: &ImapAccountConfig) -> BodystructureMode {
        if self
            .bodystructure_incompatible_accounts
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&account.account_id)
        {
            BodystructureMode::Disabled
        } else {
            BodystructureMode::Enabled
        }
    }

    fn remember_bodystructure_incompatible(&self, account: &ImapAccountConfig) {
        self.bodystructure_incompatible_accounts
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(account.account_id.clone());
    }

    async fn connect_budgeted_session(
        &self,
        account: &ImapAccountConfig,
    ) -> CommandResult<(
        tokio::sync::OwnedSemaphorePermit,
        Session<BoxedImapTransport>,
    )> {
        let permit = self.session_budgets.acquire(&account.account_id).await?;
        match connect_session(account).await {
            Ok(session) => Ok((permit, session)),
            Err(error) => {
                drop(permit);
                Err(error)
            }
        }
    }
}

fn account_fingerprint(account: &ImapAccountConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    account.host.hash(&mut hasher);
    account.port.hash(&mut hasher);
    match account.security {
        crate::core::ConnectionSecurity::None => 0_u8,
        crate::core::ConnectionSecurity::StartTls => 1_u8,
        crate::core::ConnectionSecurity::Tls => 2_u8,
    }
    .hash(&mut hasher);
    account.username.hash(&mut hasher);
    account.password.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{account_fingerprint, AsyncImapProvider};
    use crate::core::{ConnectionSecurity, ImapAccountConfig};

    fn account() -> ImapAccountConfig {
        ImapAccountConfig {
            account_id: "account".to_owned(),
            account_slot_id: "slot".to_owned(),
            download_full_messages: false,
            host: "imap.example.test".to_owned(),
            port: 993,
            security: ConnectionSecurity::Tls,
            username: "person@example.test".to_owned(),
            password: "secret".to_owned(),
        }
    }

    #[test]
    fn serializes_full_syncs_for_the_same_account_only() {
        let provider = AsyncImapProvider::default();
        let first = provider.sync_lock("account");
        let second = provider.sync_lock("account");
        let other = provider.sync_lock("other");

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &other));
    }

    #[test]
    fn invalidates_reusable_sessions_when_connection_credentials_change() {
        let original = account();
        let mut changed = original.clone();
        changed.password = "new-secret".to_owned();

        assert_ne!(
            account_fingerprint(&original),
            account_fingerprint(&changed)
        );
    }
}

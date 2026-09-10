use async_imap::Session;
use async_trait::async_trait;
use futures_util::future::try_join_all;
use std::{collections::HashSet, sync::RwLock};

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
}

#[async_trait]
impl ImapSyncProvider for AsyncImapProvider {
    async fn synchronize(
        &self,
        account: &ImapAccountConfig,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()> {
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        // A full sync leases two of the three per-account slots. The remaining
        // slot lets interactive body/attachment and pending-operation requests
        // proceed without opening a fourth connection that can cause stricter
        // servers to reset one of the existing sync sessions.
        let mut mode = self.bodystructure_mode(account);
        loop {
            let budgeted = try_join_all(
                (0..SYNC_SESSION_COUNT).map(|_| self.connect_budgeted_session(account)),
            )
            .await?;
            let (session_permits, pool): (Vec<_>, Vec<_>) = budgeted.into_iter().unzip();
            let result = sync_session(pool, account, sink, observer, mode).await;
            drop(session_permits);
            match result? {
                SyncSessionOutcome::Complete => return Ok(()),
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

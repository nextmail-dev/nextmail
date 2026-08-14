use async_imap::Session;
use async_trait::async_trait;
use futures_util::future::try_join_all;

use super::{
    connection::{connect_session, BoxedImapTransport},
    path_lock::MailboxPathLockRegistry,
    session::{
        append_message_session, apply_mailbox_operation_session, apply_operation_session,
        fetch_attachment_session, fetch_message_body_session, fetch_message_session,
        replace_draft_session,
    },
    session_budget::{SessionBudgetRegistry, SYNC_SESSION_COUNT},
    sync_mailbox_session, sync_session,
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
        // A full sync leases four of the six per-account slots. The remaining
        // two slots let interactive body/attachment and pending-operation
        // requests proceed without opening a seventh connection that can cause
        // stricter servers to reset one of the existing sync sessions.
        let budgeted =
            try_join_all((0..SYNC_SESSION_COUNT).map(|_| self.connect_budgeted_session(account)))
                .await?;
        let (session_permits, pool): (Vec<_>, Vec<_>) = budgeted.into_iter().unzip();
        let result = sync_session(pool, account, sink, observer).await;
        drop(session_permits);
        result
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
        let path_lock = self.mailbox_path_locks.lock(&account.account_id);
        let _path_guard = path_lock.read().await;
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        fetch_message_body_session(session, mailbox_name, uid, expected_uid_validity).await
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
        let (_permit, session) = self.connect_budgeted_session(account).await?;
        sync_mailbox_session(session, account, mailbox, sink, observer).await
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

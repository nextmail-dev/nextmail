use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use crate::core::{
    AccountManagementDetail, AccountRemovalImpact, AccountRuntimeState, AccountRuntimeSummary,
    AttachmentSummary, CommandError, CommandResult, ImapAccountConfig, ImapSyncProvider,
    InboxWatchOutcome, MailSyncSink, MailboxRole, MailboxSummary, MessageDetail, MessageListPage,
    PendingOperationKind, PendingOperationSummary, RemoteOperation, RemoteOperationKind,
    RemoteOperationOutcome, SyncNotice, SyncObserver, SyncPhase, SyncPolicy, SyncProgress,
};
use crate::storage::{MailRepository, MailRepositoryProvider, PendingOperationWork};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::{Notify, OnceCell, Semaphore};

use crate::adapters::{open_prepared_attachment, AttachmentOpener};
use crate::application::AppService;

pub struct MailRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    repository: OnceCell<Arc<MailRepository>>,
    recovery: OnceCell<()>,
    progress: RwLock<HashMap<String, SyncProgress>>,
    runtime_states: RwLock<HashMap<String, AccountRuntimeSummary>>,
    supervisors: RwLock<HashMap<String, Arc<AccountSupervisor>>>,
    provider: Arc<dyn ImapSyncProvider>,
    repository_provider: Arc<dyn MailRepositoryProvider>,
    attachment_opener: Arc<dyn AttachmentOpener>,
    network_limit: Arc<Semaphore>,
    next_generation: AtomicU64,
    started: AtomicBool,
}

impl MailRuntime {
    pub fn new(
        app: AppHandle,
        service: Arc<AppService>,
        provider: Arc<dyn ImapSyncProvider>,
        repository_provider: Arc<dyn MailRepositoryProvider>,
        attachment_opener: Arc<dyn AttachmentOpener>,
    ) -> Self {
        Self {
            app,
            service,
            repository: OnceCell::new(),
            recovery: OnceCell::new(),
            progress: RwLock::new(HashMap::new()),
            runtime_states: RwLock::new(HashMap::new()),
            supervisors: RwLock::new(HashMap::new()),
            provider,
            repository_provider,
            attachment_opener,
            network_limit: Arc::new(Semaphore::new(2)),
            next_generation: AtomicU64::new(1),
            started: AtomicBool::new(false),
        }
    }

    pub fn start(self: &Arc<Self>) {
        self.started.store(true, Ordering::Release);
        self.reconcile_accounts();
    }

    pub fn reconcile_accounts(self: &Arc<Self>) {
        let Ok(accounts) = self.service.list_account_summaries() else {
            return;
        };
        let configured = accounts
            .iter()
            .map(|account| account.id.clone())
            .collect::<std::collections::HashSet<_>>();
        let existing = self
            .supervisors
            .read()
            .map(|values| values.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for account_id in existing {
            if !configured.contains(&account_id) {
                self.stop_account(&account_id, AccountRuntimeState::Stopped);
            }
        }
        if !self.started.load(Ordering::Acquire) {
            return;
        }
        for account in accounts {
            self.ensure_supervisor(&account.id);
        }
    }

    pub fn restart_account(self: &Arc<Self>, account_id: &str) {
        self.stop_account(account_id, AccountRuntimeState::Stopped);
        if self.started.load(Ordering::Acquire) {
            self.ensure_supervisor(account_id);
        }
    }

    pub fn begin_remove_account(&self, account_id: &str) {
        self.stop_account(account_id, AccountRuntimeState::Removing);
    }

    pub fn wake_account(&self, account_id: &str) {
        if let Some(supervisor) = self.supervisor(account_id) {
            supervisor
                .background_sync_requested
                .store(true, Ordering::Release);
            supervisor.wake.notify_one();
        }
    }

    pub fn wake_account_by_slot(&self, account_slot_id: &str) {
        if let Ok(account) = self.service.account_record_for_slot(account_slot_id) {
            self.wake_account(&account.id);
        }
    }

    pub fn report_account_error_by_slot(&self, account_slot_id: &str, error: &CommandError) {
        if let Ok(account) = self.service.account_record_for_slot(account_slot_id) {
            self.handle_runtime_error(&account.id, error, Duration::from_secs(5));
            if is_authentication_error(&error.code) {
                if let Some(supervisor) = self.supervisor(&account.id) {
                    supervisor.wake.notify_one();
                }
            }
        }
    }

    pub fn ensure_account_writable(&self, account_id: &str) -> CommandResult<()> {
        self.service.account_record(account_id)?;
        if self.runtime_state_is(account_id, AccountRuntimeState::Removing) {
            return Err(CommandError::new("account.removing"));
        }
        Ok(())
    }

    pub fn list_account_runtime_summaries(&self) -> Vec<AccountRuntimeSummary> {
        let accounts = self.service.list_account_summaries().unwrap_or_default();
        let states = self.runtime_states.read().ok();
        accounts
            .into_iter()
            .map(|account| {
                states
                    .as_ref()
                    .and_then(|values| values.get(&account.id).cloned())
                    .unwrap_or_else(|| AccountRuntimeSummary::stopped(account.id))
            })
            .collect()
    }

    pub async fn get_account_removal_impact(
        &self,
        account_id: &str,
    ) -> CommandResult<AccountRemovalImpact> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .account_removal_impact(&account.data_slot_id)
            .await
    }

    fn ensure_supervisor(self: &Arc<Self>, account_id: &str) {
        if self.supervisor(account_id).is_some() {
            return;
        }
        if self.service.account_record(account_id).is_err() {
            return;
        }
        let generation = self.next_generation.fetch_add(1, Ordering::AcqRel);
        let supervisor = Arc::new(AccountSupervisor::new(account_id, generation));
        let inserted = self.supervisors.write().ok().is_some_and(|mut values| {
            if values.contains_key(account_id) {
                false
            } else {
                values.insert(account_id.to_owned(), Arc::clone(&supervisor));
                true
            }
        });
        if !inserted {
            return;
        }
        self.update_runtime_state(account_id, AccountRuntimeState::Starting, None, None);
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            runtime.supervisor_loop(supervisor).await;
        });
    }

    fn stop_account(&self, account_id: &str, state: AccountRuntimeState) {
        let supervisor = self
            .supervisors
            .write()
            .ok()
            .and_then(|mut values| values.remove(account_id));
        if let Some(supervisor) = supervisor {
            supervisor.stopped.store(true, Ordering::Release);
            supervisor.wake.notify_waiters();
        }
        self.update_runtime_state(account_id, state, None, None);
    }

    fn supervisor(&self, account_id: &str) -> Option<Arc<AccountSupervisor>> {
        self.supervisors
            .read()
            .ok()
            .and_then(|values| values.get(account_id).cloned())
    }

    fn is_current_supervisor(&self, account_id: &str, generation: u64) -> bool {
        self.supervisor(account_id)
            .is_some_and(|supervisor| supervisor.generation == generation)
    }

    async fn supervisor_loop(self: &Arc<Self>, supervisor: Arc<AccountSupervisor>) {
        let mut retry_delay = Duration::from_secs(2);
        let mut startup_sync_complete = false;
        while !supervisor.stopped.load(Ordering::Acquire) {
            let account_id = supervisor.account_id.clone();
            if self.service.account_record(&account_id).is_err() {
                break;
            }
            if self.runtime_state_is(&account_id, AccountRuntimeState::ReauthRequired) {
                supervisor.wake.notified().await;
                continue;
            }
            let Ok(repository) = self.repository().await else {
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                continue;
            };
            let _ = self
                .recovery
                .get_or_try_init(|| async {
                    repository.operations().recover_pending_operations().await
                })
                .await;

            if !startup_sync_complete {
                let sync_result = self
                    .run_sync(&account_id, supervisor.generation, true)
                    .await;
                if supervisor.stopped.load(Ordering::Acquire)
                    || !self.is_current_supervisor(&account_id, supervisor.generation)
                {
                    break;
                }
                if sync_result.is_ok() {
                    startup_sync_complete = true;
                    supervisor
                        .manual_sync_requested
                        .store(false, Ordering::Release);
                    supervisor
                        .background_sync_requested
                        .store(false, Ordering::Release);
                    retry_delay = Duration::from_secs(2);
                    let _ = self
                        .drain_pending_operations(&account_id, supervisor.generation)
                        .await;
                } else {
                    if self.runtime_state_is(&account_id, AccountRuntimeState::ReauthRequired) {
                        supervisor.wake.notified().await;
                        continue;
                    }
                    self.update_runtime_state(
                        &account_id,
                        AccountRuntimeState::Retrying,
                        None,
                        Some(unix_timestamp() + retry_delay.as_secs() as i64),
                    );
                    tokio::select! {
                        _ = supervisor.wake.notified() => {},
                        _ = tokio::time::sleep(retry_delay) => {},
                    }
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                    continue;
                }
            }

            let config = match self.imap_config(&account_id).await {
                Ok(config) => config,
                Err(error) => {
                    self.handle_runtime_error(&account_id, &error, retry_delay);
                    supervisor.wake.notified().await;
                    continue;
                }
            };
            let watch = self
                .provider
                .wait_for_inbox_change(&config, Duration::from_secs(25 * 60));
            let wake = tokio::select! {
                _ = supervisor.wake.notified() => SupervisorWake::Requested,
                _ = tokio::time::sleep(Duration::from_secs(5 * 60)) => SupervisorWake::BackgroundSync,
                result = watch => {
                    match result {
                        Ok(InboxWatchOutcome::Unsupported) => {
                            tokio::select! {
                                _ = supervisor.wake.notified() => SupervisorWake::Requested,
                                _ = tokio::time::sleep(Duration::from_secs(60)) => SupervisorWake::BackgroundSync,
                            }
                        }
                        Ok(InboxWatchOutcome::Changed | InboxWatchOutcome::Timeout) => SupervisorWake::BackgroundSync,
                        Err(_) => {
                            tokio::select! {
                                _ = supervisor.wake.notified() => SupervisorWake::Requested,
                                _ = tokio::time::sleep(retry_delay) => SupervisorWake::Retry,
                            }
                        }
                    }
                }
            };

            if supervisor.stopped.load(Ordering::Acquire) {
                break;
            }

            if wake == SupervisorWake::Retry {
                self.update_runtime_state(
                    &account_id,
                    AccountRuntimeState::Retrying,
                    Some("sync.idle_failed".to_owned()),
                    Some(unix_timestamp() + retry_delay.as_secs() as i64),
                );
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                continue;
            }

            let manual = supervisor
                .manual_sync_requested
                .swap(false, Ordering::AcqRel);
            let background_requested = supervisor
                .background_sync_requested
                .swap(false, Ordering::AcqRel);
            if manual || background_requested || wake == SupervisorWake::BackgroundSync {
                let sync_result = self
                    .run_sync(&account_id, supervisor.generation, manual)
                    .await;
                if supervisor.stopped.load(Ordering::Acquire)
                    || !self.is_current_supervisor(&account_id, supervisor.generation)
                {
                    break;
                }
                if sync_result.is_ok() {
                    retry_delay = Duration::from_secs(2);
                } else {
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                }
            }

            if self
                .drain_pending_operations(&account_id, supervisor.generation)
                .await
                .unwrap_or(false)
            {
                retry_delay = Duration::from_secs(2);
            }
        }
        if self.is_current_supervisor(&supervisor.account_id, supervisor.generation) {
            self.stop_account(&supervisor.account_id, AccountRuntimeState::Stopped);
        }
    }

    pub async fn list_mailboxes(&self, account_id: &str) -> CommandResult<Vec<MailboxSummary>> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .read()
            .list_mailboxes(account_id, &account.data_slot_id)
            .await
    }

    pub async fn list_messages(
        &self,
        account_id: &str,
        mailbox_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .read()
            .list_messages(&account.data_slot_id, mailbox_id, cursor, limit)
            .await
    }

    pub async fn search_messages(
        &self,
        account_id: &str,
        mailbox_id: &str,
        query: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> CommandResult<MessageListPage> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .read()
            .search_messages(&account.data_slot_id, mailbox_id, query, cursor, limit)
            .await
    }

    pub async fn get_message_detail(
        &self,
        account_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<MessageDetail> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .read()
            .get_message_detail(&account.data_slot_id, message_id, mailbox_id)
            .await
    }

    pub fn get_sync_progress(&self, account_id: &str) -> SyncProgress {
        self.progress
            .read()
            .ok()
            .and_then(|values| values.get(account_id).cloned())
            .unwrap_or_else(|| SyncProgress::idle(account_id))
    }

    pub async fn get_account_management_detail(
        &self,
        account_id: &str,
    ) -> CommandResult<AccountManagementDetail> {
        let account = self.service.account_record(account_id)?;
        let sync_policy = self
            .repository()
            .await?
            .read()
            .get_sync_policy(&account.data_slot_id)
            .await?;
        Ok(AccountManagementDetail {
            id: account.id,
            email: account.email,
            display_name: account.display_name,
            incoming_host: account.incoming.host,
            incoming_port: account.incoming.port,
            security: account.incoming.security,
            sync_policy,
        })
    }

    pub async fn set_account_sync_policy(
        self: &Arc<Self>,
        account_id: &str,
        sync_policy: SyncPolicy,
    ) -> CommandResult<SyncPolicy> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let updated = self
            .repository()
            .await?
            .read()
            .set_sync_policy(&account.data_slot_id, sync_policy)
            .await?;
        self.wake_account(account_id);
        Ok(updated)
    }

    pub fn sync_now(&self, account_id: &str) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let supervisor = self
            .supervisor(account_id)
            .ok_or_else(|| CommandError::new("account.runtime_stopped"))?;
        supervisor
            .manual_sync_requested
            .store(true, Ordering::Release);
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        supervisor.wake.notify_one();
        Ok(())
    }

    pub async fn set_message_read(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        read: bool,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .operations()
            .queue_set_read(&account.data_slot_id, mailbox_id, message_ids, read)
            .await?;
        self.emit_local_change(account_id, mailbox_id, message_ids);
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn set_message_flagged(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        flagged: bool,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .operations()
            .queue_set_flagged(&account.data_slot_id, mailbox_id, message_ids, flagged)
            .await?;
        self.emit_local_change(account_id, mailbox_id, message_ids);
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn transfer_messages(
        &self,
        account_id: &str,
        source_mailbox_id: &str,
        destination_mailbox_id: &str,
        message_ids: &[String],
        copy: bool,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .operations()
            .queue_transfer(
                &account.data_slot_id,
                source_mailbox_id,
                destination_mailbox_id,
                message_ids,
                copy,
            )
            .await?;
        self.emit_local_change(account_id, source_mailbox_id, message_ids);
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn delete_messages(
        &self,
        account_id: &str,
        source_mailbox_id: &str,
        message_ids: &[String],
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let role = repository
            .mailbox_roles()
            .mailbox_role_for_id(&account.data_slot_id, source_mailbox_id)
            .await?;
        if role == MailboxRole::Trash {
            repository
                .operations()
                .queue_permanent_delete(&account.data_slot_id, source_mailbox_id, message_ids)
                .await?;
        } else {
            let (trash_id, _) = repository
                .mailbox_roles()
                .mailbox_for_role(&account.data_slot_id, MailboxRole::Trash)
                .await?
                .ok_or_else(|| CommandError::new("mailbox.trash_not_mapped"))?;
            repository
                .operations()
                .queue_transfer(
                    &account.data_slot_id,
                    source_mailbox_id,
                    &trash_id,
                    message_ids,
                    false,
                )
                .await?;
        }
        self.emit_local_change(account_id, source_mailbox_id, message_ids);
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn archive_messages(
        &self,
        account_id: &str,
        source_mailbox_id: &str,
        message_ids: &[String],
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let (archive_id, _) = repository
            .mailbox_roles()
            .mailbox_for_role(&account.data_slot_id, MailboxRole::Archive)
            .await?
            .ok_or_else(|| CommandError::new("mailbox.archive_not_mapped"))?;
        repository
            .operations()
            .queue_transfer(
                &account.data_slot_id,
                source_mailbox_id,
                &archive_id,
                message_ids,
                false,
            )
            .await?;
        self.emit_local_change(account_id, source_mailbox_id, message_ids);
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn set_mailbox_role_mapping(
        &self,
        account_id: &str,
        role: MailboxRole,
        mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .mailbox_roles()
            .set_mailbox_role_mapping(&account.data_slot_id, role, mailbox_id)
            .await?;
        self.emit_mailbox_change(account_id, mailbox_id.unwrap_or_default(), 0);
        Ok(())
    }

    pub async fn list_pending_operation_status(
        &self,
        account_id: &str,
    ) -> CommandResult<Vec<PendingOperationSummary>> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .operations()
            .list_pending_operation_status(account_id, &account.data_slot_id)
            .await
    }

    pub async fn retry_pending_operation(
        &self,
        account_id: &str,
        operation_id: &str,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .operations()
            .retry_pending_operation(&account.data_slot_id, operation_id)
            .await?;
        self.wake_account(account_id);
        Ok(())
    }

    pub async fn request_raw_message(
        &self,
        account_id: &str,
        message_id: &str,
    ) -> CommandResult<String> {
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        let raw = match repository
            .read()
            .raw_message(&account.data_slot_id, message_id)
            .await?
        {
            Some(raw) => raw,
            None => {
                self.fetch_and_store_message(&account.id, message_id)
                    .await?;
                repository
                    .read()
                    .raw_message(&account.data_slot_id, message_id)
                    .await?
                    .ok_or_else(|| CommandError::new("message.raw_unavailable"))?
            }
        };
        Ok(String::from_utf8_lossy(&raw).into_owned())
    }

    pub async fn request_message_body(
        &self,
        account_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<MessageDetail> {
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        if let Some(raw) = repository
            .read()
            .raw_message(&account.data_slot_id, message_id)
            .await?
        {
            let body = tokio::task::spawn_blocking(move || {
                crate::protocols::sanitize_raw_message_body(&raw)
            })
            .await
            .map_err(|_| CommandError::new("message.mime_parse_failed"))?;
            if let Some(body) = body {
                repository
                    .sync_sink()
                    .replace_message_body(
                        &account.data_slot_id,
                        message_id,
                        body.plain_text.as_deref(),
                        body.safe_html.as_deref(),
                        body.remote_images_blocked,
                    )
                    .await?;
                let detail = repository
                    .read()
                    .get_message_detail(&account.data_slot_id, message_id, mailbox_id)
                    .await?;
                let _ = self.app.emit(
                    "message-content-changed",
                    MessageContentChangedEvent {
                        account_id: account_id.to_owned(),
                        message_id: message_id.to_owned(),
                        revision: detail.revision,
                    },
                );
                return Ok(detail);
            }
        }
        self.fetch_and_store_message(account_id, message_id).await?;
        self.get_message_detail(account_id, message_id, mailbox_id)
            .await
    }

    pub async fn request_attachment(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<AttachmentSummary> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let current = repository
            .read()
            .attachment_summary(&account.data_slot_id, attachment_id)
            .await?;
        if current.availability == crate::core::ContentAvailability::Available {
            return Ok(current);
        }
        self.ensure_account_writable(account_id)?;
        let (message_id, part_index) = repository
            .read()
            .attachment_context(&account.data_slot_id, attachment_id)
            .await?;
        let raw = match repository
            .read()
            .raw_message(&account.data_slot_id, &message_id)
            .await?
        {
            Some(raw) => raw,
            None => {
                self.fetch_and_store_message(&account.id, &message_id)
                    .await?;
                repository
                    .read()
                    .raw_message(&account.data_slot_id, &message_id)
                    .await?
                    .ok_or_else(|| CommandError::new("message.raw_unavailable"))?
            }
        };
        let content = crate::protocols::extract_attachment(&raw, part_index)?;
        repository
            .read()
            .store_attachment_content(&account.data_slot_id, attachment_id, &content)
            .await
    }

    pub async fn open_message_attachment(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<()> {
        let prepared = self
            .prepare_message_attachment(account_id, attachment_id)
            .await?;
        open_prepared_attachment(self.attachment_opener.as_ref(), &prepared)
    }

    pub async fn save_message_attachment_as(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<bool> {
        let prepared = self
            .prepare_message_attachment(account_id, attachment_id)
            .await?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.app
            .dialog()
            .file()
            .set_file_name(&prepared.file_name)
            .save_file(move |path| {
                let _ = sender.send(path);
            });
        let selected = receiver
            .await
            .map_err(|_| CommandError::new("attachment.save_dialog_failed"))?;
        let Some(selected) = selected else {
            return Ok(false);
        };
        let target = selected
            .into_path()
            .map_err(|_| CommandError::new("attachment.save_path_invalid"))?;
        if target == prepared.path {
            return Ok(true);
        }
        tokio::fs::copy(&prepared.path, target)
            .await
            .map_err(|_| CommandError::new("attachment.save_failed"))?;
        Ok(true)
    }

    async fn prepare_message_attachment(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<crate::storage::PreparedAttachmentFile> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        match repository
            .read()
            .prepare_attachment_file(&account.data_slot_id, attachment_id)
            .await
        {
            Ok(prepared) => Ok(prepared),
            Err(error) if error.code == "attachment.content_unavailable" => {
                self.request_attachment(account_id, attachment_id).await?;
                repository
                    .read()
                    .prepare_attachment_file(&account.data_slot_id, attachment_id)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    async fn fetch_and_store_message(
        &self,
        account_id: &str,
        message_id: &str,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        let context = repository
            .read()
            .remote_message_context(&account.data_slot_id, message_id)
            .await?;
        let config = self.imap_config(&account.id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        let message = self
            .provider
            .fetch_message(
                &config,
                &context.mailbox_name,
                context.uid,
                context.uid_validity,
            )
            .await?;
        repository
            .sync_sink()
            .upsert_message(&account.data_slot_id, &context.mailbox_id, &message)
            .await?;
        let revision = repository
            .read()
            .get_message_detail(&account.data_slot_id, message_id, Some(&context.mailbox_id))
            .await?
            .revision;
        let _ = self.app.emit(
            "message-content-changed",
            MessageContentChangedEvent {
                account_id: account_id.to_owned(),
                message_id: message_id.to_owned(),
                revision,
            },
        );
        Ok(())
    }

    async fn imap_config(&self, account_id: &str) -> CommandResult<ImapAccountConfig> {
        let account = self.service.account_record(account_id)?;
        let sync_policy = self
            .repository()
            .await?
            .read()
            .get_sync_policy(&account.data_slot_id)
            .await?;
        let password = self
            .service
            .account_password(&account.credential_ref)
            .await?;
        Ok(ImapAccountConfig {
            account_id: account.id,
            account_slot_id: account.data_slot_id,
            host: account.incoming.host,
            port: account.incoming.port,
            security: account.incoming.security,
            username: account.incoming.username,
            password,
            sync_policy,
        })
    }

    pub(crate) async fn repository(&self) -> CommandResult<&Arc<MailRepository>> {
        self.repository
            .get_or_try_init(|| async {
                let data_dir = self.service.configured_data_dir()?;
                self.repository_provider.open(&data_dir).await.map(Arc::new)
            })
            .await
    }

    async fn drain_pending_operations(
        &self,
        account_id: &str,
        generation: u64,
    ) -> CommandResult<bool> {
        if !self.is_current_supervisor(account_id, generation) {
            return Ok(false);
        }
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        let config = self.imap_config(account_id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        let mut processed = false;
        while let Some(work) = repository
            .operations()
            .claim_pending_operation(&account.data_slot_id)
            .await?
        {
            if !self.is_current_supervisor(account_id, generation) {
                break;
            }
            processed = true;
            let result = self
                .run_pending_operation(&repository, &config, &work)
                .await;
            match result {
                Ok(outcome) => {
                    repository
                        .operations()
                        .complete_pending_operation(&work, outcome.cleanup_pending)
                        .await?;
                }
                Err(error) => {
                    repository
                        .operations()
                        .fail_pending_operation(&work, &error.code, error.retryable)
                        .await?;
                    self.emit_pending_operation(account_id, &work.id, "failed");
                    if error.retryable {
                        break;
                    }
                    continue;
                }
            }
            self.emit_pending_operation(account_id, &work.id, "succeeded");
            if let Some(mailbox_id) = work.source_mailbox_id.as_deref() {
                self.emit_mailbox_change(account_id, mailbox_id, 0);
            }
        }
        Ok(processed)
    }

    async fn run_pending_operation(
        &self,
        repository: &MailRepository,
        config: &ImapAccountConfig,
        work: &PendingOperationWork,
    ) -> CommandResult<RemoteOperationOutcome> {
        match work.kind {
            PendingOperationKind::AppendSent => {
                self.run_append_sent(repository, config, work).await
            }
            PendingOperationKind::AppendDraft => {
                self.run_append_draft(repository, config, work).await
            }
            _ => {
                self.provider
                    .apply_operation(config, &remote_operation(work)?)
                    .await
            }
        }
    }

    async fn run_append_sent(
        &self,
        repository: &MailRepository,
        config: &ImapAccountConfig,
        work: &PendingOperationWork,
    ) -> CommandResult<RemoteOperationOutcome> {
        let destination = required_destination(work)?;
        let hash = required_payload(work, "mimeHash", "operation.mime_missing")?;
        let raw = repository.send_jobs().read_send_mime(hash).await?;
        self.provider
            .append_message(config, destination, "(\\Seen)", &raw)
            .await?;
        Ok(RemoteOperationOutcome::default())
    }

    async fn run_append_draft(
        &self,
        repository: &MailRepository,
        config: &ImapAccountConfig,
        work: &PendingOperationWork,
    ) -> CommandResult<RemoteOperationOutcome> {
        let destination = required_destination(work)?;
        let hash = required_payload(work, "mimeHash", "operation.mime_missing")?;
        let draft_id = required_payload(work, "draftId", "operation.draft_missing")?;
        let raw = repository.send_jobs().read_send_mime(hash).await?;
        self.provider
            .replace_draft(config, destination, draft_id, &raw)
            .await
    }

    fn emit_local_change(&self, account_id: &str, mailbox_id: &str, message_ids: &[String]) {
        self.emit_mailbox_change(account_id, mailbox_id, 0);
        for message_id in message_ids {
            let _ = self.app.emit(
                "message-content-changed",
                MessageContentChangedEvent {
                    account_id: account_id.to_owned(),
                    message_id: message_id.clone(),
                    revision: 0,
                },
            );
        }
    }

    fn emit_mailbox_change(&self, account_id: &str, mailbox_id: &str, revision: u64) {
        let _ = self.app.emit(
            "mailbox-changed",
            MailboxChangedEvent {
                account_id: account_id.to_owned(),
                mailbox_id: mailbox_id.to_owned(),
                revision,
            },
        );
    }

    fn emit_pending_operation(&self, account_id: &str, operation_id: &str, status: &str) {
        let _ = self.app.emit(
            "pending-operation-changed",
            PendingOperationChangedEvent {
                account_id: account_id.to_owned(),
                operation_id: operation_id.to_owned(),
                status: status.to_owned(),
            },
        );
    }

    async fn run_sync(
        &self,
        account_id: &str,
        generation: u64,
        report_progress: bool,
    ) -> CommandResult<()> {
        if !self.is_current_supervisor(account_id, generation) {
            return Err(CommandError::new("account.runtime_stopped"));
        }
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        self.update_runtime_state(account_id, AccountRuntimeState::Syncing, None, None);
        if report_progress {
            self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        }
        let observer = RuntimeObserver {
            runtime: self,
            account_id: account_id.to_owned(),
            generation,
            report_progress,
        };
        let result = match self.imap_config(&account.id).await {
            Ok(config) => {
                let sink = repository.sync_sink();
                self.provider.synchronize(&config, &sink, &observer).await
            }
            Err(error) => Err(error),
        };
        if !self.is_current_supervisor(account_id, generation) {
            return Err(CommandError::new("account.runtime_stopped"));
        }
        match result {
            Ok(()) => {
                if report_progress {
                    self.update_progress(account_id, SyncPhase::Complete, 1, 1, None);
                }
                self.update_runtime_state(account_id, AccountRuntimeState::Ready, None, None);
                Ok(())
            }
            Err(error) => {
                if report_progress {
                    self.update_progress(
                        account_id,
                        SyncPhase::Failed,
                        0,
                        0,
                        Some(error.code.clone()),
                    );
                }
                let _ = self.app.emit(
                    "sync-failed",
                    SyncFailedEvent {
                        account_id: account_id.to_owned(),
                        code: error.code.clone(),
                        retryable: error.retryable,
                    },
                );
                self.handle_runtime_error(account_id, &error, Duration::from_secs(2));
                Err(error)
            }
        }
    }

    fn runtime_state_is(&self, account_id: &str, expected: AccountRuntimeState) -> bool {
        self.runtime_states
            .read()
            .ok()
            .and_then(|values| values.get(account_id).cloned())
            .is_some_and(|summary| summary.state == expected)
    }

    fn handle_runtime_error(&self, account_id: &str, error: &CommandError, delay: Duration) {
        if is_authentication_error(&error.code) {
            self.update_runtime_state(
                account_id,
                AccountRuntimeState::ReauthRequired,
                Some(error.code.clone()),
                None,
            );
        } else {
            self.update_runtime_state(
                account_id,
                if error.retryable {
                    AccountRuntimeState::Retrying
                } else {
                    AccountRuntimeState::Offline
                },
                Some(error.code.clone()),
                error
                    .retryable
                    .then(|| unix_timestamp() + delay.as_secs() as i64),
            );
        }
    }

    fn update_runtime_state(
        &self,
        account_id: &str,
        state: AccountRuntimeState,
        error_code: Option<String>,
        retry_at: Option<i64>,
    ) {
        let summary = if let Ok(mut values) = self.runtime_states.write() {
            let revision = values
                .get(account_id)
                .map_or(1, |current| current.revision.saturating_add(1));
            let summary = AccountRuntimeSummary {
                account_id: account_id.to_owned(),
                state,
                error_code,
                retry_at,
                revision,
            };
            values.insert(account_id.to_owned(), summary.clone());
            summary
        } else {
            return;
        };
        let _ = self.app.emit("account-runtime-status-changed", summary);
    }

    fn update_progress(
        &self,
        account_id: &str,
        phase: SyncPhase,
        completed: u64,
        total: u64,
        error_code: Option<String>,
    ) {
        let progress = if let Ok(mut values) = self.progress.write() {
            let revision = values
                .get(account_id)
                .map_or(1, |current| current.revision.saturating_add(1));
            let progress = SyncProgress {
                account_id: account_id.to_owned(),
                phase,
                completed,
                total,
                error_code,
                revision,
            };
            values.insert(account_id.to_owned(), progress.clone());
            progress
        } else {
            return;
        };
        let _ = self.app.emit("sync-progress", progress);
    }
}

struct RuntimeObserver<'a> {
    runtime: &'a MailRuntime,
    account_id: String,
    generation: u64,
    report_progress: bool,
}

struct AccountSupervisor {
    account_id: String,
    generation: u64,
    wake: Notify,
    manual_sync_requested: AtomicBool,
    background_sync_requested: AtomicBool,
    stopped: AtomicBool,
}

impl AccountSupervisor {
    fn new(account_id: &str, generation: u64) -> Self {
        Self {
            account_id: account_id.to_owned(),
            generation,
            wake: Notify::new(),
            manual_sync_requested: AtomicBool::new(false),
            background_sync_requested: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        }
    }
}

impl SyncObserver for RuntimeObserver<'_> {
    fn notify(&self, notice: SyncNotice) {
        if !self
            .runtime
            .is_current_supervisor(&self.account_id, self.generation)
        {
            return;
        }
        match notice {
            SyncNotice::Folders { completed, total } if self.report_progress => self
                .runtime
                .update_progress(&self.account_id, SyncPhase::Folders, completed, total, None),
            SyncNotice::Summaries { completed, total } if self.report_progress => {
                self.runtime.update_progress(
                    &self.account_id,
                    SyncPhase::Summaries,
                    completed,
                    total,
                    None,
                )
            }
            SyncNotice::Bodies { completed, total } if self.report_progress => self
                .runtime
                .update_progress(&self.account_id, SyncPhase::Bodies, completed, total, None),
            SyncNotice::Folders { .. }
            | SyncNotice::Summaries { .. }
            | SyncNotice::Bodies { .. } => {}
            SyncNotice::MailboxChanged {
                mailbox_id,
                revision,
            } => {
                let _ = self.runtime.app.emit(
                    "mailbox-changed",
                    MailboxChangedEvent {
                        account_id: self.account_id.clone(),
                        mailbox_id,
                        revision,
                    },
                );
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SupervisorWake {
    Requested,
    BackgroundSync,
    Retry,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MailboxChangedEvent {
    account_id: String,
    mailbox_id: String,
    revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncFailedEvent {
    account_id: String,
    code: String,
    retryable: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageContentChangedEvent {
    account_id: String,
    message_id: String,
    revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingOperationChangedEvent {
    account_id: String,
    operation_id: String,
    status: String,
}

fn required_destination(work: &PendingOperationWork) -> CommandResult<&str> {
    work.destination_mailbox_name
        .as_deref()
        .ok_or_else(|| CommandError::new("operation.destination_required"))
}

fn required_payload<'a>(
    work: &'a PendingOperationWork,
    key: &str,
    error_code: &str,
) -> CommandResult<&'a str> {
    work.payload
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| CommandError::new(error_code))
}

fn remote_operation(work: &PendingOperationWork) -> CommandResult<RemoteOperation> {
    let value = work
        .payload
        .get("value")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let kind = match work.kind {
        PendingOperationKind::SetRead => RemoteOperationKind::SetRead(value),
        PendingOperationKind::SetFlagged => RemoteOperationKind::SetFlagged(value),
        PendingOperationKind::Copy => RemoteOperationKind::Copy,
        PendingOperationKind::Move => RemoteOperationKind::Move,
        PendingOperationKind::Delete => RemoteOperationKind::Delete,
        PendingOperationKind::AppendSent | PendingOperationKind::AppendDraft => {
            return Err(CommandError::new("operation.kind_invalid"));
        }
    };
    Ok(RemoteOperation {
        kind,
        source_mailbox: work
            .source_mailbox_name
            .clone()
            .ok_or_else(|| CommandError::new("operation.source_required"))?,
        destination_mailbox: work.destination_mailbox_name.clone(),
        uid: work
            .uid
            .ok_or_else(|| CommandError::new("operation.uid_required"))?,
        uid_validity: work
            .uid_validity
            .ok_or_else(|| CommandError::new("operation.uid_required"))?,
        base_modseq: work.base_modseq,
    })
}

fn is_authentication_error(code: &str) -> bool {
    matches!(
        code,
        "credential.read_failed"
            | "sync.imap_authentication_failed"
            | "account.imap_authentication_failed"
            | "account.smtp_authentication_failed"
            | "send.smtp_authentication_failed"
    )
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_authentication_failures_require_reauthentication() {
        assert!(is_authentication_error("credential.read_failed"));
        assert!(is_authentication_error("sync.imap_authentication_failed"));
        assert!(is_authentication_error("send.smtp_authentication_failed"));
        assert!(!is_authentication_error("account.imap_timeout"));
        assert!(!is_authentication_error("account.imap_tls_failed"));
        assert!(!is_authentication_error("send.smtp_temporary_failure"));
    }
}

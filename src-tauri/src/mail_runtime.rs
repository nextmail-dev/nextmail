mod content;
mod operations;
mod runtime_support;

use runtime_support::*;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::Duration,
};

use crate::core::{
    AccountManagementDetail, AccountRemovalImpact, AccountRuntimeState, AccountRuntimeSummary,
    CommandError, CommandResult, ImapAccountConfig, ImapSyncProvider, MailSyncSink, MailboxRole,
    MailboxSummary, MessageDetail, MessageListPage, NewMailCandidate, NotificationDisplayMode,
    NotificationNavigationTarget, PendingOperationKind, RemoteOperation, RemoteOperationKind,
    SyncInterval, SyncPhase, SyncProgress,
};
use crate::storage::{MailRepository, MailRepositoryProvider, PendingOperationWork};
use tauri::{AppHandle, Emitter};
use tokio::sync::{OnceCell, Semaphore};

use crate::adapters::AttachmentOpener;
use crate::application::AppService;
use crate::notification_runtime::NotificationRuntime;

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
    notifications: Arc<NotificationRuntime>,
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
        notifications: Arc<NotificationRuntime>,
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
            notifications,
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

    pub fn request_pending_operations(&self, account_id: &str) {
        if let Some(supervisor) = self.supervisor(account_id) {
            supervisor
                .pending_operations_requested
                .store(true, Ordering::Release);
            supervisor.wake.notify_one();
        }
    }

    pub fn request_pending_operations_by_slot(&self, account_slot_id: &str) {
        if let Ok(account) = self.service.account_record_for_slot(account_slot_id) {
            self.request_pending_operations(&account.id);
        }
    }

    fn notify_sync_schedule_changed(&self, account_id: &str) {
        if let Some(supervisor) = self.supervisor(account_id) {
            supervisor.wake.notify_one();
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
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None, None);
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
        let mut startup_sync_attempted = false;
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
                supervisor.wake.notified().await;
                continue;
            };
            if let Err(error) = self
                .recovery
                .get_or_try_init(|| async {
                    repository.operations().recover_pending_operations().await
                })
                .await
            {
                tracing::warn!(
                    %account_id,
                    code = %error.code,
                    "pending operation recovery failed"
                );
            }

            if !startup_sync_attempted {
                if let Err(error) = self
                    .run_sync(&account_id, supervisor.generation, true)
                    .await
                {
                    tracing::warn!(
                        %account_id,
                        code = %error.code,
                        "startup sync ended with an error"
                    );
                }
                if supervisor.stopped.load(Ordering::Acquire)
                    || !self.is_current_supervisor(&account_id, supervisor.generation)
                {
                    break;
                }
                startup_sync_attempted = true;
                supervisor
                    .manual_sync_requested
                    .store(false, Ordering::Release);
                supervisor
                    .pending_operations_requested
                    .store(false, Ordering::Release);
                if let Err(error) = self
                    .drain_pending_operations(&account_id, supervisor.generation)
                    .await
                {
                    tracing::warn!(
                        %account_id,
                        code = %error.code,
                        "startup pending operation drain failed"
                    );
                }
                continue;
            }

            let interval = match self.sync_interval(&account_id).await {
                Ok(interval) => interval,
                Err(error) => {
                    self.handle_runtime_error(&account_id, &error, Duration::from_secs(0));
                    supervisor.wake.notified().await;
                    continue;
                }
            };
            let wake = wait_for_supervisor(&supervisor, &interval).await;

            if supervisor.stopped.load(Ordering::Acquire) {
                break;
            }

            let manual = supervisor
                .manual_sync_requested
                .swap(false, Ordering::AcqRel);
            let pending_operations = supervisor
                .pending_operations_requested
                .swap(false, Ordering::AcqRel);
            let should_sync = manual || wake == SupervisorWake::Periodic;
            if should_sync {
                if let Err(error) = self
                    .run_sync(&account_id, supervisor.generation, manual)
                    .await
                {
                    tracing::warn!(
                        %account_id,
                        code = %error.code,
                        manual,
                        "scheduled sync ended with an error"
                    );
                }
                if supervisor.stopped.load(Ordering::Acquire)
                    || !self.is_current_supervisor(&account_id, supervisor.generation)
                {
                    break;
                }
            }

            if pending_operations || should_sync {
                if let Err(error) = self
                    .drain_pending_operations(&account_id, supervisor.generation)
                    .await
                {
                    tracing::warn!(
                        %account_id,
                        code = %error.code,
                        "pending operation drain failed"
                    );
                }
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

    pub async fn resolve_notification_target(
        &self,
        candidate: &NewMailCandidate,
    ) -> Option<NotificationNavigationTarget> {
        let mailboxes = self.list_mailboxes(&candidate.account_id).await.ok()?;
        let requested = mailboxes
            .iter()
            .find(|mailbox| mailbox.id == candidate.mailbox_id && mailbox.selectable);
        let mailbox = requested
            .or_else(|| {
                mailboxes
                    .iter()
                    .find(|mailbox| mailbox.role == MailboxRole::Inbox && mailbox.selectable)
            })
            .or_else(|| mailboxes.iter().find(|mailbox| mailbox.selectable))?;
        let message_id = if requested.is_some()
            && self
                .get_message_detail(
                    &candidate.account_id,
                    &candidate.message_id,
                    Some(&candidate.mailbox_id),
                )
                .await
                .is_ok()
        {
            Some(candidate.message_id.clone())
        } else {
            None
        };
        Some(NotificationNavigationTarget {
            account_id: candidate.account_id.clone(),
            mailbox_id: mailbox.id.clone(),
            message_id,
        })
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
        let sync_interval = self
            .repository()
            .await?
            .read()
            .get_sync_interval(&account.data_slot_id)
            .await?;
        Ok(AccountManagementDetail {
            id: account.id,
            email: account.email,
            display_name: account.display_name,
            incoming_host: account.incoming.host,
            incoming_port: account.incoming.port,
            security: account.incoming.security,
            sync_interval,
        })
    }

    pub async fn set_account_sync_interval(
        self: &Arc<Self>,
        account_id: &str,
        sync_interval: SyncInterval,
    ) -> CommandResult<SyncInterval> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let updated = self
            .repository()
            .await?
            .read()
            .set_sync_interval(&account.data_slot_id, sync_interval)
            .await?;
        self.notify_sync_schedule_changed(account_id);
        Ok(updated)
    }

    pub fn sync_now(&self, account_id: &str) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        if self.runtime_state_is(account_id, AccountRuntimeState::Syncing) {
            return Err(CommandError::new("sync.already_running"));
        }
        let supervisor = self
            .supervisor(account_id)
            .ok_or_else(|| CommandError::new("account.runtime_stopped"))?;
        supervisor
            .manual_sync_requested
            .store(true, Ordering::Release);
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None, None);
        supervisor.wake.notify_one();
        Ok(())
    }

    async fn imap_config(&self, account_id: &str) -> CommandResult<ImapAccountConfig> {
        let account = self.service.account_record(account_id)?;
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
        })
    }

    async fn sync_interval(&self, account_id: &str) -> CommandResult<SyncInterval> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .read()
            .get_sync_interval(&account.data_slot_id)
            .await
    }

    pub(crate) async fn repository(&self) -> CommandResult<&Arc<MailRepository>> {
        self.repository
            .get_or_try_init(|| async {
                let data_dir = self.service.configured_data_dir()?;
                self.repository_provider.open(&data_dir).await.map(Arc::new)
            })
            .await
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
        let notification_baseline_ready = repository
            .read()
            .notification_baseline_ready(&account.data_slot_id)
            .await?;
        tracing::info!(%account_id, report_progress, "sync started");
        self.update_runtime_state(account_id, AccountRuntimeState::Syncing, None, None);
        if report_progress {
            self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None, None);
        }
        let observer = RuntimeObserver {
            runtime: self,
            account_id: account_id.to_owned(),
            generation,
            report_progress,
            candidates: Mutex::new(Vec::new()),
        };
        let sync_result = match self.imap_config(&account.id).await {
            Ok(config) => {
                let sink = repository.sync_sink();
                self.provider.synchronize(&config, &sink, &observer).await
            }
            Err(error) => Err(error),
        };
        let result = match sync_result {
            Ok(()) => {
                repository
                    .sync_sink()
                    .complete_notification_baseline(&account.data_slot_id)
                    .await
            }
            Err(error) => Err(error),
        };
        if !self.is_current_supervisor(account_id, generation) {
            return Err(CommandError::new("account.runtime_stopped"));
        }
        match result {
            Ok(()) => {
                if notification_baseline_ready {
                    self.emit_new_mail_candidates(account_id, observer.take_candidates());
                }
                if report_progress {
                    self.update_progress(account_id, SyncPhase::Complete, 1, 1, None, None);
                }
                self.update_runtime_state(account_id, AccountRuntimeState::Ready, None, None);
                tracing::info!(%account_id, "sync completed");
                Ok(())
            }
            Err(error) => {
                tracing::error!(%account_id, code = %error.code, retryable = error.retryable, "sync failed");
                if report_progress {
                    self.update_progress(
                        account_id,
                        SyncPhase::Failed,
                        0,
                        0,
                        None,
                        Some(error.code.clone()),
                    );
                }
                if let Err(event_error) = self.app.emit(
                    "sync-failed",
                    SyncFailedEvent {
                        account_id: account_id.to_owned(),
                        code: error.code.clone(),
                        retryable: error.retryable,
                    },
                ) {
                    tracing::warn!(%account_id, ?event_error, "sync failed event emission failed");
                }
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
                        AccountRuntimeState::Offline,
                        Some(error.code.clone()),
                        None,
                    );
                }
                Err(error)
            }
        }
    }

    fn emit_new_mail_candidates(&self, account_id: &str, candidates: Vec<PendingNewMailCandidate>) {
        let Ok(preferences) = self.service.get_notification_preferences() else {
            return;
        };
        let candidates = limit_notification_batch(
            &preferences.display_mode,
            preferences.max_stacked,
            eligible_new_mail_candidates(&preferences, account_id, candidates),
        );
        for candidate in &candidates {
            if let Err(error) = self.app.emit("new-mail-candidate", candidate) {
                tracing::warn!(
                    %account_id,
                    message_id = %candidate.message_id,
                    ?error,
                    "new mail candidate event failed"
                );
            }
        }
        self.notifications.present_batch(candidates, &preferences);
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
        if let Err(error) = self.app.emit("account-runtime-status-changed", summary) {
            tracing::warn!(%account_id, ?error, "account runtime event failed");
        }
    }

    fn update_progress(
        &self,
        account_id: &str,
        phase: SyncPhase,
        completed: u64,
        total: u64,
        current_mailbox_name: Option<String>,
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
                current_mailbox_name,
                error_code,
                revision,
            };
            values.insert(account_id.to_owned(), progress.clone());
            progress
        } else {
            return;
        };
        if let Err(error) = self.app.emit("sync-progress", progress) {
            tracing::warn!(%account_id, ?error, "sync progress event failed");
        }
    }
}

fn limit_notification_batch(
    display_mode: &NotificationDisplayMode,
    max_stacked: u8,
    candidates: Vec<NewMailCandidate>,
) -> Vec<NewMailCandidate> {
    let limit = if *display_mode == NotificationDisplayMode::Replace {
        1
    } else {
        usize::from(max_stacked).max(1)
    };
    let skip = candidates.len().saturating_sub(limit);
    candidates.into_iter().skip(skip).collect()
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

    #[test]
    fn configured_sync_intervals_have_only_the_requested_schedules() {
        assert_eq!(SyncInterval::Manual.minutes(), None);
        assert_eq!(SyncInterval::Minutes1.minutes(), Some(1));
        assert_eq!(SyncInterval::Minutes5.minutes(), Some(5));
        assert_eq!(SyncInterval::Minutes10.minutes(), Some(10));
    }

    #[test]
    fn notification_candidates_respect_hierarchy_and_deduplicate() {
        let mut preferences = crate::core::NotificationPreferences::default();
        preferences
            .folders
            .push(crate::core::NotificationFolderSetting {
                account_id: "account".to_owned(),
                mailbox_id: "archive".to_owned(),
                enabled: true,
            });
        let candidate =
            |mailbox_id: &str, message_id: &str, default_enabled: bool| PendingNewMailCandidate {
                candidate: NewMailCandidate {
                    account_id: "account".to_owned(),
                    mailbox_id: mailbox_id.to_owned(),
                    message_id: message_id.to_owned(),
                    sender_name: Some("Sender".to_owned()),
                    sender_email: "sender@example.com".to_owned(),
                    subject: "Subject".to_owned(),
                },
                default_enabled,
            };
        let eligible = eligible_new_mail_candidates(
            &preferences,
            "account",
            vec![
                candidate("inbox", "one", true),
                candidate("inbox", "one", true),
                candidate("archive", "two", false),
                candidate("sent", "three", false),
            ],
        );
        assert_eq!(eligible.len(), 2);
        assert_eq!(eligible[0].message_id, "one");
        assert_eq!(eligible[1].message_id, "two");

        preferences
            .accounts
            .push(crate::core::NotificationAccountSetting {
                account_id: "account".to_owned(),
                enabled: false,
            });
        assert!(eligible_new_mail_candidates(
            &preferences,
            "account",
            vec![candidate("inbox", "four", true)],
        )
        .is_empty());
    }

    #[test]
    fn notification_batches_keep_only_the_newest_visible_candidates() {
        let candidate = |id: &str| NewMailCandidate {
            account_id: "account".to_owned(),
            mailbox_id: "inbox".to_owned(),
            message_id: id.to_owned(),
            sender_name: None,
            sender_email: "sender@example.com".to_owned(),
            subject: id.to_owned(),
        };
        let candidates = vec![
            candidate("one"),
            candidate("two"),
            candidate("three"),
            candidate("four"),
        ];
        let stacked =
            limit_notification_batch(&NotificationDisplayMode::Stacked, 2, candidates.clone());
        assert_eq!(
            stacked
                .iter()
                .map(|candidate| candidate.message_id.as_str())
                .collect::<Vec<_>>(),
            ["three", "four"]
        );
        let replace = limit_notification_batch(&NotificationDisplayMode::Replace, 10, candidates);
        assert_eq!(replace[0].message_id, "four");
    }
}

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use nextmail_core::{
    AccountManagementDetail, AttachmentSummary, CommandError, CommandResult, ImapAccountConfig,
    ImapSyncProvider, InboxWatchOutcome, MailSyncSink, MailboxRole, MailboxSummary, MessageDetail,
    MessageListPage, PendingOperationKind, PendingOperationSummary, RemoteOperation,
    RemoteOperationKind, SyncNotice, SyncObserver, SyncPhase, SyncPolicy, SyncProgress,
};
use nextmail_protocols::AsyncImapProvider;
use nextmail_storage::MailRepository;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Notify, OnceCell};

use crate::application::AppService;

pub struct MailRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    repository: OnceCell<Arc<MailRepository>>,
    progress: RwLock<HashMap<String, SyncProgress>>,
    provider: Arc<dyn ImapSyncProvider>,
    sync_lock: Mutex<()>,
    wake_supervisor: Notify,
    manual_sync_requested: AtomicBool,
    background_sync_requested: AtomicBool,
    started: AtomicBool,
}

impl MailRuntime {
    pub fn new(app: AppHandle, service: Arc<AppService>) -> Self {
        Self {
            app,
            service,
            repository: OnceCell::new(),
            progress: RwLock::new(HashMap::new()),
            provider: Arc::new(AsyncImapProvider),
            sync_lock: Mutex::new(()),
            wake_supervisor: Notify::new(),
            manual_sync_requested: AtomicBool::new(false),
            background_sync_requested: AtomicBool::new(false),
            started: AtomicBool::new(false),
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            self.wake_supervisor.notify_one();
            return;
        }
        if let Some(account) = self
            .service
            .list_account_summaries()
            .ok()
            .and_then(|accounts| accounts.into_iter().next())
        {
            self.update_progress(&account.id, SyncPhase::Connecting, 0, 0, None);
        }
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            runtime.supervisor_loop().await;
        });
    }

    pub fn wake(&self) {
        self.wake_supervisor.notify_one();
    }

    async fn supervisor_loop(self: &Arc<Self>) {
        let mut retry_delay = Duration::from_secs(2);
        let mut recovered = false;
        let mut startup_sync_complete = false;
        loop {
            let Some(account) = self
                .service
                .list_account_summaries()
                .ok()
                .and_then(|accounts| accounts.into_iter().next())
            else {
                tokio::select! {
                    _ = self.wake_supervisor.notified() => {},
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                }
                continue;
            };
            let Ok(repository) = self.repository().await else {
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                continue;
            };
            if !recovered {
                let _ = repository.recover_pending_operations().await;
                recovered = true;
            }

            if !startup_sync_complete {
                let sync_result = self.run_sync(&account.id, true).await;
                if sync_result.is_ok() {
                    startup_sync_complete = true;
                    self.manual_sync_requested.store(false, Ordering::Release);
                    self.background_sync_requested
                        .store(false, Ordering::Release);
                    retry_delay = Duration::from_secs(2);
                    let _ = self.drain_pending_operations(&account.id).await;
                } else {
                    tokio::select! {
                        _ = self.wake_supervisor.notified() => {},
                        _ = tokio::time::sleep(retry_delay) => {},
                    }
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                    continue;
                }
            }

            let Ok(config) = self.imap_config(&account.id).await else {
                tokio::time::sleep(retry_delay).await;
                continue;
            };
            let watch = self
                .provider
                .wait_for_inbox_change(&config, Duration::from_secs(25 * 60));
            let wake = tokio::select! {
                _ = self.wake_supervisor.notified() => SupervisorWake::Requested,
                _ = tokio::time::sleep(Duration::from_secs(5 * 60)) => SupervisorWake::BackgroundSync,
                result = watch => {
                    match result {
                        Ok(InboxWatchOutcome::Unsupported) => {
                            tokio::select! {
                                _ = self.wake_supervisor.notified() => SupervisorWake::Requested,
                                _ = tokio::time::sleep(Duration::from_secs(60)) => SupervisorWake::BackgroundSync,
                            }
                        }
                        Ok(InboxWatchOutcome::Changed | InboxWatchOutcome::Timeout) => SupervisorWake::BackgroundSync,
                        Err(_) => {
                            tokio::select! {
                                _ = self.wake_supervisor.notified() => SupervisorWake::Requested,
                                _ = tokio::time::sleep(retry_delay) => SupervisorWake::Retry,
                            }
                        }
                    }
                }
            };

            if wake == SupervisorWake::Retry {
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                continue;
            }

            let manual = self.manual_sync_requested.swap(false, Ordering::AcqRel);
            let background_requested = self.background_sync_requested.swap(false, Ordering::AcqRel);
            if manual || background_requested || wake == SupervisorWake::BackgroundSync {
                let sync_result = self.run_sync(&account.id, manual).await;
                if sync_result.is_ok() {
                    retry_delay = Duration::from_secs(2);
                } else {
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                }
            }

            if self
                .drain_pending_operations(&account.id)
                .await
                .unwrap_or(false)
            {
                retry_delay = Duration::from_secs(2);
            }
        }
    }

    pub async fn list_mailboxes(&self, account_id: &str) -> CommandResult<Vec<MailboxSummary>> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
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
            .list_messages(&account.data_slot_id, mailbox_id, cursor, limit)
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
        let account = self.service.account_record(account_id)?;
        let updated = self
            .repository()
            .await?
            .set_sync_policy(&account.data_slot_id, sync_policy)
            .await?;
        self.background_sync_requested
            .store(true, Ordering::Release);
        self.wake_supervisor.notify_one();
        Ok(updated)
    }

    pub fn sync_now(&self, account_id: &str) -> CommandResult<()> {
        self.service.account_record(account_id)?;
        self.manual_sync_requested.store(true, Ordering::Release);
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        self.wake_supervisor.notify_one();
        Ok(())
    }

    pub async fn set_message_read(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        read: bool,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .queue_set_read(&account.data_slot_id, mailbox_id, message_ids, read)
            .await?;
        self.emit_local_change(account_id, mailbox_id, message_ids);
        self.wake_supervisor.notify_one();
        Ok(())
    }

    pub async fn set_message_flagged(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_ids: &[String],
        flagged: bool,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .queue_set_flagged(&account.data_slot_id, mailbox_id, message_ids, flagged)
            .await?;
        self.emit_local_change(account_id, mailbox_id, message_ids);
        self.wake_supervisor.notify_one();
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
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .queue_transfer(
                &account.data_slot_id,
                source_mailbox_id,
                destination_mailbox_id,
                message_ids,
                copy,
            )
            .await?;
        self.emit_local_change(account_id, source_mailbox_id, message_ids);
        self.wake_supervisor.notify_one();
        Ok(())
    }

    pub async fn delete_messages(
        &self,
        account_id: &str,
        source_mailbox_id: &str,
        message_ids: &[String],
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let role = repository
            .mailbox_role_for_id(&account.data_slot_id, source_mailbox_id)
            .await?;
        if role == MailboxRole::Trash {
            repository
                .queue_permanent_delete(&account.data_slot_id, source_mailbox_id, message_ids)
                .await?;
        } else {
            let (trash_id, _) = repository
                .mailbox_for_role(&account.data_slot_id, MailboxRole::Trash)
                .await?
                .ok_or_else(|| CommandError::new("mailbox.trash_not_mapped"))?;
            repository
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
        self.wake_supervisor.notify_one();
        Ok(())
    }

    pub async fn archive_messages(
        &self,
        account_id: &str,
        source_mailbox_id: &str,
        message_ids: &[String],
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let (archive_id, _) = repository
            .mailbox_for_role(&account.data_slot_id, MailboxRole::Archive)
            .await?
            .ok_or_else(|| CommandError::new("mailbox.archive_not_mapped"))?;
        repository
            .queue_transfer(
                &account.data_slot_id,
                source_mailbox_id,
                &archive_id,
                message_ids,
                false,
            )
            .await?;
        self.emit_local_change(account_id, source_mailbox_id, message_ids);
        self.wake_supervisor.notify_one();
        Ok(())
    }

    pub async fn set_mailbox_role_mapping(
        &self,
        account_id: &str,
        role: MailboxRole,
        mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
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
            .list_pending_operation_status(account_id, &account.data_slot_id)
            .await
    }

    pub async fn retry_pending_operation(
        &self,
        account_id: &str,
        operation_id: &str,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .retry_pending_operation(&account.data_slot_id, operation_id)
            .await?;
        self.wake_supervisor.notify_one();
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
            .raw_message(&account.data_slot_id, message_id)
            .await?
        {
            Some(raw) => raw,
            None => {
                self.fetch_and_store_message(&account.id, message_id)
                    .await?;
                repository
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
        let (message_id, part_index) = repository
            .attachment_context(&account.data_slot_id, attachment_id)
            .await?;
        let raw = match repository
            .raw_message(&account.data_slot_id, &message_id)
            .await?
        {
            Some(raw) => raw,
            None => {
                self.fetch_and_store_message(&account.id, &message_id)
                    .await?;
                repository
                    .raw_message(&account.data_slot_id, &message_id)
                    .await?
                    .ok_or_else(|| CommandError::new("message.raw_unavailable"))?
            }
        };
        let content = nextmail_protocols::extract_attachment(&raw, part_index)?;
        repository
            .store_attachment_content(attachment_id, &content)
            .await
    }

    async fn fetch_and_store_message(
        &self,
        account_id: &str,
        message_id: &str,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        let context = repository
            .remote_message_context(&account.data_slot_id, message_id)
            .await?;
        let config = self.imap_config(&account.id).await?;
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
            .upsert_message(&account.data_slot_id, &context.mailbox_id, &message)
            .await?;
        let revision = repository
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

    async fn repository(&self) -> CommandResult<&Arc<MailRepository>> {
        self.repository
            .get_or_try_init(|| async {
                let data_dir = self.service.configured_data_dir()?;
                MailRepository::open(&data_dir).await.map(Arc::new)
            })
            .await
    }

    async fn drain_pending_operations(&self, account_id: &str) -> CommandResult<bool> {
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        let config = self.imap_config(account_id).await?;
        let mut processed = false;
        while let Some(work) = repository
            .claim_pending_operation(&account.data_slot_id)
            .await?
        {
            processed = true;
            let result = if work.kind == PendingOperationKind::AppendSent {
                let destination = work
                    .destination_mailbox_name
                    .as_deref()
                    .ok_or_else(|| CommandError::new("operation.destination_required"));
                let hash = work
                    .payload
                    .get("mimeHash")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| CommandError::new("operation.mime_missing"));
                match (destination, hash) {
                    (Ok(destination), Ok(hash)) => match repository.read_send_mime(hash).await {
                        Ok(raw) => self
                            .provider
                            .append_message(&config, destination, "(\\Seen)", &raw)
                            .await
                            .map(|_| Default::default()),
                        Err(error) => Err(error),
                    },
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            } else if work.kind == PendingOperationKind::AppendDraft {
                let destination = work
                    .destination_mailbox_name
                    .as_deref()
                    .ok_or_else(|| CommandError::new("operation.destination_required"));
                let hash = work
                    .payload
                    .get("mimeHash")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| CommandError::new("operation.mime_missing"));
                let draft_id = work
                    .payload
                    .get("draftId")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| CommandError::new("operation.draft_missing"));
                match (destination, hash, draft_id) {
                    (Ok(destination), Ok(hash), Ok(draft_id)) => {
                        match repository.read_send_mime(hash).await {
                            Ok(raw) => {
                                self.provider
                                    .replace_draft(&config, destination, draft_id, &raw)
                                    .await
                            }
                            Err(error) => Err(error),
                        }
                    }
                    (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => Err(error),
                }
            } else {
                self.provider
                    .apply_operation(&config, &remote_operation(&work)?)
                    .await
            };
            match result {
                Ok(outcome) => {
                    repository
                        .complete_pending_operation(&work, outcome.cleanup_pending)
                        .await?;
                }
                Err(error) => {
                    repository
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

    async fn run_sync(&self, account_id: &str, report_progress: bool) -> CommandResult<()> {
        let _sync_guard = self.sync_lock.lock().await;
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        if report_progress {
            self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        }
        let observer = RuntimeObserver {
            runtime: self,
            account_id: account_id.to_owned(),
            report_progress,
        };
        let result = match self.imap_config(&account.id).await {
            Ok(config) => {
                self.provider
                    .synchronize(&config, repository.as_ref(), &observer)
                    .await
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                if report_progress {
                    self.update_progress(account_id, SyncPhase::Complete, 1, 1, None);
                }
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
                Err(error)
            }
        }
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
    report_progress: bool,
}

impl SyncObserver for RuntimeObserver<'_> {
    fn notify(&self, notice: SyncNotice) {
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

fn remote_operation(
    work: &nextmail_storage::PendingOperationWork,
) -> CommandResult<RemoteOperation> {
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

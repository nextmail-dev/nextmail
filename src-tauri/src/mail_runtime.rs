use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use nextmail_core::{
    AccountManagementDetail, AttachmentSummary, CommandError, CommandResult, ImapAccountConfig,
    ImapSyncProvider, MailSyncSink, MailboxSummary, MessageDetail, MessageListPage, SyncNotice,
    SyncObserver, SyncPhase, SyncPolicy, SyncProgress,
};
use nextmail_protocols::AsyncImapProvider;
use nextmail_storage::MailRepository;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, OnceCell};

use crate::application::AppService;

pub struct MailRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    repository: OnceCell<Arc<MailRepository>>,
    progress: RwLock<HashMap<String, SyncProgress>>,
    provider: Arc<dyn ImapSyncProvider>,
    sync_lock: Mutex<()>,
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
        }
    }

    pub fn start(self: &Arc<Self>) {
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let Some(account) = runtime
                .service
                .list_account_summaries()
                .ok()
                .and_then(|accounts| accounts.into_iter().next())
            else {
                return;
            };
            let mut delay = std::time::Duration::from_secs(2);
            for attempt in 0..3 {
                match runtime.run_initial_sync(&account.id).await {
                    Ok(()) => break,
                    Err(error) if error.retryable && attempt < 2 => {
                        tokio::time::sleep(delay).await;
                        delay = delay.saturating_mul(2);
                    }
                    Err(_) => break,
                }
            }
        });
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
    ) -> CommandResult<MessageDetail> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .get_message_detail(&account.data_slot_id, message_id)
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
        let runtime = Arc::clone(self);
        let account_id = account_id.to_owned();
        tauri::async_runtime::spawn(async move {
            let _ = runtime.run_initial_sync(&account_id).await;
        });
        Ok(updated)
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
    ) -> CommandResult<MessageDetail> {
        self.fetch_and_store_message(account_id, message_id).await?;
        self.get_message_detail(account_id, message_id).await
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
            .get_message_detail(&account.data_slot_id, message_id)
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

    async fn run_initial_sync(&self, account_id: &str) -> CommandResult<()> {
        let _sync_guard = self.sync_lock.lock().await;
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        self.update_progress(account_id, SyncPhase::Connecting, 0, 0, None);
        let observer = RuntimeObserver {
            runtime: self,
            account_id: account_id.to_owned(),
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
                self.update_progress(account_id, SyncPhase::Complete, 1, 1, None);
                Ok(())
            }
            Err(error) => {
                self.update_progress(
                    account_id,
                    SyncPhase::Failed,
                    0,
                    0,
                    Some(error.code.clone()),
                );
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
}

impl SyncObserver for RuntimeObserver<'_> {
    fn notify(&self, notice: SyncNotice) {
        match notice {
            SyncNotice::Folders { completed, total } => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Folders,
                completed,
                total,
                None,
            ),
            SyncNotice::Summaries { completed, total } => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Summaries,
                completed,
                total,
                None,
            ),
            SyncNotice::Bodies { completed, total } => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Bodies,
                completed,
                total,
                None,
            ),
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

use std::{path::Path, sync::Arc, time::Duration};

use lettre::{address::Envelope, Address};
use nextmail_core::{
    AccountSummary, CommandError, CommandResult, ComposerBootstrap, DraftAttachmentSummary,
    DraftContent, DraftDetail, DraftListItem, DraftRecipientFields, DraftStatus,
    LanguagePreference, MessageAddress, SendJobSummary,
};
use nextmail_protocols::{build_outgoing_message, OutgoingAttachment};
use nextmail_storage::{MailRepository, SaveDraftRequest};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{Notify, OnceCell};

use crate::{adapters::send_raw_smtp, application::AppService};

const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_TOTAL_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;

pub struct ComposerRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    repository: OnceCell<Arc<MailRepository>>,
    wake_worker: Notify,
}

impl ComposerRuntime {
    pub fn new(app: AppHandle, service: Arc<AppService>) -> Self {
        Self {
            app,
            service,
            repository: OnceCell::new(),
            wake_worker: Notify::new(),
        }
    }

    pub fn start(self: &Arc<Self>) {
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let Ok(repository) = runtime.repository().await else {
                return;
            };
            let _ = repository.recover_interrupted_send_jobs().await;
            loop {
                while let Ok(Some(job)) = repository.claim_next_send_job().await {
                    runtime.emit_job(&job.id, &job.account_slot_id).await;
                    let result = runtime
                        .deliver(
                            &job.account_slot_id,
                            &job.mime_hash,
                            &job.envelope_recipients,
                        )
                        .await;
                    match result {
                        Ok(()) => {
                            let _ = repository.complete_send_job(&job.id).await;
                        }
                        Err(error) if error.retryable && job.attempt_count < 3 => {
                            let delay = 5_i64.saturating_mul(1_i64 << (job.attempt_count - 1));
                            let _ = repository
                                .defer_send_job(
                                    &job.id,
                                    &error.code,
                                    unix_timestamp().saturating_add(delay),
                                )
                                .await;
                        }
                        Err(error) => {
                            let _ = repository.fail_send_job(&job.id, &error.code).await;
                        }
                    }
                    runtime.emit_job(&job.id, &job.account_slot_id).await;
                }
                tokio::select! {
                    _ = runtime.wake_worker.notified() => {},
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                }
            }
        });
    }

    pub async fn open_composer(&self, account_id: &str) -> CommandResult<String> {
        let account = self.service.account_record(account_id)?;
        let draft = self
            .repository()
            .await?
            .create_draft(account_id, &account.data_slot_id)
            .await?;
        if let Err(error) = self.show_composer_window(&account.id, &draft.id).await {
            self.repository()
                .await?
                .discard_empty_draft(&account.data_slot_id, &draft.id)
                .await;
            return Err(error);
        }
        Ok(draft.id)
    }

    pub async fn list_drafts(&self, account_id: &str) -> CommandResult<Vec<DraftListItem>> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .list_editing_drafts(account_id, &account.data_slot_id)
            .await
    }

    pub async fn open_existing_composer(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let draft = self
            .repository()
            .await?
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        if draft.status != DraftStatus::Editing {
            return Err(CommandError::new("draft.not_editable"));
        }
        self.show_composer_window(account_id, draft_id).await
    }

    async fn show_composer_window(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        let label = format!("composer-{draft_id}");
        if let Some(window) = self.app.get_webview_window(&label) {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|_| CommandError::new("composer.window_create_failed"))?;
            return Ok(());
        }
        let url = format!(
            "index.html?window=composer&accountId={}&draftId={}",
            account_id, draft_id
        );
        let title = match self.service.get_preferences()?.language {
            LanguagePreference::ZhCn => "新建邮件 — NextMail",
            LanguagePreference::EnUs => "New message — NextMail",
        };
        WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(860.0, 700.0)
            .min_inner_size(680.0, 560.0)
            .build()
            .map_err(|_| CommandError::new("composer.window_create_failed"))?;
        Ok(())
    }

    pub async fn get_bootstrap(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<ComposerBootstrap> {
        let account = self.service.account_record(account_id)?;
        let draft = self
            .repository()
            .await?
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        Ok(ComposerBootstrap {
            draft,
            sender: AccountSummary::from(&account),
        })
    }

    pub async fn save_draft(
        &self,
        account_id: &str,
        draft_id: &str,
        recipients: DraftRecipientFields,
        subject: String,
        content: DraftContent,
        expected_revision: u64,
    ) -> CommandResult<DraftDetail> {
        validate_content(&content)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .save_draft(SaveDraftRequest {
                account_id,
                account_slot_id: &account.data_slot_id,
                draft_id,
                recipients: &recipients,
                subject: subject.trim(),
                content: &content,
                expected_revision,
            })
            .await
    }

    pub async fn add_attachments(
        &self,
        account_id: &str,
        draft_id: &str,
        selected_paths: Vec<String>,
    ) -> CommandResult<Vec<DraftAttachmentSummary>> {
        if selected_paths.is_empty() {
            return Ok(Vec::new());
        }
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let draft = repository
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        let mut total = draft.attachments.iter().map(|item| item.size).sum::<u64>();
        let mut added = Vec::new();
        for selected in selected_paths {
            let path = Path::new(&selected);
            let metadata = tokio::fs::metadata(path)
                .await
                .map_err(|_| CommandError::new("attachment.read_failed"))?;
            if !metadata.is_file() {
                return Err(CommandError::new("attachment.file_required"));
            }
            if metadata.len() > MAX_ATTACHMENT_BYTES {
                return Err(CommandError::new("attachment.too_large"));
            }
            total = total.saturating_add(metadata.len());
            if total > MAX_TOTAL_ATTACHMENT_BYTES {
                return Err(CommandError::new("attachment.total_too_large"));
            }
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CommandError::new("attachment.name_invalid"))?;
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|_| CommandError::new("attachment.read_failed"))?;
            let content_type = mime_guess::from_path(path)
                .first_or_octet_stream()
                .essence_str()
                .to_owned();
            added.push(
                repository
                    .add_draft_attachment(draft_id, file_name, &content_type, &bytes)
                    .await?,
            );
        }
        Ok(added)
    }

    pub async fn remove_attachment(
        &self,
        account_id: &str,
        draft_id: &str,
        attachment_id: &str,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        self.repository()
            .await?
            .remove_draft_attachment(draft_id, attachment_id)
            .await
    }

    pub async fn discard_empty_draft(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        repository
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        repository
            .discard_empty_draft(&account.data_slot_id, draft_id)
            .await;
        Ok(())
    }

    pub async fn queue_send(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<SendJobSummary> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let draft = repository
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        validate_recipient_fields(&draft.recipients, true)?;
        validate_content(&draft.content)?;
        let mut attachments = Vec::new();
        for stored in repository.draft_attachments(draft_id).await? {
            attachments.push(OutgoingAttachment {
                file_name: stored.summary.file_name,
                content_type: stored.summary.content_type,
                bytes: repository.attachment_bytes(&stored.content_hash).await?,
            });
        }
        let sender = MessageAddress {
            name: nonempty(&account.display_name),
            email: account.email.clone(),
        };
        let raw = build_outgoing_message(
            &sender,
            &draft.recipients,
            &draft.subject,
            &draft.content,
            attachments,
        )?;
        let hash = repository.write_send_mime(&raw).await?;
        let envelope = envelope_recipients(&draft.recipients);
        let job = repository
            .queue_send_job(
                account_id,
                &account.data_slot_id,
                draft_id,
                &hash,
                &envelope,
            )
            .await?;
        self.wake_worker.notify_one();
        Ok(job)
    }

    pub async fn retry_send(
        &self,
        account_id: &str,
        job_id: &str,
    ) -> CommandResult<SendJobSummary> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        repository
            .get_send_job(account_id, &account.data_slot_id, job_id)
            .await?;
        repository.retry_send_job(job_id).await?;
        self.wake_worker.notify_one();
        repository
            .get_send_job(account_id, &account.data_slot_id, job_id)
            .await
    }

    pub async fn get_send_job(
        &self,
        account_id: &str,
        job_id: &str,
    ) -> CommandResult<SendJobSummary> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .get_send_job(account_id, &account.data_slot_id, job_id)
            .await
    }

    async fn deliver(
        &self,
        account_slot_id: &str,
        mime_hash: &str,
        recipients: &[String],
    ) -> CommandResult<()> {
        let account = self.service.account_record_for_slot(account_slot_id)?;
        let password = self
            .service
            .account_password(&account.credential_ref)
            .await?;
        let from: Address = account
            .email
            .parse()
            .map_err(|_| CommandError::new("send.sender_invalid"))?;
        let to = recipients
            .iter()
            .map(|value| {
                value
                    .parse::<Address>()
                    .map_err(|_| CommandError::new("send.recipient_invalid"))
            })
            .collect::<CommandResult<Vec<_>>>()?;
        let envelope = Envelope::new(Some(from), to)
            .map_err(|_| CommandError::new("send.envelope_invalid"))?;
        let raw = self.repository().await?.read_send_mime(mime_hash).await?;
        send_raw_smtp(&account.outgoing, &password, &envelope, &raw).await
    }

    async fn emit_job(&self, job_id: &str, account_slot_id: &str) {
        let Ok(account) = self.service.account_record_for_slot(account_slot_id) else {
            return;
        };
        let Ok(repository) = self.repository().await else {
            return;
        };
        let Ok(job) = repository
            .get_send_job(&account.id, account_slot_id, job_id)
            .await
        else {
            return;
        };
        let subject = repository
            .get_draft(&account.id, account_slot_id, &job.draft_id)
            .await
            .map(|draft| draft.subject)
            .unwrap_or_default();
        let _ = self.app.emit(
            "send-job-changed",
            SendJobChangedEvent {
                account_id: job.account_id,
                draft_id: job.draft_id,
                job_id: job.id,
                status: job.status,
                subject,
                revision: job.revision,
            },
        );
    }

    async fn repository(&self) -> CommandResult<&Arc<MailRepository>> {
        self.repository
            .get_or_try_init(|| async {
                MailRepository::open(&self.service.configured_data_dir()?)
                    .await
                    .map(Arc::new)
            })
            .await
    }
}

fn validate_recipient_fields(fields: &DraftRecipientFields, required: bool) -> CommandResult<()> {
    let all = fields
        .to
        .iter()
        .chain(&fields.cc)
        .chain(&fields.bcc)
        .collect::<Vec<_>>();
    if required && all.is_empty() {
        return Err(CommandError::new("send.recipient_required"));
    }
    for address in all {
        address
            .email
            .parse::<Address>()
            .map_err(|_| CommandError::new("send.recipient_invalid"))?;
    }
    Ok(())
}

fn validate_content(content: &DraftContent) -> CommandResult<()> {
    if content.editor_json.len() > 5_000_000
        || content.html.len() > 5_000_000
        || content.plain_text.len() > 5_000_000
    {
        return Err(CommandError::new("draft.content_too_large"));
    }
    serde_json::from_str::<serde_json::Value>(&content.editor_json)
        .map_err(|_| CommandError::new("draft.editor_json_invalid"))?;
    Ok(())
}

fn envelope_recipients(fields: &DraftRecipientFields) -> Vec<String> {
    fields
        .to
        .iter()
        .chain(&fields.cc)
        .chain(&fields.bcc)
        .map(|value| value.email.clone())
        .collect()
}

fn nonempty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendJobChangedEvent {
    account_id: String,
    draft_id: String,
    job_id: String,
    status: nextmail_core::SendJobStatus,
    subject: String,
    revision: u64,
}

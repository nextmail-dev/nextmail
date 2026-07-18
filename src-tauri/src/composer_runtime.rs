use std::{
    collections::HashSet,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::core::{
    AccountSummary, CommandError, CommandResult, ComposerBootstrap, DraftAttachmentSummary,
    DraftContent, DraftDetail, DraftListItem, DraftRecipientFields, DraftStatus,
    LanguagePreference, MailboxRole, MessageAddress, MessageComposeAction, SendJobSummary,
};
use crate::protocols::{build_outgoing_message, OutgoingAttachment};
use crate::storage::{
    ClaimedSendJob, CreateMessageActionDraftRequest, MailRepository, SaveDraftRequest,
};
use lettre::{address::Envelope, Address};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{Notify, OnceCell};

use crate::{adapters::send_raw_smtp, application::AppService, mail_runtime::MailRuntime};

const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_TOTAL_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;

pub struct ComposerRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    repository: OnceCell<Arc<MailRepository>>,
    wake_worker: Notify,
    mail: Arc<MailRuntime>,
    started: AtomicBool,
}

impl ComposerRuntime {
    pub fn new(app: AppHandle, service: Arc<AppService>, mail: Arc<MailRuntime>) -> Self {
        Self {
            app,
            service,
            repository: OnceCell::new(),
            wake_worker: Notify::new(),
            mail,
            started: AtomicBool::new(false),
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            self.wake_worker.notify_one();
            return;
        }
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let Ok(repository) = runtime.repository().await else {
                runtime.started.store(false, Ordering::Release);
                return;
            };
            let _ = repository.recover_interrupted_send_jobs().await;
            let mut active_accounts = HashSet::new();
            let mut workers = tokio::task::JoinSet::new();
            let mut fair_cursor = 0_usize;
            loop {
                while active_accounts.len() < 2 {
                    let Ok(slots) = repository.ready_send_account_slots().await else {
                        break;
                    };
                    if slots.is_empty() {
                        break;
                    }
                    let Some(slot) =
                        select_ready_account(&slots, &active_accounts, &mut fair_cursor)
                    else {
                        break;
                    };
                    let Ok(Some(job)) = repository.claim_next_send_job_for_account(&slot).await
                    else {
                        break;
                    };
                    active_accounts.insert(slot.clone());
                    let worker = Arc::clone(&runtime);
                    workers.spawn(async move {
                        worker.process_send_job(job).await;
                        slot
                    });
                }
                tokio::select! {
                    completed = workers.join_next(), if !workers.is_empty() => {
                        if let Some(Ok(slot)) = completed {
                            active_accounts.remove(&slot);
                        }
                    }
                    _ = runtime.wake_worker.notified() => {},
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                }
            }
        });
    }

    async fn process_send_job(self: &Arc<Self>, job: ClaimedSendJob) {
        let Ok(repository) = self.repository().await else {
            return;
        };
        self.emit_job(&job.id, &job.account_slot_id).await;
        let result = self
            .deliver(
                &job.account_slot_id,
                &job.mime_hash,
                &job.envelope_recipients,
            )
            .await;
        match result {
            Ok(()) => {
                let sent_mailbox = repository
                    .mailbox_for_role(&job.account_slot_id, MailboxRole::Sent)
                    .await
                    .ok()
                    .flatten()
                    .map(|(id, _)| id);
                let _ = repository
                    .complete_send_job_and_queue_sent(&job.id, sent_mailbox.as_deref())
                    .await;
                self.mail.wake_account_by_slot(&job.account_slot_id);
            }
            Err(error) if error.retryable && job.attempt_count < 3 => {
                self.mail
                    .report_account_error_by_slot(&job.account_slot_id, &error);
                let delay = 5_i64.saturating_mul(1_i64 << (job.attempt_count - 1));
                let _ = repository
                    .defer_send_job(&job.id, &error.code, unix_timestamp().saturating_add(delay))
                    .await;
            }
            Err(error) => {
                self.mail
                    .report_account_error_by_slot(&job.account_slot_id, &error);
                let _ = repository.fail_send_job(&job.id, &error.code).await;
            }
        }
        self.emit_job(&job.id, &job.account_slot_id).await;
    }

    pub async fn open_composer(&self, account_id: &str) -> CommandResult<String> {
        self.mail.ensure_account_writable(account_id)?;
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

    pub async fn close_account_windows(&self, account_id: &str) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let drafts = self
            .repository()
            .await?
            .list_editing_drafts(account_id, &account.data_slot_id)
            .await?;
        let labels = drafts
            .iter()
            .map(|draft| format!("composer-{}", draft.id))
            .collect::<Vec<_>>();
        for label in &labels {
            if let Some(window) = self.app.get_webview_window(label) {
                let _ = window.close();
            }
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while labels
            .iter()
            .any(|label| self.app.get_webview_window(label).is_some())
        {
            if tokio::time::Instant::now() >= deadline {
                return Err(CommandError::new("account.composer_close_timeout"));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }

    pub async fn open_existing_composer(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
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

    pub async fn open_remote_draft(&self, account_id: &str, message_id: &str) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let mut detail = self
            .mail
            .get_message_detail(account_id, message_id, None)
            .await?;
        if detail.body_availability != crate::core::ContentAvailability::Available {
            detail = self
                .mail
                .request_message_body(account_id, message_id, None)
                .await?;
        }
        for attachment in detail.attachments {
            if attachment.availability != crate::core::ContentAvailability::Available {
                self.mail
                    .request_attachment(account_id, &attachment.id)
                    .await?;
            }
        }
        let draft = self
            .repository()
            .await?
            .import_message_as_draft(account_id, &account.data_slot_id, message_id)
            .await?;
        self.show_composer_window(account_id, &draft.id).await
    }

    pub async fn open_message_action_composer(
        &self,
        account_id: &str,
        message_id: &str,
        action: MessageComposeAction,
    ) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let mut detail = self
            .mail
            .get_message_detail(account_id, message_id, None)
            .await?;
        if detail.body_availability != crate::core::ContentAvailability::Available {
            detail = self
                .mail
                .request_message_body(account_id, message_id, None)
                .await?;
        }
        if action == MessageComposeAction::Forward {
            for attachment in &detail.attachments {
                if attachment.availability != crate::core::ContentAvailability::Available {
                    self.mail
                        .request_attachment(account_id, &attachment.id)
                        .await?;
                }
            }
        }
        let (original_message_label, wrote_label, from_label, to_label, subject_label) =
            match self.service.get_preferences()?.language {
                LanguagePreference::ZhCn => ("转发邮件", "写道：", "发件人", "收件人", "主题"),
                LanguagePreference::EnUs => {
                    ("Forwarded message", "wrote:", "From", "To", "Subject")
                }
            };
        let draft = self
            .repository()
            .await?
            .create_message_action_draft(CreateMessageActionDraftRequest {
                account_id,
                account_slot_id: &account.data_slot_id,
                own_email: &account.email,
                message_id,
                action,
                original_message_label,
                wrote_label,
                from_label,
                to_label,
                subject_label,
            })
            .await?;
        if let Err(error) = self.show_composer_window(account_id, &draft.id).await {
            let _ = self
                .repository()
                .await?
                .delete_editing_draft(&account.data_slot_id, &draft.id)
                .await;
            return Err(error);
        }
        Ok(())
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
        let builder = WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(860.0, 700.0)
            .min_inner_size(680.0, 560.0);
        #[cfg(target_os = "windows")]
        let builder = builder.decorations(false);
        #[cfg(target_os = "macos")]
        let builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(12.0, 11.0));
        builder
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
        self.mail.ensure_account_writable(account_id)?;
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
                    .add_draft_attachment(
                        &account.data_slot_id,
                        draft_id,
                        file_name,
                        &content_type,
                        &bytes,
                    )
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
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        self.repository()
            .await?
            .remove_draft_attachment(&account.data_slot_id, draft_id, attachment_id)
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

    pub async fn delete_draft(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        if self
            .app
            .get_webview_window(&format!("composer-{draft_id}"))
            .is_some()
        {
            return Err(CommandError::new("draft.window_open"));
        }
        self.repository()
            .await?
            .delete_editing_draft(&account.data_slot_id, draft_id)
            .await
    }

    pub async fn queue_remote_draft(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let draft = repository
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        let is_empty = draft.subject.trim().is_empty()
            && draft.recipients.to.is_empty()
            && draft.recipients.cc.is_empty()
            && draft.recipients.bcc.is_empty()
            && draft.content.plain_text.trim().is_empty()
            && (draft.content.html.trim().is_empty() || draft.content.html.trim() == "<p></p>")
            && draft.attachments.is_empty();
        if is_empty {
            return Ok(());
        }
        let Some((drafts_mailbox_id, _)) = repository
            .mailbox_for_role(&account.data_slot_id, MailboxRole::Drafts)
            .await?
        else {
            return Ok(());
        };
        let mut attachments = Vec::new();
        for stored in repository
            .draft_attachments(&account.data_slot_id, draft_id)
            .await?
        {
            attachments.push(OutgoingAttachment {
                file_name: stored.summary.file_name,
                content_type: stored.summary.content_type,
                bytes: repository.attachment_bytes(&stored.content_hash).await?,
            });
        }
        let sender = MessageAddress {
            name: nonempty(&account.display_name),
            email: account.email,
        };
        let raw = build_outgoing_message(
            &sender,
            &draft.recipients,
            &draft.subject,
            &draft.content,
            attachments,
        )?;
        let threading = repository
            .draft_threading_headers(&account.data_slot_id, draft_id)
            .await?;
        let raw = add_threading_headers(raw, &threading)?;
        let raw = add_draft_identity_headers(raw, draft_id, draft.revision)?;
        let hash = repository.write_send_mime(&raw).await?;
        repository
            .queue_draft_append(
                &account.data_slot_id,
                &drafts_mailbox_id,
                draft_id,
                &hash,
                draft.revision,
            )
            .await?;
        self.mail.wake_account(account_id);
        Ok(())
    }

    pub async fn queue_send(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<SendJobSummary> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let draft = repository
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        validate_recipient_fields(&draft.recipients, true)?;
        validate_content(&draft.content)?;
        let mut attachments = Vec::new();
        for stored in repository
            .draft_attachments(&account.data_slot_id, draft_id)
            .await?
        {
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
        let threading = repository
            .draft_threading_headers(&account.data_slot_id, draft_id)
            .await?;
        let raw = add_threading_headers(raw, &threading)?;
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
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        repository
            .get_send_job(account_id, &account.data_slot_id, job_id)
            .await?;
        repository
            .retry_send_job(&account.data_slot_id, job_id)
            .await?;
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

fn select_ready_account(
    slots: &[String],
    active_accounts: &HashSet<String>,
    fair_cursor: &mut usize,
) -> Option<String> {
    if slots.is_empty() {
        return None;
    }
    let index = (0..slots.len())
        .map(|offset| (*fair_cursor + offset) % slots.len())
        .find(|index| !active_accounts.contains(&slots[*index]))?;
    *fair_cursor = (index + 1) % slots.len();
    Some(slots[index].clone())
}

fn add_draft_identity_headers(
    mut raw: Vec<u8>,
    draft_id: &str,
    revision: u64,
) -> CommandResult<Vec<u8>> {
    if !draft_id
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || value == '-')
    {
        return Err(CommandError::new("draft.id_invalid"));
    }
    let separator = raw
        .windows(4)
        .position(|value| value == b"\r\n\r\n")
        .ok_or_else(|| CommandError::new("send.mime_build_failed"))?;
    let headers =
        format!("X-NextMail-Draft-ID: {draft_id}\r\nX-NextMail-Draft-Revision: {revision}\r\n");
    raw.splice(separator + 2..separator + 2, headers.bytes());
    Ok(raw)
}

fn add_threading_headers(
    mut raw: Vec<u8>,
    threading: &crate::storage::DraftThreadingHeaders,
) -> CommandResult<Vec<u8>> {
    let Some(in_reply_to) = threading
        .in_reply_to
        .as_deref()
        .and_then(normalize_message_id)
    else {
        return Ok(raw);
    };
    let mut references = threading
        .references
        .iter()
        .filter_map(|value| normalize_message_id(value))
        .collect::<Vec<_>>();
    if references.last() != Some(&in_reply_to) {
        references.push(in_reply_to.clone());
    }
    while references.join(" ").len() > 850 && references.len() > 1 {
        references.remove(0);
    }
    let separator = raw
        .windows(4)
        .position(|value| value == b"\r\n\r\n")
        .ok_or_else(|| CommandError::new("send.mime_build_failed"))?;
    let headers = format!(
        "In-Reply-To: {in_reply_to}\r\nReferences: {}\r\n",
        references.join(" ")
    );
    raw.splice(separator + 2..separator + 2, headers.bytes());
    Ok(raw)
}

fn normalize_message_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 900
        || value.chars().any(|character| character.is_control())
    {
        return None;
    }
    Some(if value.starts_with('<') && value.ends_with('>') {
        value.to_owned()
    } else {
        format!("<{value}>")
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendJobChangedEvent {
    account_id: String,
    draft_id: String,
    job_id: String,
    status: crate::core::SendJobStatus,
    subject: String,
    revision: u64,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{add_draft_identity_headers, add_threading_headers, select_ready_account};
    use crate::storage::DraftThreadingHeaders;

    #[test]
    fn adds_stable_draft_identity_before_the_message_body() {
        let raw = add_draft_identity_headers(
            b"From: sender@example.com\r\nSubject: Draft\r\n\r\nBody".to_vec(),
            "2e630859-f215-4860-a4c4-9fc50fbb132d",
            7,
        )
        .unwrap();
        let value = String::from_utf8(raw).unwrap();
        assert!(value.contains("X-NextMail-Draft-ID: 2e630859-f215-4860-a4c4-9fc50fbb132d\r\n"));
        assert!(value.contains("X-NextMail-Draft-Revision: 7\r\n\r\nBody"));
    }

    #[test]
    fn adds_safe_threading_headers_to_replies() {
        let raw = add_threading_headers(
            b"From: sender@example.com\r\nSubject: Reply\r\n\r\nBody".to_vec(),
            &DraftThreadingHeaders {
                in_reply_to: Some("original@example.com".into()),
                references: vec!["root@example.com".into()],
            },
        )
        .unwrap();
        let value = String::from_utf8(raw).unwrap();
        assert!(value.contains("In-Reply-To: <original@example.com>\r\n"));
        assert!(value.contains("References: <root@example.com> <original@example.com>\r\n\r\nBody"));
    }

    #[test]
    fn send_scheduler_rotates_between_accounts_without_parallelizing_one_account() {
        let slots = vec![
            "slot-a".to_owned(),
            "slot-b".to_owned(),
            "slot-c".to_owned(),
        ];
        let mut active = HashSet::new();
        let mut cursor = 0;

        let first = select_ready_account(&slots, &active, &mut cursor).unwrap();
        active.insert(first.clone());
        let second = select_ready_account(&slots, &active, &mut cursor).unwrap();
        active.insert(second.clone());
        assert_eq!((first.as_str(), second.as_str()), ("slot-a", "slot-b"));

        active.remove("slot-a");
        assert_eq!(
            select_ready_account(&slots, &active, &mut cursor).as_deref(),
            Some("slot-c")
        );
    }
}

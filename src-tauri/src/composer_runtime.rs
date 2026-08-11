mod definitions;
mod delivery;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use crate::core::{
    AccountRecord, AccountSummary, CommandError, CommandResult, ComposerBootstrap,
    CompositionDefinitionSummary, CompositionScene, DraftAttachmentSummary, DraftContent,
    DraftDetail, DraftListItem, DraftRecipientFields, DraftStatus, LanguagePreference,
    MessageAddress, MessageComposeAction, PreparedInlineImage,
};
use crate::storage::{
    MailRepository, PersistImportedDraftRequest, PersistMessageActionDraftRequest, SaveDraftRequest,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use lettre::Address;
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Notify;

use crate::{
    application::{
        assemble_composition_content, compose_imported_draft, compose_message_action_draft,
        is_empty_content, prefixed_message_action_subject, render_mail_signature,
        render_mail_template, AppService, CompositionRenderContext, MessageActionLabels,
    },
    mail_runtime::MailRuntime,
    window_titles::{window_title, WindowTitleKind},
};

const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
const MAX_TOTAL_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_COMPOSER_CONTENT_BYTES: usize = 5_000_000;
const MAX_DEFINITION_INLINE_IMAGE_BYTES: u64 = 3 * 1024 * 1024;

pub struct ComposerRuntime {
    app: AppHandle,
    service: Arc<AppService>,
    wake_worker: Notify,
    mail: Arc<MailRuntime>,
    started: AtomicBool,
}

impl ComposerRuntime {
    pub fn new(app: AppHandle, service: Arc<AppService>, mail: Arc<MailRuntime>) -> Self {
        Self {
            app,
            service,
            wake_worker: Notify::new(),
            mail,
            started: AtomicBool::new(false),
        }
    }

    pub async fn open_composer(&self, account_id: &str) -> CommandResult<String> {
        self.open_composer_with_recipients(account_id, DraftRecipientFields::default())
            .await
    }

    pub async fn open_composer_to_contact(
        &self,
        account_id: &str,
        contact: MessageAddress,
    ) -> CommandResult<String> {
        self.open_composer_with_recipients(
            account_id,
            DraftRecipientFields {
                to: vec![contact],
                cc: Vec::new(),
                bcc: Vec::new(),
            },
        )
        .await
    }

    async fn open_composer_with_recipients(
        &self,
        account_id: &str,
        recipients: DraftRecipientFields,
    ) -> CommandResult<String> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let empty = DraftContent {
            editor_json: r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.to_owned(),
            html: "<p></p>".to_owned(),
            plain_text: String::new(),
        };
        let (recipients, subject, content) = self
            .initial_composition(&account, CompositionScene::New, &recipients, "", &empty)
            .await?;
        let draft = self
            .repository()
            .await?
            .drafts()
            .create_initialized_draft_with_recipients(
                account_id,
                &account.data_slot_id,
                &recipients,
                &subject,
                &content,
            )
            .await?;
        if let Err(error) = self.show_composer_window(&account.id, &draft.id).await {
            if let Err(cleanup_error) = self
                .repository()
                .await?
                .drafts()
                .discard_empty_draft(&account.data_slot_id, &draft.id)
                .await
            {
                tracing::warn!(
                    draft_id = %draft.id,
                    code = %cleanup_error.code,
                    "failed composer draft cleanup failed"
                );
            }
            return Err(error);
        }
        Ok(draft.id)
    }

    pub async fn list_drafts(&self, account_id: &str) -> CommandResult<Vec<DraftListItem>> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .drafts()
            .list_editing_drafts(account_id, &account.data_slot_id)
            .await
    }

    pub async fn close_account_windows(&self, account_id: &str) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let drafts = self
            .repository()
            .await?
            .drafts()
            .list_editing_drafts(account_id, &account.data_slot_id)
            .await?;
        let labels = drafts
            .iter()
            .map(|draft| format!("composer-{}", draft.id))
            .collect::<Vec<_>>();
        for label in &labels {
            if let Some(window) = self.app.get_webview_window(label) {
                if let Err(error) = window.close() {
                    tracing::warn!(%label, ?error, "composer window close failed");
                }
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
            .drafts()
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
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        let draft = if let Some(existing) = drafts
            .existing_imported_draft(account_id, &account.data_slot_id, message_id)
            .await?
        {
            existing
        } else {
            let source = drafts
                .imported_draft_source(&account.data_slot_id, message_id)
                .await?;
            let content = compose_imported_draft(&source)?;
            drafts
                .persist_imported_draft(PersistImportedDraftRequest {
                    account_id,
                    account_slot_id: &account.data_slot_id,
                    message_id,
                    source: &source,
                    content: &content,
                })
                .await?
        };
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
        let language = self.service.get_preferences()?.language;
        let labels = match &language {
            LanguagePreference::ZhCn => MessageActionLabels {
                reply_original_message: "回复的原始邮件",
                forward_original_message: "转发的原始邮件",
                from: "发件人",
                date: "发件时间",
                to: "收件人",
                subject: "主题",
                reply_subject_prefix: "回复：",
                forward_subject_prefix: "转发：",
            },
            LanguagePreference::EnUs => MessageActionLabels {
                reply_original_message: "Original message — Reply",
                forward_original_message: "Original message — Forward",
                from: "From",
                date: "Sent",
                to: "To",
                subject: "Subject",
                reply_subject_prefix: "Re: ",
                forward_subject_prefix: "Fwd: ",
            },
        };
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        let mut source = drafts
            .message_action_source(&account.data_slot_id, message_id)
            .await?;
        source.safe_html = source
            .safe_html
            .as_deref()
            .map(crate::protocols::sanitize_mail_html_for_composer);
        let mut inline_images = Vec::new();
        if let Some(raw) = repository
            .read()
            .raw_message(&account.data_slot_id, message_id)
            .await?
        {
            let body = tokio::task::spawn_blocking(move || {
                crate::protocols::sanitize_raw_message_for_composer(&raw)
            })
            .await
            .map_err(|_| CommandError::new("message.mime_parse_failed"))?;
            if let Some(body) = body {
                inline_images = body.inline_images;
                if let Some(plain_text) = body.plain_text {
                    source.plain_text = plain_text;
                }
                if body.safe_html.is_some() {
                    source.safe_html = body.safe_html;
                }
            }
        }
        let sent_at = format_message_action_date(source.received_at, &language);
        let mut composed =
            compose_message_action_draft(&source, &account.email, action, labels, &sent_at)?;
        let scene = match action {
            MessageComposeAction::Reply => CompositionScene::Reply,
            MessageComposeAction::ReplyAll => CompositionScene::ReplyAll,
            MessageComposeAction::Forward => CompositionScene::Forward,
        };
        let (recipients, subject, content) = self
            .initial_composition(
                &account,
                scene,
                &composed.recipients,
                &composed.subject,
                &composed.content,
            )
            .await?;
        composed.recipients = recipients;
        composed.subject = prefixed_message_action_subject(&subject, action, &labels);
        composed.content = content;
        let draft = drafts
            .persist_message_action_draft(PersistMessageActionDraftRequest {
                account_id,
                account_slot_id: &account.data_slot_id,
                message_id,
                action,
                draft: &composed,
            })
            .await?;
        let mut inline_total = 0u64;
        for image in inline_images {
            let size = image.bytes.len() as u64;
            inline_total = inline_total.saturating_add(size);
            if size > MAX_ATTACHMENT_BYTES || inline_total > MAX_TOTAL_ATTACHMENT_BYTES {
                continue;
            }
            if let Err(error) = drafts
                .add_draft_inline_image(
                    &account.data_slot_id,
                    &draft.id,
                    &image.file_name,
                    &image.content_type,
                    Some(&image.content_id),
                    &image.bytes,
                )
                .await
            {
                if let Err(cleanup_error) = drafts
                    .delete_editing_draft(&account.data_slot_id, &draft.id)
                    .await
                {
                    tracing::warn!(
                        draft_id = %draft.id,
                        code = %cleanup_error.code,
                        "invalid inline image draft cleanup failed"
                    );
                }
                return Err(error);
            }
        }
        let draft = drafts
            .get_draft(account_id, &account.data_slot_id, &draft.id)
            .await?;
        if let Err(error) = self.show_composer_window(account_id, &draft.id).await {
            if let Err(cleanup_error) = self
                .repository()
                .await?
                .drafts()
                .delete_editing_draft(&account.data_slot_id, &draft.id)
                .await
            {
                tracing::warn!(
                    draft_id = %draft.id,
                    code = %cleanup_error.code,
                    "failed action composer draft cleanup failed"
                );
            }
            return Err(error);
        }
        Ok(())
    }

    async fn show_composer_window(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        let label = format!("composer-{draft_id}");
        if let Some(window) = self.app.get_webview_window(&label) {
            if !window.is_visible().unwrap_or(false) {
                return Ok(());
            }
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
        let title = window_title(
            &self.service.get_preferences()?.language,
            WindowTitleKind::Composer,
        );
        let builder = WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(860.0, 700.0)
            .min_inner_size(680.0, 560.0)
            .center()
            .visible(false);
        #[cfg(target_os = "windows")]
        let builder = builder.decorations(false);
        #[cfg(target_os = "macos")]
        let builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
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
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        let mut draft = drafts
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        let mut inline_previews = HashMap::new();
        for stored in drafts
            .draft_attachments(&account.data_slot_id, draft_id)
            .await?
        {
            if stored.summary.is_inline {
                let bytes = drafts.attachment_bytes(&stored.content_hash).await?;
                inline_previews.insert(
                    stored.summary.id,
                    format!(
                        "data:{};base64,{}",
                        stored.summary.content_type,
                        STANDARD.encode(bytes)
                    ),
                );
            }
        }
        for attachment in &mut draft.attachments {
            attachment.preview_data_url = inline_previews.remove(&attachment.id);
        }
        let definitions = repository.composition_definitions();
        let templates = definitions
            .available_mail_templates(account_id, &account.data_slot_id)
            .await?
            .into_iter()
            .map(|value| CompositionDefinitionSummary {
                id: value.id,
                name: value.name,
                scope: value.scope,
            })
            .collect();
        let signatures = definitions
            .available_mail_signatures(account_id, &account.data_slot_id)
            .await?
            .into_iter()
            .map(|value| CompositionDefinitionSummary {
                id: value.id,
                name: value.name,
                scope: value.scope,
            })
            .collect();
        Ok(ComposerBootstrap {
            draft,
            sender: AccountSummary::from(&account),
            templates,
            signatures,
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
        let content = sanitize_draft_content(content)?;
        validate_content(&content)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .drafts()
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

    pub fn sanitize_rich_text_paste(&self, html: &str) -> CommandResult<String> {
        if html.len() > MAX_COMPOSER_CONTENT_BYTES {
            return Err(CommandError::new("draft.content_too_large"));
        }
        Ok(crate::protocols::sanitize_rich_text_paste(html))
    }

    pub fn prepare_definition_inline_image(
        &self,
        file_name: String,
        content_type: String,
        content_base64: String,
    ) -> CommandResult<PreparedInlineImage> {
        prepare_definition_inline_image(file_name, content_type, content_base64)
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
        let drafts = repository.drafts();
        let draft = drafts
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
                drafts
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

    pub async fn add_inline_image(
        &self,
        account_id: &str,
        draft_id: &str,
        file_name: String,
        content_type: String,
        content_base64: String,
    ) -> CommandResult<DraftAttachmentSummary> {
        self.mail.ensure_account_writable(account_id)?;
        let content_type = content_type.trim().to_ascii_lowercase();
        if !matches!(
            content_type.as_str(),
            "image/bmp" | "image/gif" | "image/jpeg" | "image/png" | "image/webp"
        ) {
            return Err(CommandError::new("attachment.image_type_unsupported"));
        }
        if content_base64.len() as u64 > (MAX_ATTACHMENT_BYTES * 4 / 3) + 8 {
            return Err(CommandError::new("attachment.too_large"));
        }
        let bytes = STANDARD
            .decode(content_base64.trim())
            .map_err(|_| CommandError::new("attachment.image_invalid"))?;
        if bytes.is_empty() {
            return Err(CommandError::new("attachment.image_invalid"));
        }
        if !valid_image_signature(&content_type, &bytes) {
            return Err(CommandError::new("attachment.image_invalid"));
        }
        if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
            return Err(CommandError::new("attachment.too_large"));
        }
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        let draft = drafts
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        let total = draft
            .attachments
            .iter()
            .map(|item| item.size)
            .sum::<u64>()
            .saturating_add(bytes.len() as u64);
        if total > MAX_TOTAL_ATTACHMENT_BYTES {
            return Err(CommandError::new("attachment.total_too_large"));
        }
        let mut summary = drafts
            .add_draft_inline_image(
                &account.data_slot_id,
                draft_id,
                &file_name,
                &content_type,
                None,
                &bytes,
            )
            .await?;
        summary.preview_data_url = Some(format!(
            "data:{};base64,{}",
            content_type,
            STANDARD.encode(&bytes)
        ));
        Ok(summary)
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
            .drafts()
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        self.repository()
            .await?
            .drafts()
            .remove_draft_attachment(&account.data_slot_id, draft_id, attachment_id)
            .await
    }

    pub async fn discard_empty_draft(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<bool> {
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        drafts
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        drafts
            .discard_empty_draft(&account.data_slot_id, draft_id)
            .await
    }

    pub async fn discard_draft_session(
        &self,
        account_id: &str,
        draft_id: &str,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .drafts()
            .delete_editing_draft(&account.data_slot_id, draft_id)
            .await
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
            .drafts()
            .delete_editing_draft(&account.data_slot_id, draft_id)
            .await
    }

    async fn repository(&self) -> CommandResult<&Arc<MailRepository>> {
        self.mail.repository().await
    }

    async fn initial_composition(
        &self,
        account: &AccountRecord,
        scene: CompositionScene,
        recipients: &DraftRecipientFields,
        base_subject: &str,
        base_content: &DraftContent,
    ) -> CommandResult<(DraftRecipientFields, String, DraftContent)> {
        let repository = self.repository().await?;
        let definitions = repository.composition_definitions();
        let rule = definitions
            .resolved_composition_scene_rule(&account.data_slot_id, scene)
            .await?;
        let signature_preferences = definitions
            .signature_preferences(Some(&account.data_slot_id))
            .await?;
        let template = if let Some(id) = rule.template_id.as_deref() {
            Some(
                definitions
                    .available_mail_template(&account.id, &account.data_slot_id, id)
                    .await?,
            )
        } else {
            None
        };
        let effective_recipients = template_recipient_override(
            template
                .as_ref()
                .and_then(|value| value.recipients.as_ref()),
            recipients,
        );
        let context = self.render_context(account, effective_recipients.to.first())?;
        let template = template
            .as_ref()
            .map(|value| render_mail_template(value, &context))
            .transpose()?;
        let signature = if signature_preferences.auto_insert {
            signature_preferences.default_signature_id.as_deref()
        } else {
            None
        };
        let signature = if let Some(id) = signature {
            let value = definitions
                .available_mail_signature(&account.id, &account.data_slot_id, id)
                .await?;
            Some(render_mail_signature(&value, &context)?)
        } else {
            None
        };
        let subject = template
            .as_ref()
            .map(|value| value.subject.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(base_subject)
            .to_owned();
        let content_template = template
            .as_ref()
            .filter(|value| !is_empty_content(&value.content));
        let content =
            assemble_composition_content(base_content, content_template, signature.as_ref())?;
        Ok((effective_recipients, subject, content))
    }

    fn render_context<'a>(
        &self,
        account: &'a AccountRecord,
        recipient: Option<&'a MessageAddress>,
    ) -> CommandResult<CompositionRenderContext<'a>> {
        Ok(CompositionRenderContext {
            sender: MessageAddress {
                name: nonempty(&account.display_name),
                email: account.email.clone(),
            },
            recipient,
            language: self.service.get_preferences()?.language,
        })
    }

    fn definition_account(&self, account_id: Option<&str>) -> CommandResult<Option<AccountRecord>> {
        account_id
            .map(|value| self.service.account_record(value))
            .transpose()
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

fn template_recipient_override(
    template_recipients: Option<&DraftRecipientFields>,
    current_recipients: &DraftRecipientFields,
) -> DraftRecipientFields {
    let Some(template) = template_recipients else {
        return current_recipients.clone();
    };
    DraftRecipientFields {
        to: if template.to.is_empty() {
            current_recipients.to.clone()
        } else {
            template.to.clone()
        },
        cc: if template.cc.is_empty() {
            current_recipients.cc.clone()
        } else {
            template.cc.clone()
        },
        bcc: if template.bcc.is_empty() {
            current_recipients.bcc.clone()
        } else {
            template.bcc.clone()
        },
    }
}

fn validate_content(content: &DraftContent) -> CommandResult<()> {
    if content.editor_json.len() > MAX_COMPOSER_CONTENT_BYTES
        || content.html.len() > MAX_COMPOSER_CONTENT_BYTES
        || content.plain_text.len() > MAX_COMPOSER_CONTENT_BYTES
    {
        return Err(CommandError::new("draft.content_too_large"));
    }
    serde_json::from_str::<serde_json::Value>(&content.editor_json)
        .map_err(|_| CommandError::new("draft.editor_json_invalid"))?;
    Ok(())
}

fn sanitize_draft_content(mut content: DraftContent) -> CommandResult<DraftContent> {
    let mut editor_json = serde_json::from_str::<serde_json::Value>(&content.editor_json)
        .map_err(|_| CommandError::new("draft.editor_json_invalid"))?;
    sanitize_original_source_nodes(&mut editor_json);
    content.editor_json = serde_json::to_string(&editor_json)
        .map_err(|_| CommandError::new("draft.editor_json_invalid"))?;
    content.html = crate::protocols::sanitize_composer_document(&content.html);
    Ok(content)
}

fn valid_image_signature(content_type: &str, bytes: &[u8]) -> bool {
    crate::protocols::detected_raster_content_type(bytes)
        .is_some_and(|detected| detected.eq_ignore_ascii_case(content_type))
}

fn prepare_definition_inline_image(
    file_name: String,
    content_type: String,
    content_base64: String,
) -> CommandResult<PreparedInlineImage> {
    let content_type = content_type.trim().to_ascii_lowercase();
    if !matches!(
        content_type.as_str(),
        "image/bmp" | "image/gif" | "image/jpeg" | "image/png" | "image/webp"
    ) {
        return Err(CommandError::new("attachment.image_type_unsupported"));
    }
    if content_base64.len() as u64 > (MAX_DEFINITION_INLINE_IMAGE_BYTES * 4 / 3) + 8 {
        return Err(CommandError::new("definition.image_too_large"));
    }
    let bytes = STANDARD
        .decode(content_base64.trim())
        .map_err(|_| CommandError::new("attachment.image_invalid"))?;
    if bytes.is_empty() || !valid_image_signature(&content_type, &bytes) {
        return Err(CommandError::new("attachment.image_invalid"));
    }
    if bytes.len() as u64 > MAX_DEFINITION_INLINE_IMAGE_BYTES {
        return Err(CommandError::new("definition.image_too_large"));
    }
    let file_name = file_name.trim();
    let file_name = if file_name.is_empty() {
        "inline-image".to_owned()
    } else {
        file_name.chars().take(255).collect()
    };
    Ok(PreparedInlineImage {
        file_name,
        content_type: content_type.clone(),
        size: bytes.len() as u64,
        data_url: format!("data:{content_type};base64,{}", STANDARD.encode(bytes)),
    })
}

fn sanitize_original_source_nodes(value: &mut serde_json::Value) {
    if value.get("type").and_then(serde_json::Value::as_str) == Some("nextmailOriginalMessage") {
        if let Some(source) = value
            .get_mut("attrs")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|attrs| attrs.get_mut("sourceHtml"))
        {
            if let Some(html) = source.as_str() {
                *source =
                    serde_json::Value::String(crate::protocols::sanitize_composer_document(html));
            }
        }
    }
    if let Some(children) = value
        .get_mut("content")
        .and_then(serde_json::Value::as_array_mut)
    {
        for child in children {
            sanitize_original_source_nodes(child);
        }
    }
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

fn format_message_action_date(timestamp: i64, language: &LanguagePreference) -> String {
    use chrono::{Datelike, Local, Timelike};

    let Some(value) =
        chrono::DateTime::from_timestamp(timestamp, 0).map(|value| value.with_timezone(&Local))
    else {
        return String::new();
    };
    match language {
        LanguagePreference::ZhCn => format!(
            "{}年{}月{}日 {:02}:{:02}",
            value.year(),
            value.month(),
            value.day(),
            value.hour(),
            value.minute()
        ),
        LanguagePreference::EnUs => value.format("%Y-%m-%d %H:%M").to_string(),
    }
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

    use super::{
        add_draft_identity_headers, add_threading_headers, prepare_definition_inline_image,
        sanitize_draft_content, select_ready_account, template_recipient_override,
        valid_image_signature,
    };
    use crate::core::{DraftContent, DraftRecipientFields, MessageAddress};
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

    #[test]
    fn template_recipients_override_only_nonempty_fields() {
        let current = DraftRecipientFields {
            to: vec![MessageAddress {
                name: None,
                email: "current@example.com".into(),
            }],
            cc: vec![MessageAddress {
                name: None,
                email: "current-cc@example.com".into(),
            }],
            bcc: Vec::new(),
        };
        let empty_template = DraftRecipientFields::default();
        let partial_template = DraftRecipientFields {
            to: vec![MessageAddress {
                name: Some("Template".into()),
                email: "template@example.com".into(),
            }],
            cc: Vec::new(),
            bcc: Vec::new(),
        };

        assert_eq!(template_recipient_override(None, &current), current);
        assert_eq!(
            template_recipient_override(Some(&empty_template), &current),
            current
        );
        let merged = template_recipient_override(Some(&partial_template), &current);
        assert_eq!(merged.to, partial_template.to);
        assert_eq!(merged.cc, current.cc);
        assert_eq!(merged.bcc, current.bcc);
    }

    #[test]
    fn source_html_is_sanitized_again_before_draft_persistence() {
        let content = sanitize_draft_content(DraftContent {
            editor_json: serde_json::json!({
                "type": "doc",
                "content": [{
                    "type": "nextmailOriginalMessage",
                    "attrs": {
                        "sourceHtml": "<script>bad()</script><img src=\"cid:logo@example.test\">"
                    }
                }]
            })
            .to_string(),
            html: "<script>bad()</script><p><img src=\"cid:logo@example.test\"></p>".into(),
            plain_text: "Logo".into(),
        })
        .unwrap();
        assert!(!content.html.contains("script"));
        assert!(content.html.contains("cid:logo@example.test"));
        assert!(!content.editor_json.contains("script"));
        assert!(content.editor_json.contains("cid:logo@example.test"));
    }

    #[test]
    fn pasted_image_type_must_match_its_magic_bytes() {
        assert!(valid_image_signature(
            "image/png",
            b"\x89PNG\r\n\x1a\ncontent"
        ));
        assert!(!valid_image_signature("image/png", b"<svg></svg>"));
        assert!(valid_image_signature(
            "image/webp",
            b"RIFF\x04\x00\x00\x00WEBPdata"
        ));
        let mut bmp = vec![0; 54];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&54_u32.to_le_bytes());
        bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        assert!(valid_image_signature("image/bmp", &bmp));
        assert!(!valid_image_signature("image/bmp", b"BMnot-a-bmp"));
    }

    #[test]
    fn reusable_definition_images_are_bounded_and_magic_checked() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let png = b"\x89PNG\r\n\x1a\ncontent";
        let prepared = prepare_definition_inline_image(
            "logo.png".into(),
            "image/png".into(),
            STANDARD.encode(png),
        )
        .unwrap();
        assert_eq!(prepared.file_name, "logo.png");
        assert_eq!(prepared.size, png.len() as u64);
        assert!(prepared.data_url.starts_with("data:image/png;base64,"));

        let error = prepare_definition_inline_image(
            "logo.png".into(),
            "image/png".into(),
            STANDARD.encode(b"<svg></svg>"),
        )
        .unwrap_err();
        assert_eq!(error.code, "attachment.image_invalid");
    }
}

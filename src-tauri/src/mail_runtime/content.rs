use std::sync::Arc;

use crate::{
    adapters::{open_prepared_attachment, reveal_prepared_attachment},
    core::{
        AttachmentSummary, CommandError, CommandResult, MailSyncSink, MessageDetail,
        RemoteMessageBody,
    },
};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use super::{
    AttachmentDownloadStartedEvent, MailRuntime, MessageBodyProgressEvent,
    MessageContentChangedEvent,
};

impl MailRuntime {
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
        self.request_message_body_inner(account_id, message_id, mailbox_id, false)
            .await
    }

    pub async fn request_message_body_with_progress(
        &self,
        account_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<MessageDetail> {
        self.request_message_body_inner(account_id, message_id, mailbox_id, true)
            .await
    }

    async fn request_message_body_inner(
        &self,
        account_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
        emit_progress: bool,
    ) -> CommandResult<MessageDetail> {
        if emit_progress {
            self.emit_message_body_progress(account_id, message_id, "preparing", 5);
        }
        let account = self.service.account_record(account_id)?;
        let repository = Arc::clone(self.repository().await?);
        if let Some(raw) = repository
            .read()
            .raw_message(&account.data_slot_id, message_id)
            .await?
        {
            if emit_progress {
                self.emit_message_body_progress(account_id, message_id, "processing", 55);
            }
            let body = tokio::task::spawn_blocking(move || {
                crate::protocols::sanitize_raw_message_body(&raw)
            })
            .await
            .map_err(|_| CommandError::new("message.mime_parse_failed"))?;
            if let Some(body) = body {
                let body = RemoteMessageBody {
                    plain_text: body.plain_text,
                    safe_html: body.safe_html,
                    preview: None,
                    attachments: Vec::new(),
                    remote_images_blocked: body.remote_images_blocked,
                    inline_content_ids: body.inline_content_ids,
                };
                repository
                    .sync_sink()
                    .replace_message_body(&account.data_slot_id, message_id, &body)
                    .await?;
                if emit_progress {
                    self.emit_message_body_progress(account_id, message_id, "updating", 90);
                }
                let detail = repository
                    .read()
                    .get_message_detail(&account.data_slot_id, message_id, mailbox_id)
                    .await?;
                if let Err(error) = self.app.emit(
                    "message-content-changed",
                    MessageContentChangedEvent {
                        account_id: account_id.to_owned(),
                        message_id: message_id.to_owned(),
                        revision: detail.revision,
                    },
                ) {
                    tracing::warn!(
                        %account_id,
                        %message_id,
                        ?error,
                        "message content event failed"
                    );
                }
                if emit_progress {
                    self.emit_message_body_progress(account_id, message_id, "complete", 100);
                }
                return Ok(detail);
            }
        }
        if emit_progress {
            self.emit_message_body_progress(account_id, message_id, "downloading", 20);
        }
        self.fetch_and_store_message_body(account_id, message_id)
            .await?;
        if emit_progress {
            self.emit_message_body_progress(account_id, message_id, "updating", 90);
        }
        let detail = self
            .get_message_detail(account_id, message_id, mailbox_id)
            .await?;
        if emit_progress {
            self.emit_message_body_progress(account_id, message_id, "complete", 100);
        }
        Ok(detail)
    }

    fn emit_message_body_progress(
        &self,
        account_id: &str,
        message_id: &str,
        stage: &'static str,
        progress: u8,
    ) {
        if let Err(error) = self.app.emit(
            "message-body-progress",
            MessageBodyProgressEvent {
                account_id: account_id.to_owned(),
                message_id: message_id.to_owned(),
                stage,
                progress,
            },
        ) {
            tracing::warn!(
                %account_id,
                %message_id,
                %stage,
                progress,
                ?error,
                "message body progress event failed"
            );
        }
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
        // Let the UI distinguish "content is actually being fetched" from
        // dialogs or other pre-download waits (e.g. save-as target selection).
        if let Err(error) = self.app.emit(
            "attachment-download-started",
            AttachmentDownloadStartedEvent {
                account_id: account_id.to_owned(),
                attachment_id: attachment_id.to_owned(),
            },
        ) {
            tracing::warn!(
                %account_id,
                %attachment_id,
                ?error,
                "attachment download started event failed"
            );
        }
        self.ensure_account_writable(account_id)?;
        let (message_id, part_index, imap_section) = repository
            .read()
            .attachment_context(&account.data_slot_id, attachment_id)
            .await?;
        let content = match repository
            .read()
            .raw_message(&account.data_slot_id, &message_id)
            .await?
        {
            Some(raw) => crate::protocols::extract_attachment(&raw, part_index)?,
            None => match imap_section {
                Some(section) => {
                    let context = repository
                        .read()
                        .remote_message_context(&account.data_slot_id, &message_id)
                        .await?;
                    let config = self.imap_config(&account.id).await?;
                    let _permit = self
                        .network_limit
                        .acquire()
                        .await
                        .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
                    match self
                        .provider
                        .fetch_attachment(
                            &config,
                            &context.mailbox_name,
                            context.uid,
                            context.uid_validity,
                            &section,
                        )
                        .await
                    {
                        Ok(content) => content,
                        Err(error)
                            if error.code == crate::protocols::SELECTIVE_FETCH_UNSUPPORTED =>
                        {
                            drop(_permit);
                            self.fetch_and_store_message(&account.id, &message_id)
                                .await?;
                            let raw = repository
                                .read()
                                .raw_message(&account.data_slot_id, &message_id)
                                .await?
                                .ok_or_else(|| CommandError::new("message.raw_unavailable"))?;
                            crate::protocols::extract_attachment(&raw, part_index)?
                        }
                        Err(error) => return Err(error),
                    }
                }
                None => {
                    self.fetch_and_store_message(&account.id, &message_id)
                        .await?;
                    let raw = repository
                        .read()
                        .raw_message(&account.data_slot_id, &message_id)
                        .await?
                        .ok_or_else(|| CommandError::new("message.raw_unavailable"))?;
                    crate::protocols::extract_attachment(&raw, part_index)?
                }
            },
        };
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

    pub async fn reveal_message_attachment(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<()> {
        let prepared = self
            .prepare_message_attachment(account_id, attachment_id)
            .await?;
        reveal_prepared_attachment(self.attachment_opener.as_ref(), &prepared)
    }

    pub async fn save_message_attachment_as(
        &self,
        account_id: &str,
        attachment_id: &str,
    ) -> CommandResult<bool> {
        // Ask for the destination before fetching anything so a cancelled dialog
        // never downloads attachment content.
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let summary = repository
            .read()
            .attachment_summary(&account.data_slot_id, attachment_id)
            .await?;
        let suggested_name = crate::storage::sanitize_attachment_file_name(&summary.file_name);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.app
            .dialog()
            .file()
            .set_file_name(&suggested_name)
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
        let prepared = self
            .prepare_message_attachment(account_id, attachment_id)
            .await?;
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
        if let Err(error) = self.app.emit(
            "message-content-changed",
            MessageContentChangedEvent {
                account_id: account_id.to_owned(),
                message_id: message_id.to_owned(),
                revision,
            },
        ) {
            tracing::warn!(%account_id, %message_id, ?error, "message content event failed");
        }
        Ok(())
    }

    async fn fetch_and_store_message_body(
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
        match self
            .provider
            .fetch_message_body(
                &config,
                &context.mailbox_name,
                context.uid,
                context.uid_validity,
            )
            .await
        {
            Ok(body) => {
                repository
                    .sync_sink()
                    .replace_message_body(&account.data_slot_id, message_id, &body)
                    .await?;
                let revision = repository
                    .read()
                    .get_message_detail(
                        &account.data_slot_id,
                        message_id,
                        Some(&context.mailbox_id),
                    )
                    .await?
                    .revision;
                if let Err(error) = self.app.emit(
                    "message-content-changed",
                    MessageContentChangedEvent {
                        account_id: account_id.to_owned(),
                        message_id: message_id.to_owned(),
                        revision,
                    },
                ) {
                    tracing::warn!(%account_id, %message_id, ?error, "message content event failed");
                }
                Ok(())
            }
            Err(error) if error.code == crate::protocols::SELECTIVE_FETCH_UNSUPPORTED => {
                drop(_permit);
                self.fetch_and_store_message(account_id, message_id).await
            }
            Err(error) => Err(error),
        }
    }
}

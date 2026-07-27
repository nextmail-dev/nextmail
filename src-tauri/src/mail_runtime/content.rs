use std::sync::Arc;

use crate::{
    adapters::open_prepared_attachment,
    core::{AttachmentSummary, CommandError, CommandResult, MailSyncSink, MessageDetail},
};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use super::{MailRuntime, MessageBodyProgressEvent, MessageContentChangedEvent};

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
                repository
                    .sync_sink()
                    .replace_message_body(
                        &account.data_slot_id,
                        message_id,
                        body.plain_text.as_deref(),
                        body.safe_html.as_deref(),
                        body.remote_images_blocked,
                        &body.inline_content_ids,
                    )
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
        self.fetch_and_store_message(account_id, message_id).await?;
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
}

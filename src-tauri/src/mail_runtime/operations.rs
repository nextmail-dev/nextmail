use std::sync::Arc;

use crate::{
    core::{
        CommandError, CommandResult, ImapAccountConfig, MailboxRole, PendingOperationKind,
        PendingOperationSummary, RemoteOperationOutcome,
    },
    storage::{MailRepository, PendingOperationWork},
};
use tauri::Emitter;

use super::{
    remote_operation, required_destination, required_payload, MailRuntime, MailboxChangedEvent,
    MessageContentChangedEvent, PendingOperationChangedEvent,
};

impl MailRuntime {
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
        self.request_pending_operations(account_id);
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
        self.request_pending_operations(account_id);
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
        self.request_pending_operations(account_id);
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
        self.request_pending_operations(account_id);
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
        self.request_pending_operations(account_id);
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
        self.request_pending_operations(account_id);
        Ok(())
    }

    pub(super) async fn drain_pending_operations(
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
                    tracing::warn!(
                        kind = ?work.kind,
                        uid = ?work.uid,
                        mailbox = ?work.source_mailbox_name,
                        attempt = work.attempt_count,
                        code = %error.code,
                        retryable = error.retryable,
                        "pending operation failed"
                    );
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
            if let Err(error) = self.app.emit(
                "message-content-changed",
                MessageContentChangedEvent {
                    account_id: account_id.to_owned(),
                    message_id: message_id.clone(),
                    revision: 0,
                },
            ) {
                tracing::warn!(
                    %account_id,
                    %message_id,
                    ?error,
                    "message content event failed"
                );
            }
        }
    }

    fn emit_mailbox_change(&self, account_id: &str, mailbox_id: &str, revision: u64) {
        if let Err(error) = self.app.emit(
            "mailbox-changed",
            MailboxChangedEvent {
                account_id: account_id.to_owned(),
                mailbox_id: mailbox_id.to_owned(),
                revision,
            },
        ) {
            tracing::warn!(%account_id, %mailbox_id, ?error, "mailbox event failed");
        }
    }

    fn emit_pending_operation(&self, account_id: &str, operation_id: &str, status: &str) {
        if let Err(error) = self.app.emit(
            "pending-operation-changed",
            PendingOperationChangedEvent {
                account_id: account_id.to_owned(),
                operation_id: operation_id.to_owned(),
                status: status.to_owned(),
            },
        ) {
            tracing::warn!(
                %account_id,
                %operation_id,
                %status,
                ?error,
                "pending operation event failed"
            );
        }
    }
}

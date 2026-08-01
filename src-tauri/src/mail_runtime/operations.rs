use std::sync::Arc;

use crate::{
    core::{
        CommandError, CommandResult, ImapAccountConfig, MailboxRole, MailboxSyncTarget,
        PendingOperationKind, PendingOperationSummary, RemoteMailboxOperation,
        RemoteOperationOutcome,
    },
    storage::{MailRepository, PendingOperationWork},
};
use tauri::Emitter;

use super::{
    remote_operation, required_destination, required_payload, MailRuntime, MailboxChangedEvent,
    MessageContentChangedEvent, PendingOperationChangedEvent, RuntimeObserver,
};

impl MailRuntime {
    pub async fn create_mailbox(
        &self,
        account_id: &str,
        parent_mailbox_id: Option<&str>,
        name: &str,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let mailboxes = repository.mailboxes();
        let parent = match parent_mailbox_id {
            Some(mailbox_id) => Some(
                mailboxes
                    .mutation_context(&account.data_slot_id, mailbox_id)
                    .await?,
            ),
            None => None,
        };
        let delimiter = match parent.as_ref() {
            Some(parent) => parent.delimiter.clone(),
            None => mailboxes.default_delimiter(&account.data_slot_id).await?,
        };
        let leaf_name = validate_mailbox_leaf(name, delimiter.as_deref())?;
        let display_name = join_display_mailbox_path(
            parent.as_ref().map(|value| value.display_name.as_str()),
            delimiter.as_deref(),
            &leaf_name,
        )?;
        let operation = RemoteMailboxOperation::Create {
            parent_mailbox: parent.as_ref().map(|value| value.remote_name.clone()),
            delimiter: delimiter.clone(),
            leaf_name,
        };
        let config = self.imap_config(account_id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        let outcome = self
            .provider
            .apply_mailbox_operation(&config, &operation)
            .await?;
        let remote_name = outcome
            .mailbox_name
            .ok_or_else(|| CommandError::new("mailbox.remote_name_missing"))?;
        let mailbox_id = mailboxes
            .insert_created_mailbox(
                &account.data_slot_id,
                &remote_name,
                &display_name,
                delimiter.as_deref(),
            )
            .await?;
        self.emit_mailbox_change(account_id, &mailbox_id, 0);
        Ok(())
    }

    pub async fn rename_mailbox(
        &self,
        account_id: &str,
        mailbox_id: &str,
        name: &str,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let mailboxes = repository.mailboxes();
        let source = mailboxes
            .mutation_context(&account.data_slot_id, mailbox_id)
            .await?;
        ensure_mailbox_structure_mutable(&source.remote_name)?;
        let leaf_name = validate_mailbox_leaf(name, source.delimiter.as_deref())?;
        let source_parent_remote =
            mailbox_parent_name(&source.remote_name, source.delimiter.as_deref());
        let source_parent_display =
            mailbox_parent_name(&source.display_name, source.delimiter.as_deref());
        let display_name = join_display_mailbox_path(
            source_parent_display,
            source.delimiter.as_deref(),
            &leaf_name,
        )?;
        let operation = RemoteMailboxOperation::Rename {
            source_mailbox: source.remote_name.clone(),
            destination_parent: source_parent_remote.map(str::to_owned),
            delimiter: source.delimiter.clone(),
            leaf_name,
        };
        self.apply_mailbox_rename(
            account_id,
            &account.data_slot_id,
            &source,
            &display_name,
            operation,
        )
        .await
    }

    pub async fn move_mailbox(
        &self,
        account_id: &str,
        mailbox_id: &str,
        destination_parent_mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let mailboxes = repository.mailboxes();
        let source = mailboxes
            .mutation_context(&account.data_slot_id, mailbox_id)
            .await?;
        ensure_mailbox_structure_mutable(&source.remote_name)?;
        let destination_parent = match destination_parent_mailbox_id {
            Some(destination_id) => Some(
                mailboxes
                    .mutation_context(&account.data_slot_id, destination_id)
                    .await?,
            ),
            None => None,
        };
        if let Some(parent) = destination_parent.as_ref() {
            let delimiter = source.delimiter.as_deref().unwrap_or_default();
            let descendant_prefix = format!("{}{}", source.remote_name, delimiter);
            if parent.id == source.id
                || (!delimiter.is_empty() && parent.remote_name.starts_with(&descendant_prefix))
            {
                return Err(CommandError::new("mailbox.move_into_self"));
            }
        }
        let delimiter = match destination_parent.as_ref() {
            Some(parent) => parent.delimiter.clone(),
            None => source.delimiter.clone(),
        };
        let leaf_name = mailbox_leaf_name(&source.display_name, source.delimiter.as_deref());
        let current_parent = mailbox_parent_name(&source.remote_name, source.delimiter.as_deref());
        let destination_parent_remote = destination_parent
            .as_ref()
            .map(|value| value.remote_name.as_str());
        if current_parent == destination_parent_remote {
            return Err(CommandError::new("mailbox.same_parent"));
        }
        let display_name = join_display_mailbox_path(
            destination_parent
                .as_ref()
                .map(|value| value.display_name.as_str()),
            delimiter.as_deref(),
            leaf_name,
        )?;
        let operation = RemoteMailboxOperation::Rename {
            source_mailbox: source.remote_name.clone(),
            destination_parent: destination_parent
                .as_ref()
                .map(|value| value.remote_name.clone()),
            delimiter,
            leaf_name: leaf_name.to_owned(),
        };
        self.apply_mailbox_rename(
            account_id,
            &account.data_slot_id,
            &source,
            &display_name,
            operation,
        )
        .await
    }

    pub async fn delete_mailbox(&self, account_id: &str, mailbox_id: &str) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let mailboxes = repository.mailboxes();
        let source = mailboxes
            .mutation_context(&account.data_slot_id, mailbox_id)
            .await?;
        ensure_mailbox_structure_mutable(&source.remote_name)?;
        let config = self.imap_config(account_id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        self.provider
            .apply_mailbox_operation(
                &config,
                &RemoteMailboxOperation::Delete {
                    mailbox_name: source.remote_name,
                },
            )
            .await?;
        mailboxes
            .delete_mailbox(&account.data_slot_id, mailbox_id)
            .await?;
        self.emit_mailbox_change(account_id, mailbox_id, 0);
        Ok(())
    }

    pub async fn mark_mailbox_all_read(
        &self,
        account_id: &str,
        mailbox_id: &str,
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let mailboxes = repository.mailboxes();
        let source = mailboxes
            .mutation_context(&account.data_slot_id, mailbox_id)
            .await?;
        if !source.selectable {
            return Err(CommandError::new("mailbox.not_selectable"));
        }
        let config = self.imap_config(account_id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        self.provider
            .apply_mailbox_operation(
                &config,
                &RemoteMailboxOperation::MarkAllRead {
                    mailbox_name: source.remote_name,
                },
            )
            .await?;
        mailboxes
            .mark_all_read(&account.data_slot_id, mailbox_id)
            .await?;
        self.emit_mailbox_change(account_id, mailbox_id, 0);
        Ok(())
    }

    pub async fn reorder_mailboxes(
        &self,
        account_id: &str,
        ordered_mailbox_ids: &[String],
    ) -> CommandResult<()> {
        self.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        self.repository()
            .await?
            .mailboxes()
            .reorder(&account.data_slot_id, ordered_mailbox_ids)
            .await?;
        self.emit_mailbox_change(account_id, "", 0);
        Ok(())
    }

    async fn apply_mailbox_rename(
        &self,
        account_id: &str,
        account_slot_id: &str,
        source: &crate::storage::MailboxMutationContext,
        destination_display_name: &str,
        operation: RemoteMailboxOperation,
    ) -> CommandResult<()> {
        let config = self.imap_config(account_id).await?;
        let _permit = self
            .network_limit
            .acquire()
            .await
            .map_err(|_| CommandError::retryable("account.network_unavailable"))?;
        let outcome = self
            .provider
            .apply_mailbox_operation(&config, &operation)
            .await?;
        let remote_name = outcome
            .mailbox_name
            .ok_or_else(|| CommandError::new("mailbox.remote_name_missing"))?;
        self.repository()
            .await?
            .mailboxes()
            .rename_mailbox_tree(
                account_slot_id,
                source,
                &remote_name,
                destination_display_name,
            )
            .await?;
        self.emit_mailbox_change(account_id, &source.id, 0);
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
                    if work.kind == PendingOperationKind::AppendDraft {
                        if let Err(error) = self
                            .refresh_appended_draft(
                                &repository,
                                &config,
                                account_id,
                                &account.data_slot_id,
                                generation,
                                &work,
                            )
                            .await
                        {
                            tracing::warn!(
                                %account_id,
                                code = %error.code,
                                retryable = error.retryable,
                                "appended draft mailbox refresh failed"
                            );
                        }
                    }
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

    async fn refresh_appended_draft(
        &self,
        repository: &MailRepository,
        config: &ImapAccountConfig,
        account_id: &str,
        account_slot_id: &str,
        generation: u64,
        work: &PendingOperationWork,
    ) -> CommandResult<()> {
        let mailbox_id = work
            .destination_mailbox_id
            .as_deref()
            .ok_or_else(|| CommandError::new("operation.destination_required"))?;
        let mailbox = repository
            .mailboxes()
            .mutation_context(account_slot_id, mailbox_id)
            .await?;
        let observer = RuntimeObserver {
            runtime: self,
            account_id: account_id.to_owned(),
            generation,
            report_progress: false,
            candidates: std::sync::Mutex::new(Vec::new()),
        };
        let sink = repository.sync_sink();
        self.provider
            .synchronize_mailbox(
                config,
                &MailboxSyncTarget {
                    name: mailbox.remote_name,
                    display_name: mailbox.display_name,
                    delimiter: mailbox.delimiter,
                    role: MailboxRole::Drafts,
                },
                &sink,
                &observer,
            )
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

fn validate_mailbox_leaf(name: &str, delimiter: Option<&str>) -> CommandResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::new("mailbox.name_required"));
    }
    if name.chars().any(char::is_control)
        || delimiter
            .filter(|value| !value.is_empty())
            .is_some_and(|value| name.contains(value))
    {
        return Err(CommandError::new("mailbox.name_invalid"));
    }
    Ok(name.to_owned())
}

fn ensure_mailbox_structure_mutable(remote_name: &str) -> CommandResult<()> {
    if remote_name.eq_ignore_ascii_case("INBOX") {
        Err(CommandError::new("mailbox.inbox_immutable"))
    } else {
        Ok(())
    }
}

fn mailbox_parent_name<'a>(name: &'a str, delimiter: Option<&str>) -> Option<&'a str> {
    let delimiter = delimiter.filter(|value| !value.is_empty())?;
    name.rsplit_once(delimiter).map(|(parent, _)| parent)
}

fn mailbox_leaf_name<'a>(name: &'a str, delimiter: Option<&str>) -> &'a str {
    mailbox_parent_name(name, delimiter)
        .and_then(|parent| name.get(parent.len() + delimiter.unwrap_or_default().len()..))
        .filter(|value| !value.is_empty())
        .unwrap_or(name)
}

fn join_display_mailbox_path(
    parent: Option<&str>,
    delimiter: Option<&str>,
    leaf_name: &str,
) -> CommandResult<String> {
    match parent {
        Some(parent) => {
            let delimiter = delimiter
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CommandError::new("mailbox.hierarchy_unsupported"))?;
            Ok(format!("{parent}{delimiter}{leaf_name}"))
        }
        None => Ok(leaf_name.to_owned()),
    }
}

#[cfg(test)]
mod mailbox_operation_tests {
    use super::{
        join_display_mailbox_path, mailbox_leaf_name, mailbox_parent_name, validate_mailbox_leaf,
    };

    #[test]
    fn validates_leaf_names_and_preserves_server_delimiters() {
        assert_eq!(
            validate_mailbox_leaf("  2026  ", Some("/")).unwrap(),
            "2026"
        );
        assert_eq!(
            validate_mailbox_leaf("A/B", Some("/")).unwrap_err().code,
            "mailbox.name_invalid"
        );
        assert_eq!(
            join_display_mailbox_path(Some("Projects"), Some("/"), "2026").unwrap(),
            "Projects/2026"
        );
    }

    #[test]
    fn separates_parent_and_leaf_without_guessing_a_delimiter() {
        assert_eq!(
            mailbox_parent_name("Projects/2026", Some("/")),
            Some("Projects")
        );
        assert_eq!(mailbox_leaf_name("Projects/2026", Some("/")), "2026");
        assert_eq!(mailbox_parent_name("News/2026", Some(".")), None);
        assert_eq!(mailbox_leaf_name("News/2026", Some(".")), "News/2026");
    }
}

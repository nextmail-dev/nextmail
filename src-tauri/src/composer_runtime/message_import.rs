use super::*;
use crate::core::MailboxRole;

impl ComposerRuntime {
    pub async fn open_remote_draft(
        &self,
        account_id: &str,
        message_id: &str,
        mailbox_id: Option<&str>,
    ) -> CommandResult<()> {
        let account = self.service.account_record(account_id)?;
        let mailbox = match mailbox_id {
            Some(value) => value.to_owned(),
            None => self
                .repository()
                .await?
                .mailbox_roles()
                .mailbox_for_role(&account.data_slot_id, MailboxRole::Drafts)
                .await?
                .map(|(id, _)| id)
                .ok_or_else(|| CommandError::new("draft.source_not_editable"))?,
        };
        self.open_imported_message(account_id, &mailbox, message_id, MailboxRole::Drafts)
            .await
    }

    pub async fn edit_sent_message(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_id: &str,
    ) -> CommandResult<()> {
        self.open_imported_message(account_id, mailbox_id, message_id, MailboxRole::Sent)
            .await
    }

    async fn open_imported_message(
        &self,
        account_id: &str,
        mailbox_id: &str,
        message_id: &str,
        source_role: MailboxRole,
    ) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        drafts
            .ensure_import_source(&account.data_slot_id, mailbox_id, message_id, &source_role)
            .await?;
        if source_role == MailboxRole::Drafts {
            if let Some(existing) = drafts
                .existing_imported_draft(account_id, &account.data_slot_id, message_id)
                .await?
            {
                return self.show_composer_window(account_id, &existing.id).await;
            }
        }
        let raw = self.mail.ensure_raw_message(account_id, message_id).await?;
        let mut imported =
            tokio::task::spawn_blocking(move || crate::protocols::parse_editable_message(&raw))
                .await
                .map_err(|error| {
                    crate::diagnostics::command_error("message.mime_parse_failed", false, &error)
                })??;
        if imported.source.recipients.bcc.is_empty() {
            if let Some(bcc) = drafts
                .locally_sent_bcc(&account.data_slot_id, message_id)
                .await?
            {
                imported.source.recipients.bcc = bcc;
            }
        }
        let content = compose_imported_draft(&imported.source)?;
        validate_content(&content)?;
        let draft = drafts
            .persist_imported_draft(PersistImportedDraftRequest {
                account_id,
                account_slot_id: &account.data_slot_id,
                message_id,
                mailbox_id,
                source_role,
                source: &imported.source,
                content: &content,
                attachments: &imported.attachments,
            })
            .await?;
        if let Err(error) = self.show_composer_window(account_id, &draft.id).await {
            if let Err(cleanup) = drafts
                .discard_empty_draft(&account.data_slot_id, &draft.id)
                .await
            {
                tracing::warn!(code = %cleanup.code, "failed imported composer cleanup");
            }
            return Err(error);
        }
        Ok(())
    }
}

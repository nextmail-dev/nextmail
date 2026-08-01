use std::{
    collections::HashSet,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use crate::{
    adapters::send_raw_smtp,
    core::{CommandError, CommandResult, MailboxRole, MessageAddress, SendJobSummary},
    protocols::{build_outgoing_message, OutgoingAttachment},
    storage::ClaimedSendJob,
};
use lettre::{address::Envelope, Address};
use tauri::Emitter;

use super::{
    add_draft_identity_headers, add_threading_headers, envelope_recipients, nonempty,
    select_ready_account, unix_timestamp, validate_content, validate_recipient_fields,
    ComposerRuntime, SendJobChangedEvent,
};

impl ComposerRuntime {
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::AcqRel) {
            self.wake_worker.notify_one();
            return;
        }
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let repository = match runtime.repository().await {
                Ok(repository) => repository,
                Err(error) => {
                    tracing::error!(code = %error.code, "send worker repository open failed");
                    runtime.started.store(false, Ordering::Release);
                    return;
                }
            };
            if let Err(error) = repository.send_jobs().recover_interrupted_send_jobs().await {
                tracing::warn!(code = %error.code, "interrupted send job recovery failed");
            }
            let mut active_accounts = HashSet::new();
            let mut workers = tokio::task::JoinSet::new();
            let mut fair_cursor = 0_usize;
            loop {
                while active_accounts.len() < 2 {
                    let slots = match repository.send_jobs().ready_send_account_slots().await {
                        Ok(slots) => slots,
                        Err(error) => {
                            tracing::warn!(code = %error.code, "ready send account query failed");
                            break;
                        }
                    };
                    if slots.is_empty() {
                        break;
                    }
                    let Some(slot) =
                        select_ready_account(&slots, &active_accounts, &mut fair_cursor)
                    else {
                        break;
                    };
                    let job = match repository
                        .send_jobs()
                        .claim_next_send_job_for_account(&slot)
                        .await
                    {
                        Ok(Some(job)) => job,
                        Ok(None) => break,
                        Err(error) => {
                            tracing::warn!(
                                account_slot_id = %slot,
                                code = %error.code,
                                "send job claim failed"
                            );
                            break;
                        }
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
                        match completed {
                            Some(Ok(slot)) => {
                                active_accounts.remove(&slot);
                            }
                            Some(Err(error)) => {
                                tracing::error!(?error, "send worker task failed");
                            }
                            None => {}
                        }
                    }
                    _ = runtime.wake_worker.notified() => {},
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                }
            }
        });
    }

    async fn process_send_job(self: &Arc<Self>, job: ClaimedSendJob) {
        let repository = match self.repository().await {
            Ok(repository) => repository,
            Err(error) => {
                tracing::error!(
                    job_id = %job.id,
                    code = %error.code,
                    "send job repository unavailable"
                );
                return;
            }
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
                    .mailbox_roles()
                    .mailbox_for_role(&job.account_slot_id, MailboxRole::Sent)
                    .await
                    .ok()
                    .flatten()
                    .map(|(id, _)| id);
                if let Err(error) = repository
                    .send_jobs()
                    .complete_send_job_and_queue_sent(&job.id, sent_mailbox.as_deref())
                    .await
                {
                    tracing::error!(
                        job_id = %job.id,
                        code = %error.code,
                        "send job completion persistence failed"
                    );
                }
                self.mail
                    .request_pending_operations_by_slot(&job.account_slot_id);
            }
            Err(error) if error.retryable && job.attempt_count < 3 => {
                self.mail
                    .report_account_error_by_slot(&job.account_slot_id, &error);
                let delay = 5_i64.saturating_mul(1_i64 << (job.attempt_count - 1));
                if let Err(persist_error) = repository
                    .send_jobs()
                    .defer_send_job(&job.id, &error.code, unix_timestamp().saturating_add(delay))
                    .await
                {
                    tracing::error!(
                        job_id = %job.id,
                        code = %persist_error.code,
                        "send job retry persistence failed"
                    );
                }
            }
            Err(error) => {
                self.mail
                    .report_account_error_by_slot(&job.account_slot_id, &error);
                if let Err(persist_error) = repository
                    .send_jobs()
                    .fail_send_job(&job.id, &error.code)
                    .await
                {
                    tracing::error!(
                        job_id = %job.id,
                        code = %persist_error.code,
                        "send job failure persistence failed"
                    );
                }
            }
        }
        self.emit_job(&job.id, &job.account_slot_id).await;
    }

    pub async fn queue_remote_draft(&self, account_id: &str, draft_id: &str) -> CommandResult<()> {
        self.mail.ensure_account_writable(account_id)?;
        let account = self.service.account_record(account_id)?;
        let repository = self.repository().await?;
        let drafts = repository.drafts();
        let draft = drafts
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        let Some((drafts_mailbox_id, _)) = repository
            .mailbox_roles()
            .mailbox_for_role(&account.data_slot_id, MailboxRole::Drafts)
            .await?
        else {
            return Err(CommandError::new("draft.mailbox_mapping_missing"));
        };
        let mut attachments = Vec::new();
        for stored in drafts
            .draft_attachments(&account.data_slot_id, draft_id)
            .await?
        {
            attachments.push(OutgoingAttachment {
                file_name: stored.summary.file_name,
                content_type: stored.summary.content_type,
                bytes: drafts.attachment_bytes(&stored.content_hash).await?,
                content_id: stored.summary.content_id,
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
        let threading = drafts
            .draft_threading_headers(&account.data_slot_id, draft_id)
            .await?;
        let raw = add_threading_headers(raw, &threading)?;
        let raw = add_draft_identity_headers(raw, draft_id, draft.revision)?;
        let hash = repository.send_jobs().write_send_mime(&raw).await?;
        repository
            .operations()
            .queue_draft_append(
                &account.data_slot_id,
                &drafts_mailbox_id,
                draft_id,
                &hash,
                draft.revision,
            )
            .await?;
        self.mail.request_pending_operations(account_id);
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
        let drafts = repository.drafts();
        let draft = drafts
            .get_draft(account_id, &account.data_slot_id, draft_id)
            .await?;
        validate_recipient_fields(&draft.recipients, true)?;
        validate_content(&draft.content)?;
        let mut attachments = Vec::new();
        for stored in drafts
            .draft_attachments(&account.data_slot_id, draft_id)
            .await?
        {
            attachments.push(OutgoingAttachment {
                file_name: stored.summary.file_name,
                content_type: stored.summary.content_type,
                bytes: drafts.attachment_bytes(&stored.content_hash).await?,
                content_id: stored.summary.content_id,
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
        let threading = drafts
            .draft_threading_headers(&account.data_slot_id, draft_id)
            .await?;
        let raw = add_threading_headers(raw, &threading)?;
        let send_jobs = repository.send_jobs();
        let hash = send_jobs.write_send_mime(&raw).await?;
        let envelope = envelope_recipients(&draft.recipients);
        let job = send_jobs
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
        let send_jobs = repository.send_jobs();
        send_jobs
            .get_send_job(account_id, &account.data_slot_id, job_id)
            .await?;
        send_jobs
            .retry_send_job(&account.data_slot_id, job_id)
            .await?;
        self.wake_worker.notify_one();
        send_jobs
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
            .send_jobs()
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
        let raw = self
            .repository()
            .await?
            .send_jobs()
            .read_send_mime(mime_hash)
            .await?;
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
            .send_jobs()
            .get_send_job(&account.id, account_slot_id, job_id)
            .await
        else {
            return;
        };
        let subject = repository
            .drafts()
            .get_draft(&account.id, account_slot_id, &job.draft_id)
            .await
            .map(|draft| draft.subject)
            .unwrap_or_default();
        if let Err(error) = self.app.emit(
            "send-job-changed",
            SendJobChangedEvent {
                account_id: job.account_id,
                draft_id: job.draft_id,
                job_id: job.id,
                status: job.status,
                subject,
                revision: job.revision,
            },
        ) {
            tracing::warn!(%job_id, %account_slot_id, ?error, "send job event failed");
        }
    }
}

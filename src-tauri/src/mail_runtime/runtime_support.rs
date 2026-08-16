use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use crate::core::{
    MessageListItem, NewMailCandidate, SyncInterval, SyncNotice, SyncObserver, SyncPhase,
};
use serde::Serialize;
use tauri::Emitter;
use tokio::sync::Notify;

use super::MailRuntime;

pub(super) struct RuntimeObserver<'a> {
    pub(super) runtime: &'a MailRuntime,
    pub(super) account_id: String,
    pub(super) generation: u64,
    pub(super) report_progress: bool,
    pub(super) candidates: Mutex<Vec<PendingNewMailCandidate>>,
    pub(super) contacts_changed: AtomicBool,
}

impl RuntimeObserver<'_> {
    pub(super) fn take_candidates(&self) -> Vec<PendingNewMailCandidate> {
        self.candidates
            .lock()
            .map(|mut candidates| std::mem::take(&mut *candidates))
            .unwrap_or_default()
    }

    pub(super) fn contacts_changed(&self) -> bool {
        self.contacts_changed.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub(super) struct PendingNewMailCandidate {
    pub(super) candidate: NewMailCandidate,
    pub(super) default_enabled: bool,
}

pub(super) fn eligible_new_mail_candidates(
    preferences: &crate::core::NotificationPreferences,
    account_id: &str,
    candidates: Vec<PendingNewMailCandidate>,
) -> Vec<NewMailCandidate> {
    if !preferences.enabled || !preferences.account_enabled(account_id) {
        return Vec::new();
    }
    let mut emitted = std::collections::HashSet::new();
    candidates
        .into_iter()
        .filter_map(|pending| {
            let candidate = pending.candidate;
            if emitted.insert((candidate.mailbox_id.clone(), candidate.message_id.clone()))
                && preferences.folder_enabled(
                    account_id,
                    &candidate.mailbox_id,
                    pending.default_enabled,
                )
            {
                Some(candidate)
            } else {
                None
            }
        })
        .collect()
}

pub(super) struct AccountSupervisor {
    pub(super) account_id: String,
    pub(super) generation: u64,
    pub(super) sync_wake: Notify,
    pub(super) operations_wake: Notify,
    pub(super) manual_sync_requested: AtomicBool,
    pub(super) pending_operations_requested: AtomicBool,
    pub(super) stopped: AtomicBool,
}

impl AccountSupervisor {
    pub(super) fn new(account_id: &str, generation: u64) -> Self {
        Self {
            account_id: account_id.to_owned(),
            generation,
            sync_wake: Notify::new(),
            operations_wake: Notify::new(),
            manual_sync_requested: AtomicBool::new(false),
            pending_operations_requested: AtomicBool::new(true),
            stopped: AtomicBool::new(false),
        }
    }
}

impl SyncObserver for RuntimeObserver<'_> {
    fn notify(&self, notice: SyncNotice) {
        if !self
            .runtime
            .is_current_supervisor(&self.account_id, self.generation)
        {
            return;
        }
        match notice {
            SyncNotice::Folders {
                completed,
                total,
                mailbox_name,
            } if self.report_progress => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Folders,
                completed,
                total,
                mailbox_name,
                None,
            ),
            SyncNotice::Summaries {
                completed,
                total,
                mailbox_name,
            } if self.report_progress => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Summaries,
                completed,
                total,
                Some(mailbox_name),
                None,
            ),
            SyncNotice::Bodies {
                completed,
                total,
                mailbox_name,
            } if self.report_progress => self.runtime.update_progress(
                &self.account_id,
                SyncPhase::Bodies,
                completed,
                total,
                Some(mailbox_name),
                None,
            ),
            SyncNotice::Folders { .. }
            | SyncNotice::Summaries { .. }
            | SyncNotice::Bodies { .. } => {}
            SyncNotice::MailboxChanged {
                mailbox_id,
                revision,
            } => {
                if let Err(error) = self.runtime.app.emit(
                    "mailbox-changed",
                    MailboxChangedEvent {
                        account_id: self.account_id.clone(),
                        mailbox_id: mailbox_id.clone(),
                        revision,
                    },
                ) {
                    tracing::warn!(
                        account_id = %self.account_id,
                        %mailbox_id,
                        ?error,
                        "mailbox event failed"
                    );
                }
            }
            SyncNotice::MessageArrived { mailbox_id, item } => {
                if let Err(error) = self.runtime.app.emit(
                    "message-arrived",
                    MessageArrivedEvent {
                        account_id: self.account_id.clone(),
                        mailbox_id: mailbox_id.clone(),
                        item,
                    },
                ) {
                    tracing::warn!(
                        account_id = %self.account_id,
                        %mailbox_id,
                        ?error,
                        "message arrived event failed"
                    );
                }
            }
            SyncNotice::ContactsChanged => {
                self.contacts_changed.store(true, Ordering::Release);
            }
            SyncNotice::NewMessageCandidate {
                mailbox_id,
                message_id,
                sender_name,
                sender_email,
                subject,
                default_enabled,
            } => {
                if let Ok(mut candidates) = self.candidates.lock() {
                    candidates.push(PendingNewMailCandidate {
                        candidate: NewMailCandidate {
                            account_id: self.account_id.clone(),
                            mailbox_id,
                            message_id,
                            sender_name,
                            sender_email,
                            subject,
                        },
                        default_enabled,
                    });
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SupervisorWake {
    Requested,
    Periodic,
}

pub(super) async fn wait_for_supervisor(
    supervisor: &AccountSupervisor,
    interval: &SyncInterval,
) -> SupervisorWake {
    match interval.minutes() {
        Some(minutes) => tokio::select! {
            _ = supervisor.sync_wake.notified() => SupervisorWake::Requested,
            _ = tokio::time::sleep(Duration::from_secs(minutes * 60)) => SupervisorWake::Periodic,
        },
        None => {
            supervisor.sync_wake.notified().await;
            SupervisorWake::Requested
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MailboxChangedEvent {
    pub(super) account_id: String,
    pub(super) mailbox_id: String,
    pub(super) revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MessageArrivedEvent {
    pub(super) account_id: String,
    pub(super) mailbox_id: String,
    pub(super) item: MessageListItem,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContactsChangedEvent {
    pub(super) account_id: String,
    pub(super) revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SyncFailedEvent {
    pub(super) account_id: String,
    pub(super) code: String,
    pub(super) retryable: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MessageContentChangedEvent {
    pub(super) account_id: String,
    pub(super) message_id: String,
    pub(super) revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MessageBodyProgressEvent {
    pub(super) account_id: String,
    pub(super) message_id: String,
    pub(super) stage: &'static str,
    pub(super) progress: u8,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AttachmentDownloadStartedEvent {
    pub(super) account_id: String,
    pub(super) attachment_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PendingOperationChangedEvent {
    pub(super) account_id: String,
    pub(super) operation_id: String,
    pub(super) status: String,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use tokio::time::{timeout, Duration};

    use super::{wait_for_supervisor, AccountSupervisor, SupervisorWake, SyncInterval};

    #[tokio::test]
    async fn keeps_sync_and_pending_operation_wakes_independent() {
        let supervisor = AccountSupervisor::new("account", 1);
        assert!(supervisor
            .pending_operations_requested
            .swap(false, Ordering::AcqRel));

        supervisor.operations_wake.notify_one();
        timeout(
            Duration::from_millis(30),
            supervisor.operations_wake.notified(),
        )
        .await
        .expect("pending-operation wake");
        assert!(
            timeout(
                Duration::from_millis(30),
                wait_for_supervisor(&supervisor, &SyncInterval::Manual),
            )
            .await
            .is_err(),
            "an operation request must not wake the sync loop"
        );

        supervisor.sync_wake.notify_one();
        let wake = timeout(
            Duration::from_millis(30),
            wait_for_supervisor(&supervisor, &SyncInterval::Manual),
        )
        .await
        .expect("sync wake");
        assert!(wake == SupervisorWake::Requested);
    }
}

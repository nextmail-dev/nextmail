use async_trait::async_trait;

use super::{
    AccountsFile, AppearancePreferences, BootstrapConfig, CommandResult, ConnectionSecurity,
    MailboxRole, MessageAddress, ReadingPreferences, SyncPolicy,
};

pub trait AccountsConfigStore: Send + Sync {
    fn load(&self) -> CommandResult<AccountsFile>;
    fn save(&self, value: &AccountsFile) -> CommandResult<()>;
}

pub trait BootstrapConfigStore: Send + Sync {
    fn load(&self) -> CommandResult<Option<BootstrapConfig>>;
    fn save(&self, value: &BootstrapConfig) -> CommandResult<()>;
}

pub trait AppearancePreferencesStore: Send + Sync {
    fn load(&self) -> CommandResult<AppearancePreferences>;
    fn save(&self, value: &AppearancePreferences) -> CommandResult<()>;
}

pub trait ReadingPreferencesConfigStore: Send + Sync {
    fn load(&self) -> CommandResult<ReadingPreferences>;
    fn save(&self, value: &ReadingPreferences) -> CommandResult<()>;
}

pub trait ExternalLinkOpener: Send + Sync {
    fn open(&self, target: &str) -> CommandResult<()>;
}

#[derive(Clone, Debug)]
pub struct ImapAccountConfig {
    pub account_id: String,
    pub account_slot_id: String,
    pub host: String,
    pub port: u16,
    pub security: ConnectionSecurity,
    pub username: String,
    pub password: String,
    pub sync_policy: SyncPolicy,
    pub download_non_inbox_bodies: bool,
}

#[derive(Clone, Debug)]
pub struct RemoteMailbox {
    pub name: String,
    pub display_name: String,
    pub delimiter: Option<String>,
    pub role: MailboxRole,
    pub selectable: bool,
    pub uid_validity: u32,
    pub uid_next: u32,
    pub total_count: u32,
    pub unread_count: u32,
    pub highest_modseq: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RemoteMessage {
    pub uid: u32,
    pub uid_validity: u32,
    pub subject: String,
    pub from: Vec<MessageAddress>,
    pub to: Vec<MessageAddress>,
    pub cc: Vec<MessageAddress>,
    pub received_at: i64,
    pub preview: String,
    pub unread: bool,
    pub flagged: bool,
    pub size: u64,
    pub message_id: Option<String>,
    pub references: Vec<String>,
    pub in_reply_to: Option<String>,
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub raw: Option<Vec<u8>>,
    pub attachments: Vec<RemoteAttachment>,
    pub remote_images_blocked: bool,
    pub modseq: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RemoteMessageState {
    pub uid: u32,
    pub unread: bool,
    pub flagged: bool,
    pub modseq: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RemoteAttachment {
    pub part_index: u32,
    pub file_name: String,
    pub content_type: String,
    pub size: u64,
    pub content_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StoredMailbox {
    pub id: String,
    pub last_uid: u32,
    pub highest_modseq: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct StoredMessageLocation {
    pub uid: u32,
    pub uid_validity: u32,
}

#[async_trait]
pub trait MailSyncSink: Send + Sync {
    async fn upsert_mailbox(
        &self,
        account_slot_id: &str,
        mailbox: &RemoteMailbox,
    ) -> CommandResult<StoredMailbox>;

    async fn upsert_message(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message: &RemoteMessage,
    ) -> CommandResult<()>;

    async fn complete_mailbox(&self, mailbox_id: &str, last_uid: u32) -> CommandResult<()>;

    async fn pending_body_locations(
        &self,
        mailbox_id: &str,
        received_after: Option<i64>,
    ) -> CommandResult<Vec<StoredMessageLocation>>;

    async fn reconcile_mailbox(
        &self,
        mailbox_id: &str,
        uid_validity: u32,
        highest_modseq: Option<u64>,
        states: &[RemoteMessageState],
    ) -> CommandResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteOperationKind {
    SetRead(bool),
    SetFlagged(bool),
    Copy,
    Move,
    Delete,
}

#[derive(Clone, Debug)]
pub struct RemoteOperation {
    pub kind: RemoteOperationKind,
    pub source_mailbox: String,
    pub destination_mailbox: Option<String>,
    pub uid: u32,
    pub uid_validity: u32,
    pub base_modseq: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct RemoteOperationOutcome {
    pub cleanup_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InboxWatchOutcome {
    Changed,
    Timeout,
    Unsupported,
}

#[derive(Clone, Debug)]
pub enum SyncNotice {
    Folders { completed: u64, total: u64 },
    Summaries { completed: u64, total: u64 },
    Bodies { completed: u64, total: u64 },
    MailboxChanged { mailbox_id: String, revision: u64 },
}

pub trait SyncObserver: Send + Sync {
    fn notify(&self, notice: SyncNotice);
}

#[async_trait]
pub trait ImapSyncProvider: Send + Sync {
    async fn synchronize(
        &self,
        account: &ImapAccountConfig,
        sink: &(dyn MailSyncSink + Send + Sync),
        observer: &(dyn SyncObserver + Send + Sync),
    ) -> CommandResult<()>;

    async fn fetch_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessage>;

    async fn apply_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteOperation,
    ) -> CommandResult<RemoteOperationOutcome>;

    async fn append_message(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        flags: &str,
        raw: &[u8],
    ) -> CommandResult<()>;

    async fn replace_draft(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        draft_id: &str,
        raw: &[u8],
    ) -> CommandResult<RemoteOperationOutcome>;

    async fn wait_for_inbox_change(
        &self,
        account: &ImapAccountConfig,
        timeout: std::time::Duration,
    ) -> CommandResult<InboxWatchOutcome>;
}

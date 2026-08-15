use async_trait::async_trait;

use super::{
    AccountsFile, AppearancePreferences, BootstrapConfig, CommandResult, ConnectionSecurity,
    DesktopPreferences, MailboxRole, MessageAddress, MessageListItem, NotificationPreferences,
    ReadingPreferences,
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

pub trait NotificationPreferencesConfigStore: Send + Sync {
    fn load(&self) -> CommandResult<NotificationPreferences>;
    fn save(&self, value: &NotificationPreferences) -> CommandResult<()>;
}

pub trait ExternalLinkOpener: Send + Sync {
    fn open(&self, target: &str) -> CommandResult<()>;
}

#[derive(Clone, Debug)]
pub struct ImapAccountConfig {
    pub account_id: String,
    pub account_slot_id: String,
    pub download_full_messages: bool,
    pub host: String,
    pub port: u16,
    pub security: ConnectionSecurity,
    pub username: String,
    pub password: String,
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

pub trait DesktopPreferencesConfigStore: Send + Sync {
    fn load(&self) -> CommandResult<DesktopPreferences>;
    fn save(&self, value: &DesktopPreferences) -> CommandResult<()>;
}

#[derive(Clone, Debug)]
pub struct MailboxSyncTarget {
    pub name: String,
    pub display_name: String,
    pub delimiter: Option<String>,
    pub role: MailboxRole,
}

#[derive(Clone, Debug)]
pub struct RemoteMessage {
    pub uid: u32,
    pub uid_validity: u32,
    pub subject: String,
    pub from: Vec<MessageAddress>,
    pub to: Vec<MessageAddress>,
    pub cc: Vec<MessageAddress>,
    pub contact_addresses: Vec<RemoteContactAddress>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactAddressRole {
    From,
    Sender,
    ReplyTo,
    To,
    Cc,
    Bcc,
}

impl ContactAddressRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::Sender => "sender",
            Self::ReplyTo => "reply_to",
            Self::To => "to",
            Self::Cc => "cc",
            Self::Bcc => "bcc",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RemoteContactAddress {
    pub role: ContactAddressRole,
    pub address: MessageAddress,
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
    pub imap_section: Option<String>,
    pub file_name: String,
    pub content_type: String,
    pub size: u64,
    pub content_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RemoteMessageBody {
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub preview: Option<String>,
    pub attachments: Vec<RemoteAttachment>,
    pub remote_images_blocked: bool,
    pub inline_content_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StoredMailbox {
    pub id: String,
    pub last_uid: u32,
    pub highest_modseq: Option<u64>,
    pub notification_baseline_required: bool,
}

#[derive(Clone, Debug)]
pub struct MessageUpsertOutcome {
    pub message_id: String,
    pub is_new_location: bool,
    pub contacts_changed: bool,
}

#[derive(Clone, Debug)]
pub struct StoredMessageLocation {
    pub message_id: String,
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

    /// Inserts the mailbox when it is missing for the account and returns the
    /// new row; an existing row is left untouched and yields `None`. The sync
    /// pre-lists the folder tree with this so the UI can show the structure
    /// before per-folder message sync starts, and it must not overwrite
    /// stored sync metadata (uid_validity, counts) with preliminary values.
    async fn ensure_mailbox(
        &self,
        account_slot_id: &str,
        mailbox: &RemoteMailbox,
    ) -> CommandResult<Option<StoredMailbox>>;

    async fn upsert_message(
        &self,
        account_slot_id: &str,
        mailbox_id: &str,
        message: &RemoteMessage,
    ) -> CommandResult<MessageUpsertOutcome>;

    async fn complete_notification_baseline(&self, account_slot_id: &str) -> CommandResult<()>;

    async fn complete_mailbox(&self, mailbox_id: &str, last_uid: u32) -> CommandResult<()>;

    /// UIDs already stored for `(mailbox_id, uid_validity)`. Used to resume a
    /// partially-completed sync: only UIDs missing from this set are fetched,
    /// so a failed run picks up where it stopped instead of restarting from 1.
    async fn stored_uids(&self, mailbox_id: &str, uid_validity: u32) -> CommandResult<Vec<u32>>;

    async fn pending_body_locations(
        &self,
        mailbox_id: &str,
        received_after: Option<i64>,
    ) -> CommandResult<Vec<StoredMessageLocation>>;

    async fn replace_message_body(
        &self,
        account_slot_id: &str,
        message_id: &str,
        body: &RemoteMessageBody,
    ) -> CommandResult<()>;

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
pub enum RemoteMailboxOperation {
    Create {
        parent_mailbox: Option<String>,
        delimiter: Option<String>,
        leaf_name: String,
    },
    Rename {
        source_mailbox: String,
        destination_parent: Option<String>,
        delimiter: Option<String>,
        leaf_name: String,
    },
    Delete {
        mailbox_name: String,
    },
    MarkAllRead {
        mailbox_name: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteMailboxOperationOutcome {
    pub mailbox_name: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SyncNotice {
    Folders {
        completed: u64,
        total: u64,
        mailbox_name: Option<String>,
    },
    Summaries {
        completed: u64,
        total: u64,
        mailbox_name: String,
    },
    Bodies {
        completed: u64,
        total: u64,
        mailbox_name: String,
    },
    MailboxChanged {
        mailbox_id: String,
        revision: u64,
    },
    MessageArrived {
        mailbox_id: String,
        item: MessageListItem,
    },
    ContactsChanged,
    NewMessageCandidate {
        mailbox_id: String,
        message_id: String,
        sender_name: Option<String>,
        sender_email: String,
        subject: String,
        default_enabled: bool,
    },
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

    async fn synchronize_mailbox(
        &self,
        account: &ImapAccountConfig,
        mailbox: &MailboxSyncTarget,
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

    async fn fetch_message_body(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
    ) -> CommandResult<RemoteMessageBody>;

    async fn fetch_attachment(
        &self,
        account: &ImapAccountConfig,
        mailbox_name: &str,
        uid: u32,
        expected_uid_validity: u32,
        imap_section: &str,
    ) -> CommandResult<Vec<u8>>;

    async fn apply_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteOperation,
    ) -> CommandResult<RemoteOperationOutcome>;

    async fn apply_mailbox_operation(
        &self,
        account: &ImapAccountConfig,
        operation: &RemoteMailboxOperation,
    ) -> CommandResult<RemoteMailboxOperationOutcome>;

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
}

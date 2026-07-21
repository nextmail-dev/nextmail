use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapStage {
    NeedsDataDirectory,
    DataDirectoryMissing,
    NeedsAccount,
    Ready,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatus {
    pub stage: BootstrapStage,
    pub default_data_dir: PathBuf,
    pub configured_data_dir: Option<PathBuf>,
    pub accounts: Vec<AccountSummary>,
    pub last_selected_account_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryValidation {
    pub valid: bool,
    pub can_initialize: bool,
    pub is_existing_dataset: bool,
    pub message_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LanguagePreference {
    #[serde(rename = "zh-CN", alias = "zh-cn")]
    ZhCn,
    #[serde(rename = "en-US", alias = "en-us")]
    EnUs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearancePreferences {
    pub theme: ThemePreference,
    pub accent_color: String,
    pub language: LanguagePreference,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            accent_color: "#2563eb".to_owned(),
            language: LanguagePreference::ZhCn,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct ReadingPreferences {
    pub auto_load_remote_images: bool,
    pub auto_open_downloaded_attachments: bool,
}

impl Default for ReadingPreferences {
    fn default() -> Self {
        Self {
            auto_load_remote_images: false,
            auto_open_downloaded_attachments: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionSecurity {
    None,
    StartTls,
    Tls,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub security: ConnectionSecurity,
    pub username: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDraft {
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub incoming: ServerConfig,
    pub outgoing: ServerConfig,
    pub insecure_acknowledged: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountConnectionDraft {
    pub email: String,
    pub display_name: String,
    pub incoming: ServerConfig,
    pub outgoing: ServerConfig,
    pub insecure_acknowledged: bool,
}

impl AccountConnectionDraft {
    pub fn with_password(&self, password: String) -> AccountDraft {
        AccountDraft {
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            password,
            incoming: self.incoming.clone(),
            outgoing: self.outgoing.clone(),
            insecure_acknowledged: self.insecure_acknowledged,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredAccountConfig {
    pub source: String,
    pub incoming: ServerConfig,
    pub outgoing: ServerConfig,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub imap_capabilities: Vec<String>,
    pub smtp_authenticated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountRuntimeState {
    Starting,
    Ready,
    Syncing,
    Offline,
    Retrying,
    ReauthRequired,
    Removing,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRuntimeSummary {
    pub account_id: String,
    pub state: AccountRuntimeState,
    pub error_code: Option<String>,
    pub retry_at: Option<i64>,
    pub revision: u64,
}

impl AccountRuntimeSummary {
    pub fn stopped(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            state: AccountRuntimeState::Stopped,
            error_code: None,
            retry_at: None,
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRemovalImpact {
    pub editing_drafts: u64,
    pub queued_send_jobs: u64,
    pub pending_operations: u64,
    pub can_remove: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapConfig {
    pub data_dir: PathBuf,
    pub onboarding_completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRecord {
    pub id: String,
    pub data_slot_id: String,
    pub email: String,
    pub display_name: String,
    pub incoming: ServerConfig,
    pub outgoing: ServerConfig,
    pub credential_ref: String,
    pub created_at: i64,
}

impl From<&AccountRecord> for AccountSummary {
    fn from(record: &AccountRecord) -> Self {
        Self {
            id: record.id.clone(),
            email: record.email.clone(),
            display_name: record.display_name.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub accounts: Vec<AccountRecord>,
    #[serde(default)]
    pub last_selected_account_id: Option<String>,
    #[serde(default)]
    pub pending_credential_cleanup: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryMarker {
    pub format_version: u32,
    pub dataset_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncPolicy {
    Days30,
    #[default]
    Days90,
    Days365,
    All,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailboxRole {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Junk,
    Archive,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentAvailability {
    Missing,
    Queued,
    Available,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Idle,
    Connecting,
    Folders,
    Summaries,
    Bodies,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub account_id: String,
    pub phase: SyncPhase,
    pub completed: u64,
    pub total: u64,
    pub error_code: Option<String>,
    pub revision: u64,
}

impl SyncProgress {
    pub fn idle(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            phase: SyncPhase::Idle,
            completed: 0,
            total: 0,
            error_code: None,
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxSummary {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub delimiter: Option<String>,
    pub role: MailboxRole,
    pub selectable: bool,
    pub total_count: u32,
    pub unread_count: u32,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageAddress {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageListItem {
    pub id: String,
    pub mailbox_id: String,
    pub subject: String,
    pub from: Vec<MessageAddress>,
    pub received_at: i64,
    pub preview: String,
    pub unread: bool,
    pub flagged: bool,
    pub has_attachments: bool,
    pub body_availability: ContentAvailability,
    pub pending_operation: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PendingOperationKind {
    SetRead,
    SetFlagged,
    Copy,
    Move,
    Delete,
    AppendSent,
    AppendDraft,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PendingOperationStatus {
    Queued,
    Running,
    RetryWait,
    NeedsReconcile,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOperationSummary {
    pub id: String,
    pub account_id: String,
    pub message_id: Option<String>,
    pub kind: PendingOperationKind,
    pub status: PendingOperationStatus,
    pub attempt_count: u32,
    pub error_code: Option<String>,
    pub cleanup_pending: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageListPage {
    pub items: Vec<MessageListItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSummary {
    pub id: String,
    pub file_name: String,
    pub content_type: String,
    pub size: u64,
    pub availability: ContentAvailability,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    pub id: String,
    pub mailbox_id: String,
    pub subject: String,
    pub from: Vec<MessageAddress>,
    pub to: Vec<MessageAddress>,
    pub cc: Vec<MessageAddress>,
    pub received_at: i64,
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub body_availability: ContentAvailability,
    pub attachments: Vec<AttachmentSummary>,
    pub remote_images_blocked: bool,
    pub revision: u64,
    pub unread: bool,
    pub flagged: bool,
    pub pending_operation: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountManagementDetail {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub incoming_host: String,
    pub incoming_port: u16,
    pub security: ConnectionSecurity,
    pub sync_policy: SyncPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAbout {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftContent {
    pub editor_json: String,
    pub html: String,
    pub plain_text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftRecipientFields {
    pub to: Vec<MessageAddress>,
    pub cc: Vec<MessageAddress>,
    pub bcc: Vec<MessageAddress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Editing,
    Queued,
    Sent,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageComposeAction {
    Reply,
    ReplyAll,
    Forward,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompositionScene {
    New,
    Reply,
    ReplyAll,
    Forward,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageActionSource {
    pub subject: String,
    pub from: Vec<MessageAddress>,
    pub to: Vec<MessageAddress>,
    pub cc: Vec<MessageAddress>,
    pub message_id: Option<String>,
    pub references: Vec<String>,
    pub plain_text: String,
    pub safe_html: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposedMessageActionDraft {
    pub recipients: DraftRecipientFields,
    pub subject: String,
    pub content: DraftContent,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedDraftSource {
    pub recipients: DraftRecipientFields,
    pub subject: String,
    pub plain_text: String,
    pub safe_html: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftAttachmentSummary {
    pub id: String,
    pub file_name: String,
    pub content_type: String,
    pub size: u64,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub preview_data_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftDetail {
    pub id: String,
    pub account_id: String,
    pub status: DraftStatus,
    pub recipients: DraftRecipientFields,
    pub subject: String,
    pub content: DraftContent,
    pub attachments: Vec<DraftAttachmentSummary>,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftListItem {
    pub id: String,
    pub account_id: String,
    pub subject: String,
    pub recipients: Vec<MessageAddress>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerBootstrap {
    pub draft: DraftDetail,
    pub sender: AccountSummary,
    pub templates: Vec<CompositionDefinitionSummary>,
    pub signatures: Vec<CompositionDefinitionSummary>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompositionDefinitionScope {
    Global,
    Account,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionDefinitionSummary {
    pub id: String,
    pub name: String,
    pub scope: CompositionDefinitionScope,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionSceneRule {
    pub scene: CompositionScene,
    pub template_id: Option<String>,
    pub signature_id: Option<String>,
    pub inherited: bool,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionSceneRuleDraft {
    pub scene: CompositionScene,
    pub template_id: Option<String>,
    pub signature_id: Option<String>,
    pub inherit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailTemplateDraft {
    pub name: String,
    pub subject: String,
    pub content: DraftContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailTemplate {
    pub id: String,
    pub scope: CompositionDefinitionScope,
    pub account_id: Option<String>,
    pub name: String,
    pub subject: String,
    pub content: DraftContent,
    pub revision: u64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailSignatureDraft {
    pub name: String,
    pub content: DraftContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailSignature {
    pub id: String,
    pub scope: CompositionDefinitionScope,
    pub account_id: Option<String>,
    pub name: String,
    pub content: DraftContent,
    pub revision: u64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedMailTemplate {
    pub id: String,
    pub subject: String,
    pub content: DraftContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedMailSignature {
    pub id: String,
    pub content: DraftContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SendJobStatus {
    Queued,
    Sending,
    Sent,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendJobSummary {
    pub id: String,
    pub draft_id: String,
    pub account_id: String,
    pub status: SendJobStatus,
    pub attempt_count: u32,
    pub error_code: Option<String>,
    pub revision: u64,
}

#[cfg(test)]
mod tests {
    use super::LanguagePreference;

    #[test]
    fn uses_bcp_47_tags_and_accepts_legacy_lowercase_values() {
        assert_eq!(
            serde_json::to_string(&LanguagePreference::EnUs).unwrap(),
            "\"en-US\""
        );
        assert_eq!(
            serde_json::from_str::<LanguagePreference>("\"zh-cn\"").unwrap(),
            LanguagePreference::ZhCn
        );
    }
}

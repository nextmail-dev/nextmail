export type BootstrapStage =
  | "needs_data_directory"
  | "data_directory_missing"
  | "needs_account"
  | "ready";

export type ThemePreference = "system" | "light" | "dark";
export type LanguagePreference = "zh-CN" | "en-US";
export type ConnectionSecurity = "none" | "start_tls" | "tls";

export interface AccountSummary {
  id: string;
  email: string;
  displayName: string;
}

export type AccountRuntimeState =
  | "starting"
  | "ready"
  | "syncing"
  | "offline"
  | "retrying"
  | "reauth_required"
  | "removing"
  | "stopped";

export interface AccountRuntimeSummary {
  accountId: string;
  state: AccountRuntimeState;
  errorCode: string | null;
  retryAt: number | null;
  revision: number;
}

export interface BootstrapStatus {
  stage: BootstrapStage;
  defaultDataDir: string;
  configuredDataDir: string | null;
  accounts: AccountSummary[];
  lastSelectedAccountId: string | null;
}

export interface DataDirectoryValidation {
  valid: boolean;
  canInitialize: boolean;
  isExistingDataset: boolean;
  messageCode: string;
}

export interface AppearancePreferences {
  theme: ThemePreference;
  accentColor: string;
  language: LanguagePreference;
}

export interface ReadingPreferences {
  autoLoadRemoteImages: boolean;
  autoOpenDownloadedAttachments: boolean;
  autoLoadMoreMessages: boolean;
  autoLoadMoreContacts: boolean;
}

export type NotificationDisplayMode = "stacked" | "replace";

export interface NotificationAccountSetting {
  accountId: string;
  enabled: boolean;
}

export interface NotificationFolderSetting {
  accountId: string;
  mailboxId: string;
  enabled: boolean;
}

export interface NotificationPreferences {
  enabled: boolean;
  displayMode: NotificationDisplayMode;
  maxStacked: number;
  displayDurationSeconds: number;
  accounts: NotificationAccountSetting[];
  folders: NotificationFolderSetting[];
}

export interface NewMailCandidate {
  accountId: string;
  mailboxId: string;
  messageId: string;
  senderName: string | null;
  senderEmail: string;
  subject: string;
}

export interface NewMailNotification extends NewMailCandidate {
  id: string;
  accountName: string;
  accountEmail: string;
}

export interface NotificationNavigationTarget {
  accountId: string;
  mailboxId: string;
  messageId: string | null;
}

export interface ServerConfig {
  host: string;
  port: number;
  security: ConnectionSecurity;
  username: string;
}

export interface AccountDraft {
  email: string;
  displayName: string;
  password: string;
  incoming: ServerConfig;
  outgoing: ServerConfig;
  insecureAcknowledged: boolean;
}

export interface AccountConnectionDraft {
  email: string;
  displayName: string;
  incoming: ServerConfig;
  outgoing: ServerConfig;
  insecureAcknowledged: boolean;
}

export interface AccountRemovalImpact {
  editingDrafts: number;
  queuedSendJobs: number;
  pendingOperations: number;
  canRemove: boolean;
}

export interface DiscoveredAccountConfig {
  source: "built_in" | "dns_srv" | "autoconfig";
  incoming: ServerConfig;
  outgoing: ServerConfig;
}

export interface ConnectionTestResult {
  imapCapabilities: string[];
  smtpAuthenticated: boolean;
}

export interface CommandError {
  code: string;
  params: Record<string, string>;
  retryable: boolean;
}

export interface AppAbout {
  name: string;
  version: string;
}

export type SyncInterval = "manual" | "minutes1" | "minutes5" | "minutes10";
export type MailboxRole = "inbox" | "sent" | "drafts" | "trash" | "junk" | "archive" | "other";
export type ContentAvailability = "missing" | "queued" | "available" | "failed";
export type SyncPhase = "idle" | "connecting" | "folders" | "summaries" | "bodies" | "complete" | "failed";

export interface SyncProgress {
  accountId: string;
  phase: SyncPhase;
  completed: number;
  total: number;
  currentMailboxName: string | null;
  errorCode: string | null;
  revision: number;
}

export interface MailboxSummary {
  id: string;
  accountId: string;
  name: string;
  delimiter: string | null;
  role: MailboxRole;
  selectable: boolean;
  totalCount: number;
  unreadCount: number;
  revision: number;
}

export interface MessageAddress {
  name: string | null;
  email: string;
}

export interface AddressPresentation {
  contactId: string | null;
  name: string | null;
  headerName: string | null;
  email: string;
}

export interface ContactDraft {
  name: string;
  email: string;
}

export interface ContactSummary {
  id: string;
  name: string;
  email: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface ContactListPage {
  items: ContactSummary[];
  nextCursor: string | null;
  total: number;
}

export interface ContactRecentMessage {
  messageId: string;
  mailboxId: string;
  subject: string;
  receivedAt: number;
}

export interface ContactDetail {
  contact: ContactSummary;
  recentMessages: ContactRecentMessage[];
}

export interface MessageListItem {
  id: string;
  mailboxId: string;
  subject: string;
  from: AddressPresentation[];
  receivedAt: number;
  preview: string;
  unread: boolean;
  flagged: boolean;
  hasAttachments: boolean;
  bodyAvailability: ContentAvailability;
  pendingOperation: boolean;
}

export interface MessageListPage {
  items: MessageListItem[];
  nextCursor: string | null;
}

export interface AttachmentSummary {
  id: string;
  fileName: string;
  contentType: string;
  size: number;
  availability: ContentAvailability;
}

export interface MessageDetail {
  id: string;
  mailboxId: string;
  subject: string;
  from: AddressPresentation[];
  to: AddressPresentation[];
  cc: AddressPresentation[];
  receivedAt: number;
  plainText: string | null;
  safeHtml: string | null;
  bodyAvailability: ContentAvailability;
  attachments: AttachmentSummary[];
  remoteImagesBlocked: boolean;
  revision: number;
  unread: boolean;
  flagged: boolean;
  pendingOperation: boolean;
}

export type PendingOperationKind = "set_read" | "set_flagged" | "copy" | "move" | "delete" | "append_sent" | "append_draft";
export type PendingOperationStatus = "queued" | "running" | "retry_wait" | "needs_reconcile" | "succeeded" | "failed";

export interface PendingOperationSummary {
  id: string;
  accountId: string;
  messageId: string | null;
  kind: PendingOperationKind;
  status: PendingOperationStatus;
  attemptCount: number;
  errorCode: string | null;
  cleanupPending: boolean;
}

export interface AccountManagementDetail {
  id: string;
  email: string;
  displayName: string;
  incomingHost: string;
  incomingPort: number;
  security: ConnectionSecurity;
  syncInterval: SyncInterval;
  downloadFullMessages: boolean;
}

export interface MessageBodyProgress {
  accountId: string;
  messageId: string;
  stage: "preparing" | "downloading" | "processing" | "updating" | "complete";
  progress: number;
}

export interface DraftContent {
  editorJson: string;
  html: string;
  plainText: string;
}

export interface DraftRecipientFields {
  to: MessageAddress[];
  cc: MessageAddress[];
  bcc: MessageAddress[];
}

export type DraftStatus = "editing" | "queued" | "sent";
export type MessageComposeAction = "reply" | "reply_all" | "forward";
export type CompositionScene = "new" | "reply" | "reply_all" | "forward";

export interface DraftAttachmentSummary {
  id: string;
  fileName: string;
  contentType: string;
  size: number;
  contentId: string | null;
  isInline: boolean;
  previewDataUrl: string | null;
}

export interface PreparedInlineImage {
  fileName: string;
  contentType: string;
  size: number;
  dataUrl: string;
}

export interface DraftDetail {
  id: string;
  accountId: string;
  status: DraftStatus;
  recipients: DraftRecipientFields;
  subject: string;
  content: DraftContent;
  attachments: DraftAttachmentSummary[];
  revision: number;
}

export interface DraftListItem {
  id: string;
  accountId: string;
  subject: string;
  recipients: MessageAddress[];
  updatedAt: number;
}

export interface ComposerBootstrap {
  draft: DraftDetail;
  sender: AccountSummary;
  templates: CompositionDefinitionSummary[];
  signatures: CompositionDefinitionSummary[];
}

export type CompositionDefinitionScope = "global" | "account";

export interface CompositionDefinitionSummary {
  id: string;
  name: string;
  scope: CompositionDefinitionScope;
}

export interface CompositionSceneRule {
  scene: CompositionScene;
  templateId: string | null;
  signatureId: string | null;
  inherited: boolean;
  revision: number;
}

export interface CompositionSceneRuleDraft {
  scene: CompositionScene;
  templateId: string | null;
  signatureId: string | null;
  inherit: boolean;
}

export interface MailTemplateDraft {
  name: string;
  subject: string;
  recipients: DraftRecipientFields;
  content: DraftContent;
}

export interface MailTemplate extends Omit<MailTemplateDraft, "recipients"> {
  id: string;
  scope: CompositionDefinitionScope;
  accountId: string | null;
  recipients: DraftRecipientFields | null;
  revision: number;
  updatedAt: number;
}

export interface MailSignatureDraft {
  name: string;
  content: DraftContent;
}

export interface MailSignature extends MailSignatureDraft {
  id: string;
  scope: CompositionDefinitionScope;
  accountId: string | null;
  revision: number;
  updatedAt: number;
}

export interface SignaturePreferences {
  defaultSignatureId: string | null;
  autoInsert: boolean;
  inherited: boolean;
  revision: number;
}

export interface SignaturePreferencesDraft {
  defaultSignatureId: string | null;
  autoInsert: boolean;
  inherit: boolean;
}

export interface RenderedMailTemplate {
  id: string;
  subject: string;
  recipients: DraftRecipientFields | null;
  content: DraftContent;
}

export interface RenderedMailSignature {
  id: string;
  content: DraftContent;
}

export type SendJobStatus = "queued" | "sending" | "sent" | "failed";

export interface SendJobSummary {
  id: string;
  draftId: string;
  accountId: string;
  status: SendJobStatus;
  attemptCount: number;
  errorCode: string | null;
  revision: number;
}

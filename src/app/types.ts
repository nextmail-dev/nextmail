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

export interface BootstrapStatus {
  stage: BootstrapStage;
  defaultDataDir: string;
  configuredDataDir: string | null;
  accounts: AccountSummary[];
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

export type SyncPolicy = "days30" | "days90" | "days365" | "all";
export type MailboxRole = "inbox" | "sent" | "drafts" | "trash" | "junk" | "archive" | "other";
export type ContentAvailability = "missing" | "queued" | "available" | "failed";
export type SyncPhase = "idle" | "connecting" | "folders" | "summaries" | "bodies" | "complete" | "failed";

export interface SyncProgress {
  accountId: string;
  phase: SyncPhase;
  completed: number;
  total: number;
  errorCode: string | null;
  revision: number;
}

export interface MailboxSummary {
  id: string;
  accountId: string;
  name: string;
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

export interface MessageListItem {
  id: string;
  mailboxId: string;
  subject: string;
  from: MessageAddress[];
  receivedAt: number;
  preview: string;
  unread: boolean;
  flagged: boolean;
  hasAttachments: boolean;
  bodyAvailability: ContentAvailability;
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
  subject: string;
  from: MessageAddress[];
  to: MessageAddress[];
  cc: MessageAddress[];
  receivedAt: number;
  plainText: string | null;
  safeHtml: string | null;
  bodyAvailability: ContentAvailability;
  attachments: AttachmentSummary[];
  remoteImagesBlocked: boolean;
  revision: number;
}

export interface AccountManagementDetail {
  id: string;
  email: string;
  displayName: string;
  incomingHost: string;
  incomingPort: number;
  security: ConnectionSecurity;
  syncPolicy: SyncPolicy;
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

export interface DraftAttachmentSummary {
  id: string;
  fileName: string;
  contentType: string;
  size: number;
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

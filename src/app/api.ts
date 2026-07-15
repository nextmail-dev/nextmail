import { invoke } from "@tauri-apps/api/core";
import type {
  AccountDraft,
  AccountSummary,
  AppearancePreferences,
  BootstrapStatus,
  CommandError,
  ConnectionTestResult,
  DataDirectoryValidation,
  DiscoveredAccountConfig,
  AppAbout,
  AccountManagementDetail,
  AttachmentSummary,
  MailboxSummary,
  MessageDetail,
  MessageListPage,
  SyncPolicy,
  SyncProgress,
  ComposerBootstrap,
  DraftAttachmentSummary,
  DraftContent,
  DraftDetail,
  DraftListItem,
  DraftRecipientFields,
  SendJobSummary,
  PendingOperationSummary,
  MailboxRole,
  MessageComposeAction,
  ReadingPreferences,
} from "./types";

export const api = {
  getBootstrapStatus: () =>
    invoke<BootstrapStatus>("get_bootstrap_status"),
  validateDataDirectory: (path: string) =>
    invoke<DataDirectoryValidation>("validate_data_directory", { path }),
  initializeDataDirectory: (path: string) =>
    invoke<BootstrapStatus>("initialize_data_directory", { path }),
  getPreferences: () => invoke<AppearancePreferences>("get_preferences"),
  setAppearancePreferences: (preferences: AppearancePreferences) =>
    invoke<AppearancePreferences>("set_appearance_preferences", {
      preferences,
    }),
  getReadingPreferences: () =>
    invoke<ReadingPreferences>("get_reading_preferences"),
  setReadingPreferences: (preferences: ReadingPreferences) =>
    invoke<ReadingPreferences>("set_reading_preferences", { preferences }),
  discoverAccountConfig: (email: string) =>
    invoke<DiscoveredAccountConfig>("discover_account_config", { email }),
  testAccountConnections: (draft: AccountDraft) =>
    invoke<ConnectionTestResult>("test_account_connections", { draft }),
  savePasswordAccount: (draft: AccountDraft) =>
    invoke<AccountSummary>("save_password_account", { draft }),
  completeOnboarding: () =>
    invoke<BootstrapStatus>("complete_onboarding"),
  listAccountSummaries: () =>
    invoke<AccountSummary[]>("list_account_summaries"),
  getAppAbout: () => invoke<AppAbout>("get_app_about"),
  quitApp: () => invoke<void>("quit_app"),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  listMailboxes: (accountId: string) =>
    invoke<MailboxSummary[]>("list_mailboxes", { accountId }),
  listMessages: (accountId: string, mailboxId: string, cursor: string | null, limit = 50) =>
    invoke<MessageListPage>("list_messages", { accountId, mailboxId, cursor, limit }),
  getMessageDetail: (accountId: string, messageId: string, mailboxId: string) =>
    invoke<MessageDetail>("get_message_detail", { accountId, messageId, mailboxId }),
  requestMessageBody: (accountId: string, messageId: string, mailboxId: string) =>
    invoke<MessageDetail>("request_message_body", { accountId, messageId, mailboxId }),
  getSyncProgress: (accountId: string) =>
    invoke<SyncProgress>("get_sync_progress", { accountId }),
  syncNow: (accountId: string) => invoke<void>("sync_now", { accountId }),
  setMessageRead: (accountId: string, mailboxId: string, messageIds: string[], read: boolean) =>
    invoke<void>("set_message_read", { accountId, mailboxId, messageIds, read }),
  setMessageFlagged: (accountId: string, mailboxId: string, messageIds: string[], flagged: boolean) =>
    invoke<void>("set_message_flagged", { accountId, mailboxId, messageIds, flagged }),
  moveMessages: (accountId: string, sourceMailboxId: string, destinationMailboxId: string, messageIds: string[]) =>
    invoke<void>("move_messages", { accountId, sourceMailboxId, destinationMailboxId, messageIds }),
  copyMessages: (accountId: string, sourceMailboxId: string, destinationMailboxId: string, messageIds: string[]) =>
    invoke<void>("copy_messages", { accountId, sourceMailboxId, destinationMailboxId, messageIds }),
  deleteMessages: (accountId: string, sourceMailboxId: string, messageIds: string[]) =>
    invoke<void>("delete_messages", { accountId, sourceMailboxId, messageIds }),
  archiveMessages: (accountId: string, sourceMailboxId: string, messageIds: string[]) =>
    invoke<void>("archive_messages", { accountId, sourceMailboxId, messageIds }),
  setMailboxRoleMapping: (accountId: string, role: MailboxRole, mailboxId: string | null) =>
    invoke<void>("set_mailbox_role_mapping", { accountId, role, mailboxId }),
  listPendingOperationStatus: (accountId: string) =>
    invoke<PendingOperationSummary[]>("list_pending_operation_status", { accountId }),
  retryPendingOperation: (accountId: string, operationId: string) =>
    invoke<void>("retry_pending_operation", { accountId, operationId }),
  getAccountManagementDetail: (accountId: string) =>
    invoke<AccountManagementDetail>("get_account_management_detail", { accountId }),
  setAccountSyncPolicy: (accountId: string, syncPolicy: SyncPolicy) =>
    invoke<SyncPolicy>("set_account_sync_policy", { accountId, syncPolicy }),
  requestRawMessage: (accountId: string, messageId: string) =>
    invoke<string>("request_raw_message", { accountId, messageId }),
  requestAttachment: (accountId: string, attachmentId: string) =>
    invoke<AttachmentSummary>("request_attachment", { accountId, attachmentId }),
  openComposer: (accountId: string) =>
    invoke<string>("open_composer", { accountId }),
  listDrafts: (accountId: string) =>
    invoke<DraftListItem[]>("list_drafts", { accountId }),
  openExistingComposer: (accountId: string, draftId: string) =>
    invoke<void>("open_existing_composer", { accountId, draftId }),
  openRemoteDraft: (accountId: string, messageId: string) =>
    invoke<void>("open_remote_draft", { accountId, messageId }),
  openMessageActionComposer: (accountId: string, messageId: string, action: MessageComposeAction) =>
    invoke<void>("open_message_action_composer", { accountId, messageId, action }),
  getComposerBootstrap: (accountId: string, draftId: string) =>
    invoke<ComposerBootstrap>("get_composer_bootstrap", { accountId, draftId }),
  saveDraft: (
    accountId: string,
    draftId: string,
    recipients: DraftRecipientFields,
    subject: string,
    content: DraftContent,
    expectedRevision: number,
  ) => invoke<DraftDetail>("save_draft", {
    accountId, draftId, recipients, subject, content, expectedRevision,
  }),
  addDraftAttachments: (accountId: string, draftId: string, selectedPaths: string[]) =>
    invoke<DraftAttachmentSummary[]>("add_draft_attachments", {
      accountId, draftId, selectedPaths,
    }),
  removeDraftAttachment: (accountId: string, draftId: string, attachmentId: string) =>
    invoke<void>("remove_draft_attachment", { accountId, draftId, attachmentId }),
  discardEmptyDraft: (accountId: string, draftId: string) =>
    invoke<void>("discard_empty_draft", { accountId, draftId }),
  deleteDraft: (accountId: string, draftId: string) =>
    invoke<void>("delete_draft", { accountId, draftId }),
  queueRemoteDraft: (accountId: string, draftId: string) =>
    invoke<void>("queue_remote_draft", { accountId, draftId }),
  queueDraftSend: (accountId: string, draftId: string) =>
    invoke<SendJobSummary>("queue_draft_send", { accountId, draftId }),
  retrySendJob: (accountId: string, sendJobId: string) =>
    invoke<SendJobSummary>("retry_send_job", { accountId, sendJobId }),
  getSendJob: (accountId: string, sendJobId: string) =>
    invoke<SendJobSummary>("get_send_job", { accountId, sendJobId }),
};

export function normalizeCommandError(error: unknown): CommandError {
  if (typeof error === "object" && error !== null && "code" in error) {
    const candidate = error as Partial<CommandError>;
    return {
      code: String(candidate.code),
      params: candidate.params ?? {},
      retryable: candidate.retryable ?? false,
    };
  }
  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as Partial<CommandError>;
      if (parsed.code) {
        return {
          code: parsed.code,
          params: parsed.params ?? {},
          retryable: parsed.retryable ?? false,
        };
      }
    } catch {
      // Tauri can also reject with a plain string in development builds.
    }
  }
  return { code: "common.unexpected_error", params: {}, retryable: false };
}

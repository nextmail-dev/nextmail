import { invoke } from "@tauri-apps/api/core";
import type {
  AccountDraft,
  AccountConnectionDraft,
  AccountRemovalImpact,
  AccountRuntimeSummary,
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
  MailSignature,
  MailSignatureDraft,
  MailTemplate,
  MailTemplateDraft,
  CompositionSceneRule,
  CompositionSceneRuleDraft,
  RenderedMailSignature,
  RenderedMailTemplate,
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
  addPasswordAccount: (draft: AccountDraft) =>
    invoke<AccountSummary>("add_password_account", { draft }),
  completeOnboarding: () =>
    invoke<BootstrapStatus>("complete_onboarding"),
  startBackgroundServices: () =>
    invoke<void>("start_background_services"),
  listAccountSummaries: () =>
    invoke<AccountSummary[]>("list_account_summaries"),
  getAccountConnectionDraft: (accountId: string) =>
    invoke<AccountConnectionDraft>("get_account_connection_draft", { accountId }),
  updatePasswordAccount: (
    accountId: string,
    draft: AccountConnectionDraft,
    newPassword: string | null,
  ) => invoke<AccountSummary>("update_password_account", { accountId, draft, newPassword }),
  reauthenticatePasswordAccount: (accountId: string, password: string) =>
    invoke<AccountSummary>("reauthenticate_password_account", { accountId, password }),
  getAccountRemovalImpact: (accountId: string) =>
    invoke<AccountRemovalImpact>("get_account_removal_impact", { accountId }),
  removeAccount: (accountId: string) =>
    invoke<void>("remove_account", { accountId }),
  listAccountRuntimeSummaries: () =>
    invoke<AccountRuntimeSummary[]>("list_account_runtime_summaries"),
  getLastSelectedAccount: () =>
    invoke<string | null>("get_last_selected_account"),
  setLastSelectedAccount: (accountId: string) =>
    invoke<string>("set_last_selected_account", { accountId }),
  getAppAbout: () => invoke<AppAbout>("get_app_about"),
  quitApp: () => invoke<void>("quit_app"),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  listMailboxes: (accountId: string) =>
    invoke<MailboxSummary[]>("list_mailboxes", { accountId }),
  listMessages: (accountId: string, mailboxId: string, cursor: string | null, limit = 50) =>
    invoke<MessageListPage>("list_messages", { accountId, mailboxId, cursor, limit }),
  searchMessages: (
    accountId: string,
    mailboxId: string,
    query: string,
    cursor: string | null,
    limit = 50,
  ) => invoke<MessageListPage>("search_messages", {
    accountId, mailboxId, query, cursor, limit,
  }),
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
  openMessageAttachment: (accountId: string, attachmentId: string) =>
    invoke<void>("open_message_attachment", { accountId, attachmentId }),
  saveMessageAttachmentAs: (accountId: string, attachmentId: string) =>
    invoke<boolean>("save_message_attachment_as", { accountId, attachmentId }),
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
  listMailTemplates: (accountId: string | null) =>
    invoke<MailTemplate[]>("list_mail_templates", { accountId }),
  createMailTemplate: (accountId: string | null, draft: MailTemplateDraft) =>
    invoke<MailTemplate>("create_mail_template", { accountId, draft }),
  updateMailTemplate: (
    accountId: string | null,
    templateId: string,
    draft: MailTemplateDraft,
    expectedRevision: number,
  ) => invoke<MailTemplate>("update_mail_template", {
    accountId, templateId, draft, expectedRevision,
  }),
  deleteMailTemplate: (accountId: string | null, templateId: string, expectedRevision: number) =>
    invoke<void>("delete_mail_template", { accountId, templateId, expectedRevision }),
  listMailSignatures: (accountId: string | null) =>
    invoke<MailSignature[]>("list_mail_signatures", { accountId }),
  createMailSignature: (accountId: string | null, draft: MailSignatureDraft) =>
    invoke<MailSignature>("create_mail_signature", { accountId, draft }),
  updateMailSignature: (
    accountId: string | null,
    signatureId: string,
    draft: MailSignatureDraft,
    expectedRevision: number,
  ) => invoke<MailSignature>("update_mail_signature", {
    accountId, signatureId, draft, expectedRevision,
  }),
  deleteMailSignature: (accountId: string | null, signatureId: string, expectedRevision: number) =>
    invoke<void>("delete_mail_signature", { accountId, signatureId, expectedRevision }),
  listCompositionSceneRules: (accountId: string | null) =>
    invoke<CompositionSceneRule[]>("list_composition_scene_rules", { accountId }),
  saveCompositionSceneRule: (
    accountId: string | null,
    draft: CompositionSceneRuleDraft,
    expectedRevision: number,
  ) => invoke<CompositionSceneRule>("save_composition_scene_rule", {
    accountId, draft, expectedRevision,
  }),
  renderMailTemplate: (
    accountId: string,
    templateId: string,
    recipients: DraftRecipientFields,
  ) => invoke<RenderedMailTemplate>("render_mail_template", {
    accountId, templateId, recipients,
  }),
  renderMailSignature: (
    accountId: string,
    signatureId: string,
    recipients: DraftRecipientFields,
  ) => invoke<RenderedMailSignature>("render_mail_signature", {
    accountId, signatureId, recipients,
  }),
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
  addDraftInlineImage: (
    accountId: string,
    draftId: string,
    fileName: string,
    contentType: string,
    contentBase64: string,
  ) => invoke<DraftAttachmentSummary>("add_draft_inline_image", {
    accountId, draftId, fileName, contentType, contentBase64,
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

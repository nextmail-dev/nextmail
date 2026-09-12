import type {
  ComposerBootstrap,
  CompositionSceneRule,
  CompositionSceneRuleDraft,
  DraftAttachmentSummary,
  DraftContent,
  DraftDetail,
  DraftListItem,
  DraftRecipientFields,
  MailSignature,
  MailSignatureDraft,
  MailTemplate,
  MailTemplateDraft,
  MessageComposeAction,
  PreparedInlineImage,
  RenderedMailSignature,
  RenderedMailTemplate,
  SendJobSummary,
  SignaturePreferences,
  SignaturePreferencesDraft,
} from "../types";
import { invoke } from "./invoke";

export const composerApi = {
  openComposer: (accountId: string) => invoke<string>("open_composer", { accountId }),
  listDrafts: (accountId: string) => invoke<DraftListItem[]>("list_drafts", { accountId }),
  openExistingComposer: (accountId: string, draftId: string) =>
    invoke<void>("open_existing_composer", { accountId, draftId }),
  openRemoteDraft: (accountId: string, messageId: string) =>
    invoke<void>("open_remote_draft", { accountId, messageId }),
  openMessageActionComposer: (
    accountId: string,
    messageId: string,
    action: MessageComposeAction,
  ) => invoke<void>("open_message_action_composer", { accountId, messageId, action }),
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
  deleteMailTemplate: (
    accountId: string | null,
    templateId: string,
    expectedRevision: number,
  ) => invoke<void>("delete_mail_template", { accountId, templateId, expectedRevision }),
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
  deleteMailSignature: (
    accountId: string | null,
    signatureId: string,
    expectedRevision: number,
  ) => invoke<void>("delete_mail_signature", { accountId, signatureId, expectedRevision }),
  getSignaturePreferences: (accountId: string | null) =>
    invoke<SignaturePreferences>("get_signature_preferences", { accountId }),
  saveSignaturePreferences: (
    accountId: string | null,
    draft: SignaturePreferencesDraft,
    expectedRevision: number,
  ) => invoke<SignaturePreferences>("save_signature_preferences", {
    accountId, draft, expectedRevision,
  }),
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
  sanitizeRichTextPaste: (html: string) =>
    invoke<string>("sanitize_rich_text_paste", { html }),
  prepareCompositionDefinitionImage: (
    fileName: string,
    contentType: string,
    contentBase64: string,
  ) => invoke<PreparedInlineImage>("prepare_composition_definition_image", {
    fileName, contentType, contentBase64,
  }),
  removeDraftAttachment: (accountId: string, draftId: string, attachmentId: string) =>
    invoke<void>("remove_draft_attachment", { accountId, draftId, attachmentId }),
  discardEmptyDraft: (accountId: string, draftId: string) =>
    invoke<boolean>("discard_empty_draft", { accountId, draftId }),
  discardDraftSession: (accountId: string, draftId: string) =>
    invoke<void>("discard_draft_session", { accountId, draftId }),
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

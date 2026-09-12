import type {
  AttachmentSummary,
  MailboxRole,
  MailboxSummary,
  MessageDetail,
  MessageListPage,
  PendingOperationSummary,
  SyncProgress,
} from "../types";
import { invoke } from "./invoke";

export const mailApi = {
  listMailboxes: (accountId: string) =>
    invoke<MailboxSummary[]>("list_mailboxes", { accountId }),
  createMailbox: (accountId: string, parentMailboxId: string | null, name: string) =>
    invoke<void>("create_mailbox", { accountId, parentMailboxId, name }),
  renameMailbox: (accountId: string, mailboxId: string, name: string) =>
    invoke<void>("rename_mailbox", { accountId, mailboxId, name }),
  moveMailbox: (
    accountId: string,
    mailboxId: string,
    destinationParentMailboxId: string | null,
  ) => invoke<void>("move_mailbox", {
    accountId, mailboxId, destinationParentMailboxId,
  }),
  deleteMailbox: (accountId: string, mailboxId: string) =>
    invoke<void>("delete_mailbox", { accountId, mailboxId }),
  markMailboxAllRead: (accountId: string, mailboxId: string) =>
    invoke<void>("mark_mailbox_all_read", { accountId, mailboxId }),
  setMailboxFavorite: (accountId: string, mailboxId: string, favorite: boolean) =>
    invoke<void>("set_mailbox_favorite", { accountId, mailboxId, favorite }),
  reorderMailboxes: (accountId: string, orderedMailboxIds: string[]) =>
    invoke<void>("reorder_mailboxes", { accountId, orderedMailboxIds }),
  listMessages: (accountId: string, mailboxId: string, cursor: string | null, limit = 50) =>
    invoke<MessageListPage>("list_messages", { accountId, mailboxId, cursor, limit }),
  listUnreadMessages: (accountId: string, cursor: string | null, limit = 50) =>
    invoke<MessageListPage>("list_unread_messages", { accountId, cursor, limit }),
  listStarredMessages: (accountId: string, cursor: string | null, limit = 50) =>
    invoke<MessageListPage>("list_starred_messages", { accountId, cursor, limit }),
  searchMessages: (
    accountId: string,
    mailboxId: string | null,
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
  setMessageRead: (
    accountId: string,
    mailboxId: string,
    messageIds: string[],
    read: boolean,
  ) => invoke<void>("set_message_read", { accountId, mailboxId, messageIds, read }),
  setMessageFlagged: (
    accountId: string,
    mailboxId: string,
    messageIds: string[],
    flagged: boolean,
  ) => invoke<void>("set_message_flagged", { accountId, mailboxId, messageIds, flagged }),
  moveMessages: (
    accountId: string,
    sourceMailboxId: string,
    destinationMailboxId: string,
    messageIds: string[],
  ) => invoke<void>("move_messages", {
    accountId, sourceMailboxId, destinationMailboxId, messageIds,
  }),
  copyMessages: (
    accountId: string,
    sourceMailboxId: string,
    destinationMailboxId: string,
    messageIds: string[],
  ) => invoke<void>("copy_messages", {
    accountId, sourceMailboxId, destinationMailboxId, messageIds,
  }),
  deleteMessages: (accountId: string, sourceMailboxId: string, messageIds: string[]) =>
    invoke<void>("delete_messages", { accountId, sourceMailboxId, messageIds }),
  archiveMessages: (accountId: string, sourceMailboxId: string, messageIds: string[]) =>
    invoke<void>("archive_messages", { accountId, sourceMailboxId, messageIds }),
  setMailboxRoleMapping: (
    accountId: string,
    role: MailboxRole,
    mailboxId: string | null,
  ) => invoke<void>("set_mailbox_role_mapping", { accountId, role, mailboxId }),
  listPendingOperationStatus: (accountId: string) =>
    invoke<PendingOperationSummary[]>("list_pending_operation_status", { accountId }),
  retryPendingOperation: (accountId: string, operationId: string) =>
    invoke<void>("retry_pending_operation", { accountId, operationId }),
  requestRawMessage: (accountId: string, messageId: string) =>
    invoke<string>("request_raw_message", { accountId, messageId }),
  saveMessageAs: (accountId: string, messageId: string) =>
    invoke<boolean>("save_message_as", { accountId, messageId }),
  requestAttachment: (accountId: string, attachmentId: string) =>
    invoke<AttachmentSummary>("request_attachment", { accountId, attachmentId }),
  openMessageAttachment: (accountId: string, attachmentId: string) =>
    invoke<void>("open_message_attachment", { accountId, attachmentId }),
  revealMessageAttachment: (accountId: string, attachmentId: string) =>
    invoke<void>("reveal_message_attachment", { accountId, attachmentId }),
  saveMessageAttachmentAs: (accountId: string, attachmentId: string) =>
    invoke<boolean>("save_message_attachment_as", { accountId, attachmentId }),
};

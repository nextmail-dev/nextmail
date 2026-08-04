export const mailQueryKeys = {
  accounts: ["accounts"] as const,
  accountRuntimes: ["account-runtimes"] as const,
  mailboxes: (accountId: string) => ["mailboxes", accountId] as const,
  messagesForAccount: (accountId: string) => ["messages", accountId] as const,
  messagesForMailbox: (accountId: string, mailboxId: string) =>
    ["messages", accountId, mailboxId] as const,
  messageSearch: (accountId: string, mailboxId: string, query: string) =>
    ["messages", accountId, mailboxId, "search", query] as const,
  syncProgress: (accountId: string) => ["sync-progress", accountId] as const,
  drafts: (accountId: string) => ["drafts", accountId] as const,
  pendingOperations: (accountId: string) => ["pending-operations", accountId] as const,
  contactsForAccount: (accountId: string) => ["contacts", accountId] as const,
  contactList: (accountId: string, query: string) => ["contacts", accountId, "list", query] as const,
  contactDetail: (accountId: string, contactId: string) => ["contacts", accountId, "detail", contactId] as const,
  contactSuggestions: (accountId: string, query: string) => ["contacts", accountId, "suggestions", query] as const,
  contactAddresses: (accountId: string, emails: string[]) => ["contacts", accountId, "addresses", ...emails] as const,
};

export const messageQueryKeys = {
  account: (accountId: string) => ["message", accountId] as const,
  detail: (accountId: string, mailboxId: string, messageId: string) =>
    ["message", accountId, mailboxId, messageId] as const,
};

export const messageQueryKeys = {
  account: (accountId: string) => ["message", accountId] as const,
  detail: (accountId: string, mailboxId: string, messageId: string) =>
    ["message", accountId, mailboxId, messageId] as const,
};

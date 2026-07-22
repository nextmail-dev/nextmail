export function nextMessageIdAfterRemoval(messageIds: string[], removedMessageId: string) {
  const index = messageIds.indexOf(removedMessageId);
  if (index < 0) return "";
  return messageIds[index + 1] ?? messageIds[index - 1] ?? "";
}

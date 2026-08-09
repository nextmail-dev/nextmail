export function nextMessageIdAfterRemoval(messageIds: string[], removedMessageId: string) {
  const index = messageIds.indexOf(removedMessageId);
  if (index < 0) return "";
  return messageIds[index + 1] ?? messageIds[index - 1] ?? "";
}

export function nextMessageIdAfterRemovals(
  messageIds: string[],
  removedMessageIds: string[],
  selectedMessageId: string,
) {
  const removed = new Set(removedMessageIds);
  if (!removed.has(selectedMessageId)) return selectedMessageId;
  const selectedIndex = messageIds.indexOf(selectedMessageId);
  if (selectedIndex < 0) return "";
  for (let index = selectedIndex + 1; index < messageIds.length; index += 1) {
    if (!removed.has(messageIds[index])) return messageIds[index];
  }
  for (let index = selectedIndex - 1; index >= 0; index -= 1) {
    if (!removed.has(messageIds[index])) return messageIds[index];
  }
  return "";
}

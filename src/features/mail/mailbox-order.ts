import type { MailboxHierarchyItem } from "./MailboxPane";

export type MailboxDropPosition = "before" | "after";

export function reorderMailboxHierarchy(
  items: MailboxHierarchyItem[],
  sourceId: string,
  targetId: string,
  position: MailboxDropPosition,
): string[] | null {
  if (sourceId === targetId) return null;
  const sourceIndex = items.findIndex(({ mailbox }) => mailbox.id === sourceId);
  const targetIndex = items.findIndex(({ mailbox }) => mailbox.id === targetId);
  if (sourceIndex < 0 || targetIndex < 0) return null;
  const source = items[sourceIndex];
  const target = items[targetIndex];
  const sourceParent = source.ancestorIds[source.ancestorIds.length - 1] ?? null;
  const targetParent = target.ancestorIds[target.ancestorIds.length - 1] ?? null;
  if (sourceParent !== targetParent) return null;

  const sourceEnd = subtreeEnd(items, sourceIndex);
  const sourceChunk = items.slice(sourceIndex, sourceEnd);
  const remaining = [...items.slice(0, sourceIndex), ...items.slice(sourceEnd)];
  const remainingTargetIndex = remaining.findIndex(({ mailbox }) => mailbox.id === targetId);
  if (remainingTargetIndex < 0) return null;
  const insertionIndex = position === "before"
    ? remainingTargetIndex
    : subtreeEnd(remaining, remainingTargetIndex);
  const reordered = [
    ...remaining.slice(0, insertionIndex),
    ...sourceChunk,
    ...remaining.slice(insertionIndex),
  ];
  const nextIds = reordered.map(({ mailbox }) => mailbox.id);
  const currentIds = items.map(({ mailbox }) => mailbox.id);
  return nextIds.every((id, index) => id === currentIds[index]) ? null : nextIds;
}

function subtreeEnd(items: MailboxHierarchyItem[], index: number) {
  const depth = items[index].depth;
  let end = index + 1;
  while (end < items.length && items[end].depth > depth) end += 1;
  return end;
}

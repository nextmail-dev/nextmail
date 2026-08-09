import { useEffect, useMemo, useRef, useState } from "react";

interface SelectionModifiers {
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export function useListSelection({
  itemIds,
  primaryId,
  resetKey,
  onPrimaryChange,
}: {
  itemIds: string[];
  primaryId: string;
  resetKey: string;
  onPrimaryChange: (id: string) => void;
}) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set(primaryId ? [primaryId] : []));
  const anchorIdRef = useRef(primaryId);
  const itemIdsKey = itemIds.join("\0");

  useEffect(() => {
    const next = new Set(primaryId ? [primaryId] : []);
    setSelectedIds(next);
    anchorIdRef.current = primaryId;
  }, [resetKey]);

  useEffect(() => {
    const visible = new Set(itemIds);
    setSelectedIds((current) => {
      const next = new Set([...current].filter((id) => visible.has(id)));
      if (primaryId && visible.has(primaryId) && !next.has(primaryId)) return new Set([primaryId]);
      if (next.size === current.size) return current;
      return next;
    });
  }, [itemIdsKey, primaryId]);

  const orderedSelectedIds = useMemo(
    () => itemIds.filter((id) => selectedIds.has(id)),
    [itemIdsKey, selectedIds],
  );

  function select(id: string, modifiers: SelectionModifiers) {
    const additive = modifiers.ctrlKey || modifiers.metaKey;
    if (modifiers.shiftKey) {
      const anchorId = anchorIdRef.current || primaryId || id;
      const anchorIndex = itemIds.indexOf(anchorId);
      const targetIndex = itemIds.indexOf(id);
      if (anchorIndex >= 0 && targetIndex >= 0) {
        const [start, end] = anchorIndex <= targetIndex
          ? [anchorIndex, targetIndex]
          : [targetIndex, anchorIndex];
        const next = additive ? new Set(selectedIds) : new Set<string>();
        itemIds.slice(start, end + 1).forEach((itemId) => next.add(itemId));
        setSelectedIds(next);
        onPrimaryChange(id);
        return;
      }
    }

    if (additive) {
      const next = new Set(selectedIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      const nextPrimary = next.has(id) ? id : orderedSelectedIds.find((selectedId) => next.has(selectedId)) ?? "";
      setSelectedIds(next);
      anchorIdRef.current = nextPrimary;
      onPrimaryChange(nextPrimary);
      return;
    }

    setSelectedIds(new Set([id]));
    anchorIdRef.current = id;
    onPrimaryChange(id);
  }

  function selectForContextMenu(id: string) {
    if (selectedIds.has(id)) return;
    setSelectedIds(new Set([id]));
    anchorIdRef.current = id;
    onPrimaryChange(id);
  }

  function clear() {
    setSelectedIds(new Set());
    anchorIdRef.current = "";
  }

  return {
    clear,
    isSelected: (id: string) => selectedIds.has(id),
    orderedSelectedIds,
    select,
    selectForContextMenu,
  };
}

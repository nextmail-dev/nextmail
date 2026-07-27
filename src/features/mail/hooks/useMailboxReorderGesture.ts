import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import type { MailboxDropPosition } from "../mailbox-order";

const LONG_PRESS_MS = 360;
const HOLD_TOLERANCE_PX = 7;

interface DropTarget {
  mailboxId: string;
  position: MailboxDropPosition;
}

interface UseMailboxReorderGestureOptions {
  enabled: boolean;
  canDrop: (sourceId: string, targetId: string) => boolean;
  onDrop: (sourceId: string, targetId: string, position: MailboxDropPosition) => void;
}

interface PendingGesture {
  pointerId: number;
  sourceId: string;
  startX: number;
  startY: number;
  timer: number;
  element: HTMLElement;
}

export function useMailboxReorderGesture({
  enabled,
  canDrop,
  onDrop,
}: UseMailboxReorderGestureOptions) {
  const pendingRef = useRef<PendingGesture | null>(null);
  const draggingIdRef = useRef<string | null>(null);
  const dropTargetRef = useRef<DropTarget | null>(null);
  const suppressClickRef = useRef<string | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);

  const clearPending = useCallback(() => {
    const pending = pendingRef.current;
    if (pending) window.clearTimeout(pending.timer);
    pendingRef.current = null;
  }, []);

  const finish = useCallback((commit: boolean) => {
    clearPending();
    const sourceId = draggingIdRef.current;
    const target = dropTargetRef.current;
    draggingIdRef.current = null;
    dropTargetRef.current = null;
    setDraggingId(null);
    setDropTarget(null);
    if (sourceId) suppressClickRef.current = sourceId;
    if (commit && sourceId && target) {
      onDrop(sourceId, target.mailboxId, target.position);
    }
  }, [clearPending, onDrop]);

  useEffect(() => () => {
    clearPending();
  }, [clearPending]);

  const updateDropTarget = useCallback((clientX: number, clientY: number, sourceId: string) => {
    const element = document.elementFromPoint(clientX, clientY);
    const row = element?.closest<HTMLElement>("[data-mailbox-reorder-id]");
    const targetId = row?.dataset.mailboxReorderId;
    if (!row || !targetId || targetId === sourceId || !canDrop(sourceId, targetId)) {
      dropTargetRef.current = null;
      setDropTarget(null);
      return;
    }
    const bounds = row.getBoundingClientRect();
    const next = {
      mailboxId: targetId,
      position: clientY < bounds.top + bounds.height / 2 ? "before" : "after",
    } satisfies DropTarget;
    dropTargetRef.current = next;
    setDropTarget(next);
  }, [canDrop]);

  const getGestureProps = useCallback((mailboxId: string) => ({
    "data-mailbox-reorder-id": mailboxId,
    onPointerDown: (event: ReactPointerEvent<HTMLElement>) => {
      if (!enabled || event.button !== 0 || event.pointerType === "touch" && !event.isPrimary) return;
      clearPending();
      const element = event.currentTarget;
      const timer = window.setTimeout(() => {
        const pending = pendingRef.current;
        if (pending?.sourceId !== mailboxId) return;
        pending.element.setPointerCapture?.(pending.pointerId);
        draggingIdRef.current = mailboxId;
        setDraggingId(mailboxId);
      }, LONG_PRESS_MS);
      pendingRef.current = {
        pointerId: event.pointerId,
        sourceId: mailboxId,
        startX: event.clientX,
        startY: event.clientY,
        timer,
        element,
      };
    },
    onPointerMove: (event: ReactPointerEvent<HTMLElement>) => {
      const pending = pendingRef.current;
      if (!pending || pending.pointerId !== event.pointerId) return;
      if (!draggingIdRef.current) {
        const distance = Math.hypot(
          event.clientX - pending.startX,
          event.clientY - pending.startY,
        );
        if (distance > HOLD_TOLERANCE_PX) clearPending();
        return;
      }
      event.preventDefault();
      updateDropTarget(event.clientX, event.clientY, mailboxId);
    },
    onPointerUp: (event: ReactPointerEvent<HTMLElement>) => {
      const wasDragging = draggingIdRef.current === mailboxId;
      finish(wasDragging);
      if (wasDragging) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
    onPointerCancel: () => finish(false),
    onContextMenu: clearPending,
    onClickCapture: (event: ReactMouseEvent<HTMLElement>) => {
      if (suppressClickRef.current !== mailboxId) return;
      suppressClickRef.current = null;
      event.preventDefault();
      event.stopPropagation();
    },
  }), [clearPending, enabled, finish, updateDropTarget]);

  return { draggingId, dropTarget, getGestureProps };
}

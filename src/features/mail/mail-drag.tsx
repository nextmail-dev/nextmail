import {
  DndContext, DragOverlay, PointerSensor, pointerWithin,
  useDraggable, useDroppable, useSensor, useSensors,
  type CollisionDetection, type Modifier,
} from "@dnd-kit/core";
import { Slot } from "@radix-ui/react-slot";
import { createContext, forwardRef, useCallback, useContext, useRef, useState, type HTMLAttributes, type ReactElement, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { MailboxSummary } from "@/app/types";
import type { MailboxDropPosition } from "./mailbox-order";

export interface MessageDragItem { id: string; mailboxId: string }
interface FolderDrag {
  kind: "folder"; id: string; parentId: string | null; label: string;
  reorder: (targetId: string, position: MailboxDropPosition) => void;
}
interface MessageDrag {
  kind: "messages"; messages: MessageDragItem[];
  move: (destination: string) => void;
}
type DragPayload = FolderDrag | MessageDrag;
interface FolderTarget {
  mailbox: MailboxSummary; parentId: string | null; tree: boolean; label: string;
}
interface DropTarget { id: string; data: FolderTarget; y: number }
interface FolderMarker { position: MailboxDropPosition; indicatorTop: number }
interface DragState { active: DragPayload | null; target: DropTarget | null; marker: FolderMarker | null }
const MailDragState = createContext<DragState | null>(null);

// Virtual rows already use CSS transforms for their position. Measure their
// visible rectangle, then anchor the compact preview beside the pointer.
const followPointer: Modifier = ({ activatorEvent, activeNodeRect, transform }) => {
  if (!activeNodeRect || !activatorEvent || !("clientX" in activatorEvent)) return transform;
  const pointer = activatorEvent as PointerEvent;
  return { ...transform,
    x: transform.x + pointer.clientX - activeNodeRect.left + 12,
    y: transform.y + pointer.clientY - activeNodeRect.top + 12,
  };
};
const previewModifiers = [followPointer];

export function canDropMail(payload: DragPayload, target: FolderTarget) {
  if (payload.kind === "folder") {
    return target.tree && payload.id !== target.mailbox.id && payload.parentId === target.parentId;
  }
  return target.mailbox.selectable && payload.messages.some((message) => message.mailboxId !== target.mailbox.id);
}

function sensorActivator() {
  const original = PointerSensor.activators[0];
  return [{ ...original, handler: (...args: Parameters<typeof original.handler>) => {
    const event = args[0];
    const target = event.target as HTMLElement;
    if (!event.currentTarget.hasAttribute("data-mail-drag-kind")) return false;
    const button = target.closest("button");
    if (button && !button.hasAttribute("data-mailbox-select-button") && !button.hasAttribute("data-message-select-button")) return false;
    return original.handler(...args);
  } }];
}
class MailPointerSensor extends PointerSensor {
  static activators = sensorActivator();
  constructor(props: ConstructorParameters<typeof PointerSensor>[0]) {
    super({ ...props, options: { ...props.options, activationConstraint:
      props.activeNode.data.current?.kind === "folder"
        ? { delay: 360, tolerance: 7 } : { distance: 6 },
    } });
  }
}

export function folderMarker(node: HTMLElement, y: number): FolderMarker {
  const bounds = node.getBoundingClientRect();
  const position = y < bounds.top + bounds.height / 2 ? "before" : "after";
  let boundary = node;
  if (position === "after") {
    const depth = Number(node.dataset.mailboxDepth ?? 0);
    let sibling = node.nextElementSibling;
    while (sibling instanceof HTMLElement && sibling.dataset.mailboxReorderId
      && Number(sibling.dataset.mailboxDepth ?? 0) > depth) {
      boundary = sibling;
      sibling = sibling.nextElementSibling;
    }
  }
  const edge = position === "before" ? bounds.top : boundary.getBoundingClientRect().bottom;
  return { position, indicatorTop: edge - (node.parentElement?.getBoundingClientRect().top ?? 0) };
}

export function useMailDragState() { return useContext(MailDragState); }

// Standalone panes (including tests) can share the same boundary as the main workspace.
export function MailDragBoundary({ children }: { children: ReactNode }) {
  return useMailDragState() ? children : <MailDragProvider>{children}</MailDragProvider>;
}

export function MailDragProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [state, setState] = useState<DragState>({ active: null, target: null, marker: null });
  const payload = useRef<DragPayload | null>(null);
  const candidate = useRef<(DropTarget & { node: HTMLElement }) | null>(null);
  const sensors = useSensors(
    useSensor(MailPointerSensor),
  );
  const collisionDetection: CollisionDetection = (args) => {
    const source = payload.current ?? args.active.data.current as DragPayload | undefined;
    const containers = args.droppableContainers.filter((container) => {
      const target = container.data.current as FolderTarget | undefined;
      return source && target?.mailbox && canDropMail(source, target);
    });
    const hits = pointerWithin({ ...args, droppableContainers: containers });
    const over = containers.find((container) => container.id === hits[0]?.id);
    candidate.current = over?.node.current && args.pointerCoordinates
      ? { id: String(over.id), data: over.data.current as FolderTarget, y: args.pointerCoordinates.y, node: over.node.current }
      : null;
    return hits;
  };
  function updateFeedback() {
    const active = payload.current;
    const target = candidate.current;
    setState({ active, target, marker: active?.kind === "folder" && target ? folderMarker(target.node, target.y) : null });
  }
  function clear() {
    payload.current = null;
    candidate.current = null;
    setState({ active: null, target: null, marker: null });
  }
  const dragLabel = (active: DragPayload) => active.kind === "folder"
    ? active.label : t("mail.dragMessages", { count: active.messages.length });
  return (
    <DndContext
      sensors={sensors}
      measuring={{ draggable: { measure: (node) => node.getBoundingClientRect() } }}
      collisionDetection={collisionDetection}
      autoScroll={{ canScroll: (element) => Boolean(element.querySelector("[data-mailbox-drop-id]")) }}
      accessibility={{
        screenReaderInstructions: { draggable: t("mail.dragInstructions") },
        announcements: {
          onDragStart: ({ active }) => t("mail.dragStarted", { item: dragLabel(active.data.current as DragPayload) }),
          onDragOver: () => candidate.current ? t("mail.dragTarget", { folder: candidate.current.data.label }) : t("mail.dragNoTarget"),
          onDragEnd: ({ over }) => t(over ? "mail.dragFinished" : "mail.dragCancelled"),
          onDragCancel: () => t("mail.dragCancelled"),
        },
      }}
      onDragStart={({ active }) => {
        payload.current = active.data.current as DragPayload;
        candidate.current = null;
        updateFeedback();
      }}
      onDragMove={updateFeedback}
      onDragOver={updateFeedback}
      onDragCancel={clear}
      onDragEnd={({ over }) => {
        const active = payload.current;
        const target = candidate.current;
        clear();
        if (!active || !target || over?.id !== target.id || !canDropMail(active, target.data)) return;
        if (active.kind === "messages") active.move(target.data.mailbox.id);
        else active.reorder(target.data.mailbox.id, folderMarker(target.node, target.y).position);
      }}
    >
      <MailDragState.Provider value={state}>
        {children}
        {createPortal(<DragOverlay dropAnimation={null} modifiers={previewModifiers} zIndex={50} style={{ width: "max-content", pointerEvents: "none" }}>
          {state.active ? <div data-mail-drag-preview={state.active.kind} className={`pointer-events-none max-w-64 truncate border border-border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-md${state.active.kind === "folder" ? " opacity-50" : ""}`}>{dragLabel(state.active)}</div> : null}
        </DragOverlay>, document.body)}
      </MailDragState.Provider>
    </DndContext>
  );
}

export function MailboxDragTarget({ mailbox, parentId = null, label, tree = false, disabled = false, reorder, children }: {
  mailbox: MailboxSummary; parentId?: string | null; label: string; tree?: boolean;
  disabled?: boolean; reorder?: FolderDrag["reorder"]; children: ReactElement;
}) {
  const state = useMailDragState();
  const id = `${tree ? "folder" : "shortcut"}:${mailbox.id}`;
  const drag = useDraggable({ id, disabled: disabled || !tree || !reorder,
    data: { kind: "folder", id: mailbox.id, parentId, label, reorder } satisfies Omit<FolderDrag, "reorder"> & { reorder?: FolderDrag["reorder"] } });
  const drop = useDroppable({ id, disabled, data: { mailbox, parentId, label, tree } satisfies FolderTarget });
  const highlighted = state?.active?.kind === "messages" && state.target?.id === id;
  const setNodeRef = useCallback((node: HTMLElement | null) => {
    drag.setNodeRef(node);
    drop.setNodeRef(node);
  }, [drag.setNodeRef, drop.setNodeRef]);
  return <Slot
    ref={setNodeRef}
    {...drag.listeners}
    data-mail-drag-kind="folder"
    data-mailbox-drop-id={id}
    className={highlighted ? "!bg-primary/15 !text-primary ring-1 ring-inset ring-primary" : undefined}
  >{children}</Slot>;
}

export const MessageDragSource = forwardRef<HTMLElement, HTMLAttributes<HTMLElement> & {
  source: MessageDragItem; messages: MessageDragItem[]; disabled: boolean; move: MessageDrag["move"]; children: ReactElement;
}>(function MessageDragSource({ source, messages, disabled, move, children, ...props }, ref) {
  const drag = useDraggable({ id: `message:${source.mailboxId}:${source.id}`, disabled,
    data: { kind: "messages", messages, move } satisfies MessageDrag });
  return <Slot ref={drag.setNodeRef} {...drag.listeners} data-mail-drag-kind="messages"><Slot {...props} ref={ref}>{children}</Slot></Slot>;
});

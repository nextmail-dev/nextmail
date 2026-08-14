import {
  Archive,
  ChevronDown,
  ChevronRight,
  FilePenLine,
  Folder,
  FolderInput,
  FolderPlus,
  Inbox,
  MailCheck,
  MailPlus,
  Pencil,
  RefreshCw,
  Send,
  Settings,
  ShieldAlert,
  Trash2,
  UsersRound,
} from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { normalizeCommandError } from "@/app/api";
import { reportCaughtError } from "@/app/errorReporting";
import type { MailboxRole, MailboxSummary, SyncProgress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Progress } from "@/components/ui/progress";
import { LabelText, Text } from "@/components/ui/typography";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  MailboxFolderDialog,
  type MailboxDialogAction,
} from "./MailboxFolderDialog";
import { useMailboxReorderGesture } from "./hooks/useMailboxReorderGesture";
import { reorderMailboxHierarchy } from "./mailbox-order";

interface MailboxPaneProps {
  mailboxes: MailboxSummary[];
  selectedMailboxId: string;
  onSelect: (mailboxId: string) => void;
  progress?: SyncProgress;
  error?: unknown;
  onCompose: () => void;
  contactsSelected?: boolean;
  onSelectContacts?: () => void;
  onReceive: () => void;
  receiving: boolean;
  folderActionBusy?: boolean;
  onCreateFolder?: (parentMailboxId: string | null, name: string) => Promise<void>;
  onRenameFolder?: (mailboxId: string, name: string) => Promise<void>;
  onMoveFolder?: (
    mailboxId: string,
    destinationParentMailboxId: string | null,
  ) => Promise<void>;
  onDeleteFolder?: (mailboxId: string) => Promise<void>;
  onMarkFolderAllRead?: (mailboxId: string) => Promise<void>;
  onReorderFolders?: (orderedMailboxIds: string[]) => Promise<void>;
  onOpenSettings: () => void;
  collapsed?: boolean;
}

export function MailboxPane({
  mailboxes,
  selectedMailboxId,
  onSelect,
  progress,
  error,
  onCompose,
  contactsSelected = false,
  onSelectContacts = () => {},
  onReceive,
  receiving,
  folderActionBusy = false,
  onCreateFolder = async () => {},
  onRenameFolder = async () => {},
  onMoveFolder = async () => {},
  onDeleteFolder = async () => {},
  onMarkFolderAllRead = async () => {},
  onReorderFolders = async () => {},
  onOpenSettings,
  collapsed = false,
}: MailboxPaneProps) {
  const { t } = useTranslation();
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<Set<string>>(() => new Set());
  const [folderDialogAction, setFolderDialogAction] = useState<MailboxDialogAction | null>(null);
  const activeSync = progress && !["idle", "complete", "failed"].includes(progress.phase);
  const percentage = progress?.phase === "summaries" && progress.total
    ? (progress.completed / progress.total) * 100
    : 8;
  const normalizedError = error ? normalizeCommandError(error) : null;
  const mailboxItems = flattenMailboxHierarchy(mailboxes);
  const visibleMailboxItems = mailboxItems.filter((item) =>
    item.ancestorIds.every((ancestorId) => !collapsedFolderIds.has(ancestorId)));
  const handleFolderDrop = useCallback((
    sourceId: string,
    targetId: string,
    position: "before" | "after",
  ) => {
    const orderedMailboxIds = reorderMailboxHierarchy(
      mailboxItems,
      sourceId,
      targetId,
      position,
    );
    if (!orderedMailboxIds) return;
    void onReorderFolders(orderedMailboxIds)
      .catch((error) => reportCaughtError("mailbox.reorder", error));
  }, [mailboxItems, onReorderFolders]);
  const canDropFolder = useCallback((sourceId: string, targetId: string) => {
    const source = mailboxItems.find(({ mailbox }) => mailbox.id === sourceId);
    const target = mailboxItems.find(({ mailbox }) => mailbox.id === targetId);
    if (!source || !target) return false;
    return (source.ancestorIds[source.ancestorIds.length - 1] ?? null)
      === (target.ancestorIds[target.ancestorIds.length - 1] ?? null);
  }, [mailboxItems]);
  const {
    draggingId,
    dropTarget,
    getGestureProps,
  } = useMailboxReorderGesture({
    enabled: !collapsed && !folderActionBusy,
    canDrop: canDropFolder,
    onDrop: handleFolderDrop,
  });

  function toggleFolder(mailboxId: string) {
    setCollapsedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(mailboxId)) next.delete(mailboxId);
      else next.add(mailboxId);
      return next;
    });
  }

  function openFolderDialog(action: MailboxDialogAction) {
    // Let Radix release the ContextMenu body lock before Dialog acquires it.
    setTimeout(() => setFolderDialogAction(action), 0);
  }

  return (
    <Stack className={collapsed ? "min-h-0 flex-1 items-center px-2 py-4" : "min-h-0 flex-1 px-4 py-4"} gap="sm">
      <Inline className={collapsed ? "w-full justify-center gap-0" : "w-full gap-1"}>
        <Button
          className={collapsed ? "mx-auto size-10 flex-none p-0" : "h-10 min-w-0 flex-1 justify-start px-3.5"}
          aria-label={collapsed ? t("mail.compose") : undefined}
          title={collapsed ? t("mail.compose") : undefined}
          onClick={onCompose}
        >
          <MailPlus className="size-[18px] shrink-0" />
          {collapsed ? null : t("mail.compose")}
        </Button>
      </Inline>
      <Button
        variant="ghost"
        className={collapsed
          ? contactsSelected
            ? "mx-auto size-10 flex-none justify-center bg-primary/10 p-0 text-primary hover:bg-primary/15"
            : "mx-auto size-10 flex-none justify-center p-0"
          : contactsSelected
            ? "h-9 w-full flex-none justify-start bg-primary/10 px-3 text-primary shadow-[inset_2px_0_0_var(--primary)] hover:bg-primary/15"
            : "h-9 w-full flex-none justify-start px-3"}
        aria-label={t("contacts.title")}
        title={collapsed ? t("contacts.title") : undefined}
        aria-current={contactsSelected ? "page" : undefined}
        onClick={onSelectContacts}
      >
        <UsersRound className="size-[18px] shrink-0" strokeWidth={1.8} />
        {collapsed ? null : <Text className="text-[length:var(--ui-font-control)] text-inherit">{t("contacts.title")}</Text>}
      </Button>
      {collapsed ? (
        <Inline className="w-full justify-center">
          <Button
            variant="ghost"
            size="icon"
            className="size-9"
            aria-label={t("mail.receive")}
            title={t("mail.receive")}
            disabled={receiving}
            onClick={onReceive}
          >
            <RefreshCw className={receiving ? "animate-spin" : undefined} size={15} />
          </Button>
        </Inline>
      ) : (
        <ContextMenu>
          <ContextMenuTrigger asChild>
            <Inline className="w-full px-2 pt-0.5">
              <LabelText className="min-w-0 flex-1 text-[length:var(--ui-font-caption)] tracking-[0.09em] text-muted-foreground uppercase">
                {t("mail.folders")}
              </LabelText>
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                aria-label={t("mail.receive")}
                title={t("mail.receive")}
                disabled={receiving}
                onClick={onReceive}
              >
                <RefreshCw className={receiving ? "animate-spin" : undefined} size={15} />
              </Button>
            </Inline>
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItem
              disabled={folderActionBusy}
              onSelect={() => openFolderDialog({ kind: "create", parent: null })}
            >
              <FolderPlus size={15} />
              {t("mail.createRootFolder")}
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      )}
      {activeSync && !collapsed ? (
        <Stack className="rounded-lg border border-border/60 bg-card/60 p-2.5" gap="sm">
          <Text className="text-xs">
            {progress.phase === "summaries" && progress.currentMailboxName && progress.total > 0
              ? t("sync.currentFolderProgress", {
                folder: progress.currentMailboxName,
                completed: progress.completed,
                total: progress.total,
              })
              : progress.currentMailboxName
                ? t("sync.currentFolder", { folder: progress.currentMailboxName })
              : t(`sync.${progress.phase}`)}
          </Text>
          <Progress value={percentage} />
        </Stack>
      ) : null}
      {normalizedError && !collapsed ? (
        <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>
      ) : null}
      {mailboxes.length ? (
        <OverlayScrollArea
          className={collapsed ? "min-h-0 w-full flex-1" : "-mr-3 min-h-0 flex-1"}
          contentClassName={collapsed ? "gap-0.5" : "gap-0.5 pr-3"}
          trackClassName="right-0 w-3"
        >
          {visibleMailboxItems.map(({ mailbox, depth, displayName, hasChildren }) => {
            const selected = mailbox.id === selectedMailboxId;
            const label = mailbox.role === "other" ? displayName : t(`mailboxNames.${mailbox.role}`);
            const folderCollapsed = collapsedFolderIds.has(mailbox.id);
            const structureMutable = mailbox.role !== "inbox";
            const dropClass = dropTarget?.mailboxId === mailbox.id
              ? dropTarget.position === "before"
                ? "shadow-[inset_0_2px_0_var(--primary)]"
                : "shadow-[inset_0_-2px_0_var(--primary)]"
              : "";
            const actions = (
              <ContextMenuContent>
                <ContextMenuItem
                  disabled={folderActionBusy || !mailbox.delimiter}
                  onSelect={() => openFolderDialog({ kind: "create", parent: mailbox })}
                >
                  <FolderPlus size={15} />
                  {t("mail.createSubfolder")}
                </ContextMenuItem>
                <ContextMenuItem
                  disabled={folderActionBusy || !structureMutable}
                  onSelect={() => openFolderDialog({
                    kind: "rename",
                    mailbox,
                    displayName,
                  })}
                >
                  <Pencil size={15} />
                  {t("mail.renameFolder")}
                </ContextMenuItem>
                <ContextMenuItem
                  disabled={folderActionBusy || !structureMutable}
                  onSelect={() => openFolderDialog({
                    kind: "move",
                    mailbox,
                    displayName,
                  })}
                >
                  <FolderInput size={15} />
                  {t("mail.moveFolder")}
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                  disabled={folderActionBusy || !mailbox.selectable || mailbox.unreadCount === 0}
                  onSelect={() => void onMarkFolderAllRead(mailbox.id)
                    .catch((error) => reportCaughtError("mailbox.mark-all-read", error))}
                >
                  <MailCheck size={15} />
                  {t("mail.markFolderAllRead")}
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                  className="text-destructive focus:text-destructive"
                  disabled={folderActionBusy || !structureMutable}
                  onSelect={() => openFolderDialog({
                    kind: "delete",
                    mailbox,
                    displayName,
                  })}
                >
                  <Trash2 size={15} />
                  {t("mail.deleteFolder")}
                </ContextMenuItem>
              </ContextMenuContent>
            );
            if (!collapsed) {
              return (
                <ContextMenu key={mailbox.id}>
                  <ContextMenuTrigger asChild>
                    <Inline
                      {...getGestureProps(mailbox.id)}
                      className={`${selected
                        ? "h-9 w-full gap-0 rounded-md bg-primary/10 pr-2 text-primary shadow-[inset_2px_0_0_var(--primary)]"
                        : "h-9 w-full gap-0 rounded-md pr-2 text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"} ${dropClass} ${draggingId === mailbox.id ? "cursor-grabbing opacity-55" : "cursor-default"}`}
                      style={{ paddingInlineStart: `${4 + depth * 16}px` }}
                      aria-grabbed={draggingId === mailbox.id}
                    >
                      {hasChildren ? (
                        <Button
                          variant="ghost"
                          size="icon"
                          className="size-6 shrink-0 rounded-none bg-transparent p-0 hover:bg-transparent hover:text-foreground"
                          aria-label={t(folderCollapsed ? "mail.expandFolder" : "mail.collapseFolder", { folder: label })}
                          aria-expanded={!folderCollapsed}
                          onClick={() => toggleFolder(mailbox.id)}
                        >
                          {folderCollapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
                        </Button>
                      ) : (
                        <Inline className="size-6 shrink-0" aria-hidden="true" />
                      )}
                      <Button
                        variant="ghost"
                        className="h-9 min-w-0 flex-1 justify-start rounded-md bg-transparent px-1.5 text-inherit hover:bg-transparent hover:text-foreground"
                        aria-label={label}
                        onClick={() => onSelect(mailbox.id)}
                      >
                        <MailboxIcon role={mailbox.role} />
                        <Text className="min-w-0 flex-1 truncate text-left text-[length:var(--ui-font-control)] text-inherit">{label}</Text>
                        {mailbox.unreadCount ? (
                          <Text className="min-w-5 text-right text-[11px] leading-none font-semibold text-primary">{mailbox.unreadCount}</Text>
                        ) : null}
                      </Button>
                    </Inline>
                  </ContextMenuTrigger>
                  {actions}
                </ContextMenu>
              );
            }
            return (
              <ContextMenu key={mailbox.id}>
                <ContextMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    className={selected
                      ? "mx-auto size-10 flex-none justify-center bg-primary/10 p-0 text-primary hover:bg-primary/15"
                      : "mx-auto size-10 flex-none justify-center p-0"}
                    aria-label={label}
                    title={label}
                    onClick={() => onSelect(mailbox.id)}
                  >
                    <MailboxIcon role={mailbox.role} />
                  </Button>
                </ContextMenuTrigger>
                {actions}
              </ContextMenu>
            );
          })}
        </OverlayScrollArea>
      ) : (
        <EmptyState className="mt-6 flex-1 items-center p-4 text-center" icon={<Inbox size={21} />} title={t("mail.noFolders")} />
      )}
      <Button
        variant="ghost"
        className={collapsed
          ? "mx-auto mt-auto size-10 flex-none justify-center p-0"
          : "mt-auto h-9 w-full flex-none justify-start px-3"}
        aria-label={t("mail.settings")}
        title={collapsed ? t("mail.settings") : undefined}
        onClick={onOpenSettings}
      >
        <Settings className="size-[18px] shrink-0" strokeWidth={1.8} />
        {collapsed ? null : <Text className="text-[length:var(--ui-font-control)] text-inherit">{t("mail.settings")}</Text>}
      </Button>
      <MailboxFolderDialog
        action={folderDialogAction}
        hierarchy={mailboxItems}
        busy={folderActionBusy}
        onClose={() => setFolderDialogAction(null)}
        onCreate={onCreateFolder}
        onRename={onRenameFolder}
        onMove={onMoveFolder}
        onDelete={onDeleteFolder}
      />
    </Stack>
  );
}

export interface MailboxHierarchyItem {
  mailbox: MailboxSummary;
  depth: number;
  displayName: string;
  ancestorIds: string[];
  hasChildren: boolean;
}

export function flattenMailboxHierarchy(mailboxes: MailboxSummary[]): MailboxHierarchyItem[] {
  const nodes = mailboxes.map((mailbox, index) => ({ mailbox, index, children: [] as number[] }));
  const byPath = new Map<string, number>();
  const keyFor = (mailbox: MailboxSummary, name = mailbox.name) => `${mailbox.delimiter ?? ""}\u0000${name}`;
  nodes.forEach((node, index) => byPath.set(keyFor(node.mailbox), index));

  const roots: number[] = [];
  nodes.forEach((node, index) => {
    const delimiter = node.mailbox.delimiter;
    const boundary = delimiter ? node.mailbox.name.lastIndexOf(delimiter) : -1;
    const parentIndex = boundary > 0
      ? byPath.get(keyFor(node.mailbox, node.mailbox.name.slice(0, boundary)))
      : undefined;
    if (parentIndex === undefined || parentIndex === index) roots.push(index);
    else nodes[parentIndex].children.push(index);
  });

  const result: MailboxHierarchyItem[] = [];
  const visit = (index: number, depth: number, ancestorIds: string[]) => {
    const node = nodes[index];
    const delimiter = node.mailbox.delimiter;
    const boundary = depth > 0 && delimiter ? node.mailbox.name.lastIndexOf(delimiter) : -1;
    result.push({
      mailbox: node.mailbox,
      depth,
      displayName: boundary >= 0 ? node.mailbox.name.slice(boundary + delimiter!.length) : node.mailbox.name,
      ancestorIds,
      hasChildren: node.children.length > 0,
    });
    node.children.sort((left, right) => nodes[left].index - nodes[right].index);
    node.children.forEach((child) => visit(child, depth + 1, [...ancestorIds, node.mailbox.id]));
  };
  roots.sort((left, right) => nodes[left].index - nodes[right].index);
  roots.forEach((root) => visit(root, 0, []));
  return result;
}

function MailboxIcon({ role }: { role: MailboxRole }) {
  const props = { className: "size-[18px] shrink-0", strokeWidth: 1.8, "aria-hidden": true } as const;
  if (role === "inbox") return <Inbox {...props} />;
  if (role === "sent") return <Send {...props} />;
  if (role === "drafts") return <FilePenLine {...props} />;
  if (role === "archive") return <Archive {...props} />;
  if (role === "junk") return <ShieldAlert {...props} />;
  if (role === "trash") return <Trash2 {...props} />;
  return <Folder {...props} />;
}

import {
  Archive,
  ChevronDown,
  ChevronRight,
  FilePenLine,
  Folder,
  Inbox,
  MailPlus,
  RefreshCw,
  Send,
  Settings,
  ShieldAlert,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { normalizeCommandError } from "@/app/api";
import type { DraftListItem, MailboxRole, MailboxSummary, SyncProgress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Progress } from "@/components/ui/progress";
import { LabelText, Text } from "@/components/ui/typography";

interface MailboxPaneProps {
  mailboxes: MailboxSummary[];
  selectedMailboxId: string;
  onSelect: (mailboxId: string) => void;
  progress?: SyncProgress;
  error?: unknown;
  onCompose: () => void;
  drafts: DraftListItem[];
  onOpenDraft: (draftId: string) => void;
  onDeleteDraft: (draftId: string) => Promise<void>;
  onReceive: () => void;
  receiving: boolean;
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
  drafts,
  onOpenDraft,
  onDeleteDraft,
  onReceive,
  receiving,
  onOpenSettings,
  collapsed = false,
}: MailboxPaneProps) {
  const { t } = useTranslation();
  const [pendingDeleteDraftId, setPendingDeleteDraftId] = useState<string | null>(null);
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<Set<string>>(() => new Set());
  const activeSync = progress && !["idle", "complete", "failed"].includes(progress.phase);
  const percentage = progress?.total ? (progress.completed / progress.total) * 100 : 8;
  const normalizedError = error ? normalizeCommandError(error) : null;
  const mailboxItems = flattenMailboxHierarchy(mailboxes);
  const visibleMailboxItems = mailboxItems.filter((item) =>
    item.ancestorIds.every((ancestorId) => !collapsedFolderIds.has(ancestorId)));

  function toggleFolder(mailboxId: string) {
    setCollapsedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(mailboxId)) next.delete(mailboxId);
      else next.add(mailboxId);
      return next;
    });
  }

  useEffect(() => {
    if (!pendingDeleteDraftId) return;
    const timeout = window.setTimeout(() => setPendingDeleteDraftId(null), 4_000);
    return () => window.clearTimeout(timeout);
  }, [pendingDeleteDraftId]);

  return (
    <Stack className={collapsed ? "min-h-0 flex-1 items-center px-2 py-5" : "min-h-0 flex-1 px-4 py-5"} gap="md">
      <Inline className={collapsed ? "w-full justify-center gap-0" : "w-full gap-1"}>
        <Button
          className={collapsed ? "mx-auto size-11 flex-none p-0" : "h-11 min-w-0 flex-1 justify-start px-4"}
          aria-label={collapsed ? t("mail.compose") : undefined}
          title={collapsed ? t("mail.compose") : undefined}
          onClick={onCompose}
        >
          <MailPlus className="size-[18px] shrink-0" />
          {collapsed ? null : t("mail.compose")}
        </Button>
        {drafts.length && !collapsed ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button className="h-11 w-10 px-0" aria-label={t("composer.openDraft")}>
                <ChevronDown size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-72 p-2">
              <DropdownMenuLabel className="px-2 py-1.5 normal-case">{t("composer.localDrafts")}</DropdownMenuLabel>
              {drafts.map((draft) => (
                <Inline key={draft.id} className="gap-1 rounded-md px-1 hover:bg-accent/70">
                  <DropdownMenuItem
                    className="h-auto min-h-12 min-w-0 flex-1 items-start bg-transparent px-2 py-2.5 focus:bg-transparent"
                    onSelect={() => onOpenDraft(draft.id)}
                  >
                    <FilePenLine className="mt-0.5 shrink-0" size={15} />
                    <Stack gap="xs" className="min-w-0 py-0.5">
                      <Text className="truncate text-[length:var(--ui-font-control)] leading-5 text-foreground">{draft.subject || t("mail.noSubject")}</Text>
                      <Text className="truncate text-[length:var(--ui-font-caption)] leading-4">
                        {draft.recipients.map((recipient) => recipient.name || recipient.email).join(", ") || t("composer.noRecipients")}
                      </Text>
                    </Stack>
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className={pendingDeleteDraftId === draft.id
                      ? "size-7 shrink-0 justify-center self-center bg-destructive p-0 text-white focus:bg-destructive/90 focus:text-white"
                      : "size-7 shrink-0 justify-center self-center bg-transparent p-0 text-muted-foreground"}
                    aria-label={pendingDeleteDraftId === draft.id ? t("composer.confirmDeleteDraft") : t("composer.deleteDraft")}
                    title={pendingDeleteDraftId === draft.id ? t("composer.confirmDeleteDraft") : t("composer.deleteDraft")}
                    onSelect={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      if (pendingDeleteDraftId !== draft.id) {
                        setPendingDeleteDraftId(draft.id);
                        return;
                      }
                      void onDeleteDraft(draft.id).then(() => setPendingDeleteDraftId(null)).catch(() => undefined);
                    }}
                  >
                    {pendingDeleteDraftId === draft.id ? <Trash2 size={13} /> : <X size={13} />}
                  </DropdownMenuItem>
                </Inline>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </Inline>
      <Inline className={collapsed ? "w-full justify-center" : "w-full px-2 pt-1"}>
        {collapsed ? null : (
          <LabelText className="min-w-0 flex-1 text-[length:var(--ui-font-caption)] tracking-[0.09em] text-muted-foreground uppercase">
            {t("mail.folders")}
          </LabelText>
        )}
        <Button
          variant="ghost"
          size="icon"
          className={collapsed ? "size-9" : "size-7"}
          aria-label={t("mail.receive")}
          title={t("mail.receive")}
          disabled={receiving}
          onClick={onReceive}
        >
          <RefreshCw className={receiving ? "animate-spin" : undefined} size={15} />
        </Button>
      </Inline>
      {activeSync && !collapsed ? (
        <Stack className="rounded-lg bg-card/70 p-3" gap="sm">
          <Text className="text-xs">{t(`sync.${progress.phase}`)}</Text>
          <Progress value={percentage} />
        </Stack>
      ) : null}
      {progress?.phase === "failed" && !collapsed ? <Alert tone="warning" title={t("sync.failed")}>{t("sync.failedDescription")}</Alert> : null}
      {normalizedError && !collapsed ? (
        <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>
      ) : null}
      {mailboxes.length ? (
        <OverlayScrollArea
          className={collapsed ? "-mr-1.5 min-h-0 w-full flex-1" : "-mr-3 min-h-0 flex-1"}
          contentClassName="gap-1.5"
          viewportClassName={collapsed ? "pr-1.5" : "pr-3"}
        >
          {visibleMailboxItems.map(({ mailbox, depth, displayName, hasChildren }) => {
            const selected = mailbox.id === selectedMailboxId;
            const label = mailbox.role === "other" ? displayName : t(`mailboxNames.${mailbox.role}`);
            const folderCollapsed = collapsedFolderIds.has(mailbox.id);
            if (!collapsed) {
              return (
                <Inline
                  key={mailbox.id}
                  className={selected
                    ? "h-10 w-full gap-0 rounded-md bg-card pr-2 text-foreground shadow-[0_6px_20px_rgb(15_23_42/0.06)]"
                    : "h-10 w-full gap-0 rounded-md pr-2 text-muted-foreground transition-colors hover:bg-foreground/5"}
                  style={{ paddingInlineStart: `${4 + depth * 16}px` }}
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
                    className="h-10 min-w-0 flex-1 justify-start rounded-md bg-transparent px-1.5 text-inherit hover:bg-transparent hover:text-foreground"
                    aria-label={label}
                    onClick={() => onSelect(mailbox.id)}
                  >
                    <MailboxIcon role={mailbox.role} />
                    <Text className="min-w-0 flex-1 truncate text-left text-[length:var(--ui-font-control)] text-inherit">{label}</Text>
                    {mailbox.unreadCount ? (
                      <Text className="rounded-full bg-primary px-2 py-0.5 text-[10px] font-bold leading-none text-primary-foreground">{mailbox.unreadCount}</Text>
                    ) : null}
                  </Button>
                </Inline>
              );
            }
            return (
              <Button
                key={mailbox.id}
                variant="ghost"
                className={selected
                  ? "mx-auto size-11 flex-none justify-center bg-card p-0 text-foreground shadow-[0_6px_20px_rgb(15_23_42/0.06)] hover:bg-card"
                  : "mx-auto size-11 flex-none justify-center p-0"}
                aria-label={label}
                title={label}
                onClick={() => onSelect(mailbox.id)}
              >
                <MailboxIcon role={mailbox.role} />
              </Button>
            );
          })}
        </OverlayScrollArea>
      ) : (
        <EmptyState className="mt-6 flex-1 items-center p-4 text-center" icon={<Inbox size={21} />} title={t("mail.noFolders")} />
      )}
      <Button
        variant="ghost"
        className={collapsed
          ? "mx-auto mt-auto size-11 flex-none justify-center p-0"
          : "mt-auto h-10 w-full flex-none justify-start px-3"}
        aria-label={t("mail.settings")}
        title={collapsed ? t("mail.settings") : undefined}
        onClick={onOpenSettings}
      >
        <Settings className="size-[18px] shrink-0" strokeWidth={1.8} />
        {collapsed ? null : <Text className="text-[length:var(--ui-font-control)] text-inherit">{t("mail.settings")}</Text>}
      </Button>
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

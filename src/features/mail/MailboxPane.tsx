import {
  Archive,
  ChevronDown,
  FilePenLine,
  Folder,
  Inbox,
  MailPlus,
  Send,
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
  collapsed = false,
}: MailboxPaneProps) {
  const { t } = useTranslation();
  const [pendingDeleteDraftId, setPendingDeleteDraftId] = useState<string | null>(null);
  const activeSync = progress && !["idle", "complete", "failed"].includes(progress.phase);
  const percentage = progress?.total ? (progress.completed / progress.total) * 100 : 8;
  const normalizedError = error ? normalizeCommandError(error) : null;

  useEffect(() => {
    if (!pendingDeleteDraftId) return;
    const timeout = window.setTimeout(() => setPendingDeleteDraftId(null), 4_000);
    return () => window.clearTimeout(timeout);
  }, [pendingDeleteDraftId]);

  return (
    <Stack className={collapsed ? "min-h-0 flex-1 items-center px-2 py-3" : "min-h-0 flex-1 p-3"} gap="sm">
      <Inline className="gap-0">
        <Button
          className={collapsed
            ? "size-10 px-0"
            : drafts.length
              ? "h-10 flex-1 justify-start rounded-r-none"
              : "h-10 w-full justify-start"}
          aria-label={collapsed ? t("mail.compose") : undefined}
          title={collapsed ? t("mail.compose") : undefined}
          onClick={onCompose}
        >
          <MailPlus size={17} />
          {collapsed ? null : t("mail.compose")}
        </Button>
        {drafts.length && !collapsed ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                className="h-10 w-9 rounded-l-none border-l border-l-primary-foreground/25 px-0"
                aria-label={t("composer.openDraft")}
              >
                <ChevronDown size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-72 p-2">
              <DropdownMenuLabel className="px-2 py-1.5 normal-case">{t("composer.localDrafts")}</DropdownMenuLabel>
              {drafts.map((draft) => (
                <Inline key={draft.id} className="gap-1">
                  <DropdownMenuItem
                    className="h-auto min-h-12 min-w-0 flex-1 items-start px-3 py-2.5"
                    onSelect={() => onOpenDraft(draft.id)}
                  >
                    <FilePenLine className="mt-0.5 shrink-0" size={15} />
                    <Stack gap="xs" className="min-w-0 py-0.5">
                      <Text className="truncate text-[13px] leading-5 text-foreground">
                        {draft.subject || t("mail.noSubject")}
                      </Text>
                      <Text className="truncate text-[11px] leading-4">
                        {draft.recipients.map((recipient) => recipient.name || recipient.email).join(", ") || t("composer.noRecipients")}
                      </Text>
                    </Stack>
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className={pendingDeleteDraftId === draft.id
                      ? "size-7 shrink-0 justify-center self-center bg-destructive p-0 text-white focus:bg-destructive/90 focus:text-white"
                      : "size-7 shrink-0 justify-center self-center p-0 text-muted-foreground"}
                    aria-label={pendingDeleteDraftId === draft.id
                      ? t("composer.confirmDeleteDraft")
                      : t("composer.deleteDraft")}
                    title={pendingDeleteDraftId === draft.id
                      ? t("composer.confirmDeleteDraft")
                      : t("composer.deleteDraft")}
                    onSelect={(event) => {
                      if (pendingDeleteDraftId !== draft.id) {
                        event.preventDefault();
                        setPendingDeleteDraftId(draft.id);
                        return;
                      }
                      void onDeleteDraft(draft.id)
                        .then(() => setPendingDeleteDraftId(null))
                        .catch(() => undefined);
                    }}
                  >
                    {pendingDeleteDraftId === draft.id
                      ? <Trash2 size={13} />
                      : <X size={13} />}
                  </DropdownMenuItem>
                </Inline>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </Inline>
      {collapsed ? null : <LabelText className="px-2 py-1 text-muted-foreground">{t("mail.folders")}</LabelText>}
      {activeSync && !collapsed ? (
        <Stack className="rounded-sm border border-border bg-background p-3" gap="sm">
          <Text className="text-xs">{t(`sync.${progress.phase}`)}</Text>
          <Progress value={percentage} />
        </Stack>
      ) : null}
      {progress?.phase === "failed" && !collapsed ? (
        <Alert tone="warning" title={t("sync.failed")}>{t("sync.failedDescription")}</Alert>
      ) : null}
      {normalizedError && !collapsed ? (
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      ) : null}
      {mailboxes.length ? (
        <Stack className={collapsed ? "min-h-0 w-full overflow-auto" : "min-h-0 overflow-auto"} gap="xs">
          {mailboxes.map((mailbox) => (
            <Button
              key={mailbox.id}
              variant="ghost"
              className={collapsed
                ? mailbox.id === selectedMailboxId
                  ? "h-10 w-full justify-center bg-accent px-0 text-foreground"
                  : "h-10 w-full justify-center px-0"
                :
                mailbox.id === selectedMailboxId
                  ? "h-9 w-full justify-start bg-accent px-2.5 text-foreground"
                  : "h-9 w-full justify-start px-2.5"
              }
              aria-label={mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)}
              title={collapsed ? (mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)) : undefined}
              onClick={() => onSelect(mailbox.id)}
            >
              <MailboxIcon role={mailbox.role} />
              {collapsed ? null : <Text className="min-w-0 flex-1 truncate text-left text-[13px] text-inherit">
                {mailbox.role === "other"
                  ? mailbox.name
                  : t(`mailboxNames.${mailbox.role}`)}
              </Text>}
              {mailbox.unreadCount && !collapsed ? (
                <Text className="rounded-xs bg-primary/10 px-1.5 text-[11px] font-semibold text-primary">
                  {mailbox.unreadCount}
                </Text>
              ) : null}
            </Button>
          ))}
        </Stack>
      ) : (
        <EmptyState
          className="mt-6 items-center p-4 text-center"
          icon={<Inbox size={21} />}
          title={t("mail.noFolders")}
        />
      )}
    </Stack>
  );
}

function MailboxIcon({ role }: { role: MailboxRole }) {
  const props = { size: 16, "aria-hidden": true } as const;
  if (role === "inbox") return <Inbox {...props} />;
  if (role === "sent") return <Send {...props} />;
  if (role === "drafts") return <FilePenLine {...props} />;
  if (role === "archive") return <Archive {...props} />;
  if (role === "junk") return <ShieldAlert {...props} />;
  if (role === "trash") return <Trash2 {...props} />;
  return <Folder {...props} />;
}

import {
  Archive,
  FilePenLine,
  Folder,
  Inbox,
  Send,
  ShieldAlert,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { normalizeCommandError } from "@/app/api";
import type { MailboxRole, MailboxSummary, SyncProgress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Stack } from "@/components/ui/layout";
import { Progress } from "@/components/ui/progress";
import { LabelText, Text } from "@/components/ui/typography";

interface MailboxPaneProps {
  mailboxes: MailboxSummary[];
  selectedMailboxId: string;
  onSelect: (mailboxId: string) => void;
  progress?: SyncProgress;
  error?: unknown;
}

export function MailboxPane({
  mailboxes,
  selectedMailboxId,
  onSelect,
  progress,
  error,
}: MailboxPaneProps) {
  const { t } = useTranslation();
  const activeSync = progress && !["idle", "complete", "failed"].includes(progress.phase);
  const percentage = progress?.total ? (progress.completed / progress.total) * 100 : 8;
  const normalizedError = error ? normalizeCommandError(error) : null;

  return (
    <Stack className="min-h-0 flex-1 p-3" gap="sm">
      <LabelText className="px-2 py-1 text-muted-foreground">{t("mail.folders")}</LabelText>
      {activeSync ? (
        <Stack className="rounded-sm border border-border bg-background p-3" gap="sm">
          <Text className="text-xs">{t(`sync.${progress.phase}`)}</Text>
          <Progress value={percentage} />
        </Stack>
      ) : null}
      {progress?.phase === "failed" ? (
        <Alert tone="warning" title={t("sync.failed")}>{t("sync.failedDescription")}</Alert>
      ) : null}
      {normalizedError ? (
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      ) : null}
      {mailboxes.length ? (
        <Stack className="min-h-0 overflow-auto" gap="xs">
          {mailboxes.map((mailbox) => (
            <Button
              key={mailbox.id}
              variant="ghost"
              className={
                mailbox.id === selectedMailboxId
                  ? "h-9 w-full justify-start bg-accent px-2.5 text-foreground"
                  : "h-9 w-full justify-start px-2.5"
              }
              onClick={() => onSelect(mailbox.id)}
            >
              <MailboxIcon role={mailbox.role} />
              <Text className="min-w-0 flex-1 truncate text-left text-[13px] text-inherit">
                {mailbox.role === "other"
                  ? mailbox.name
                  : t(`mailboxNames.${mailbox.role}`)}
              </Text>
              {mailbox.unreadCount ? (
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

import { Archive, CloudUpload, Download, FilePenLine, FileText, FolderInput, Mail, MailOpen, Paperclip, Star, Trash2 } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AttachmentSummary, MailboxSummary, MessageAddress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/dialog";
import { Inline, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { SafeMailFrame } from "./SafeMailFrame";

export function MessageViewer({ accountId, mailboxId, messageId, mailboxes, onMessageRemoved }: {
  accountId: string;
  mailboxId: string;
  messageId: string;
  mailboxes: MailboxSummary[];
  onMessageRemoved: () => void;
}) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [rawSource, setRawSource] = useState<string | null>(null);
  const [remoteImagesAllowed, setRemoteImagesAllowed] = useState(false);

  useEffect(() => {
    setRemoteImagesAllowed(false);
  }, [messageId]);
  const query = useQuery({
    queryKey: ["message", accountId, mailboxId, messageId],
    queryFn: () => api.getMessageDetail(accountId, messageId, mailboxId),
    enabled: Boolean(accountId && mailboxId && messageId),
  });
  const attachmentMutation = useMutation({
    mutationFn: (attachmentId: string) => api.requestAttachment(accountId, attachmentId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["message", accountId, messageId] }),
  });
  const bodyMutation = useMutation({
    mutationFn: () => api.requestMessageBody(accountId, messageId, mailboxId),
    onSuccess: (detail) => queryClient.setQueryData(["message", accountId, mailboxId, messageId], detail),
  });
  const rawMutation = useMutation({
    mutationFn: () => api.requestRawMessage(accountId, messageId),
    onSuccess: setRawSource,
  });
  const messageOperation = useMutation({
    mutationFn: async (operation: { kind: "read" | "flag" | "move" | "copy" | "archive" | "delete"; destination?: string }) => {
      if (operation.kind === "read") await api.setMessageRead(accountId, mailboxId, [messageId], query.data?.unread ?? false);
      if (operation.kind === "flag") await api.setMessageFlagged(accountId, mailboxId, [messageId], !query.data?.flagged);
      if (operation.kind === "move" && operation.destination) await api.moveMessages(accountId, mailboxId, operation.destination, [messageId]);
      if (operation.kind === "copy" && operation.destination) await api.copyMessages(accountId, mailboxId, operation.destination, [messageId]);
      if (operation.kind === "archive") await api.archiveMessages(accountId, mailboxId, [messageId]);
      if (operation.kind === "delete") await api.deleteMessages(accountId, mailboxId, [messageId]);
      return operation.kind;
    },
    onSuccess: (kind) => {
      void queryClient.invalidateQueries({ queryKey: ["mailboxes", accountId] });
      void queryClient.invalidateQueries({ queryKey: ["messages", accountId] });
      void queryClient.invalidateQueries({ queryKey: ["message", accountId] });
      if (["move", "archive", "delete"].includes(kind)) onMessageRemoved();
    },
  });
  const editDraftMutation = useMutation({
    mutationFn: () => api.openRemoteDraft(accountId, messageId),
  });

  if (!messageId) {
    return <EmptyState icon={<MailOpen size={28} />} title={t("mail.selectMessage")} />;
  }
  if (query.isPending) {
    return <Stack className="m-auto items-center"><Spinner size={24} /></Stack>;
  }
  if (query.isError || !query.data) {
    const error = normalizeCommandError(query.error);
    return (
      <Alert className="m-5" tone="danger" title={t("errors.title")}>
        {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
      </Alert>
    );
  }
  const message = query.data;
  const operationError = bodyMutation.error ?? rawMutation.error ?? attachmentMutation.error ?? messageOperation.error ?? editDraftMutation.error;
  const normalizedOperationError = operationError ? normalizeCommandError(operationError) : null;
  const date = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(message.receivedAt * 1000));

  async function showRemoteImages() {
    if (message.safeHtml && /<img[^>]+src=["']https?:\/\//i.test(message.safeHtml)) {
      setRemoteImagesAllowed(true);
      return;
    }
    try {
      await bodyMutation.mutateAsync();
      setRemoteImagesAllowed(true);
    } catch {
      // The mutation error is rendered by the shared operation alert.
    }
  }

  return (
    <Stack className="min-h-0 flex-1" gap="none">
      <Stack className="border-b border-border px-6 py-5" gap="sm">
        <Heading level={2}>{message.subject || t("mail.noSubject")}</Heading>
        <Inline className="flex-wrap">
          <LabelText>{formatAddresses(message.from)}</LabelText>
          <Text className="text-xs">{date}</Text>
          <Button
            className="ml-auto"
            variant="ghost"
            size="sm"
            loading={rawMutation.isPending}
            onClick={() => rawMutation.mutate()}
          >
            <FileText size={14} />
            {t("mail.viewSource")}
          </Button>
        </Inline>
        <Text className="text-xs">
          {t("mail.toRecipients", { recipients: formatAddresses(message.to) })}
        </Text>
        <Inline className="flex-wrap border-t border-border/70 pt-3" role="toolbar" aria-label={t("mail.messageActions")}>
          {mailboxes.find((mailbox) => mailbox.id === mailboxId)?.role === "drafts" ? (
            <Button variant="secondary" size="sm" loading={editDraftMutation.isPending} onClick={() => editDraftMutation.mutate()}>
              <FilePenLine size={14} />{t("mail.editDraft")}
            </Button>
          ) : null}
          <Button variant="ghost" size="sm" onClick={() => messageOperation.mutate({ kind: "read" })}>
            {message.unread ? <MailOpen size={14} /> : <Mail size={14} />}
            {message.unread ? t("mail.markRead") : t("mail.markUnread")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => messageOperation.mutate({ kind: "flag" })}>
            <Star size={14} className={message.flagged ? "fill-current text-primary" : undefined} />
            {message.flagged ? t("mail.removeStar") : t("mail.addStar")}
          </Button>
          {mailboxes.some((mailbox) => mailbox.role === "archive" && mailbox.id !== mailboxId) ? (
            <Button variant="ghost" size="sm" onClick={() => messageOperation.mutate({ kind: "archive" })}>
              <Archive size={14} />{t("mail.archive")}
            </Button>
          ) : null}
          <MailboxActionMenu
            icon={<FolderInput size={14} />}
            label={t("mail.moveTo")}
            mailboxes={mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== mailboxId)}
            onSelect={(destination) => messageOperation.mutate({ kind: "move", destination })}
          />
          <Button variant="ghost" size="sm" className="text-destructive" onClick={() => messageOperation.mutate({ kind: "delete" })}>
            <Trash2 size={14} />{t("mail.delete")}
          </Button>
          {message.pendingOperation ? (
            <Inline className="ml-auto text-muted-foreground"><CloudUpload size={14} /><Text className="text-xs">{t("mail.pendingSync")}</Text></Inline>
          ) : null}
        </Inline>
        {message.remoteImagesBlocked && !remoteImagesAllowed ? (
          <Alert tone="warning" title={t("mail.remoteImagesBlocked")}>
            <Inline className="flex-wrap justify-between">
              <Text className="text-xs text-current">{t("mail.remoteImagesBlockedDescription")}</Text>
              <Button variant="secondary" size="sm" loading={bodyMutation.isPending} onClick={() => void showRemoteImages()}>
                {t("mail.showRemoteImages")}
              </Button>
            </Inline>
          </Alert>
        ) : null}
        {normalizedOperationError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${normalizedOperationError.code}`, {
              defaultValue: t("common.unexpectedError"),
            })}
          </Alert>
        ) : null}
      </Stack>

      <Stack className="min-h-0 flex-1" gap="none">
        {message.safeHtml ? (
          <SafeMailFrame
            document={message.safeHtml}
            title={message.subject || t("mail.messageBody")}
            allowRemoteImages={remoteImagesAllowed}
          />
        ) : message.plainText ? (
          <Text className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap p-6 text-sm leading-relaxed text-foreground">
            {message.plainText}
          </Text>
        ) : (
          <EmptyState
            icon={<MailOpen size={24} />}
            title={t("mail.bodyUnavailable")}
            description={bodyMutation.isPending
              ? t("mail.downloadingBody")
              : t("mail.bodyUnavailableDescription")}
            action={(
              <Button loading={bodyMutation.isPending} onClick={() => bodyMutation.mutate()}>
                <Download size={14} />
                {t("mail.downloadBody")}
              </Button>
            )}
          />
        )}
      </Stack>

      {message.attachments.length ? (
        <Stack className="border-t border-border p-4" gap="sm">
          <Inline><Paperclip size={15} /><LabelText>{t("mail.attachments")}</LabelText></Inline>
          <Inline className="flex-wrap">
            {message.attachments.map((attachment) => (
              <AttachmentButton
                key={attachment.id}
                attachment={attachment}
                loading={attachmentMutation.isPending && attachmentMutation.variables === attachment.id}
                onClick={() => attachmentMutation.mutate(attachment.id)}
              />
            ))}
          </Inline>
        </Stack>
      ) : null}
      <Modal
        open={rawSource !== null}
        onOpenChange={(open) => { if (!open) setRawSource(null); }}
        title={t("mail.sourceTitle")}
        closeLabel={t("common.close")}
      >
        <Text className="mt-4 max-h-[65vh] overflow-auto whitespace-pre-wrap break-all rounded-sm border border-border bg-muted p-3 font-mono text-xs text-foreground">
          {rawSource ?? ""}
        </Text>
      </Modal>
    </Stack>
  );
}

function MailboxActionMenu({ icon, label, mailboxes, onSelect }: {
  icon: ReactNode;
  label: string;
  mailboxes: MailboxSummary[];
  onSelect: (mailboxId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" disabled={!mailboxes.length}>{icon}{label}</Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-72 overflow-auto">
        {mailboxes.map((mailbox) => (
          <DropdownMenuItem key={mailbox.id} onSelect={() => onSelect(mailbox.id)}>
            {mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AttachmentButton({
  attachment,
  loading,
  onClick,
}: {
  attachment: AttachmentSummary;
  loading: boolean;
  onClick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Button
      variant="secondary"
      loading={loading}
      disabled={attachment.availability === "available"}
      title={attachment.availability === "available" ? t("mail.attachmentReady") : undefined}
      onClick={onClick}
    >
      <Download size={14} />
      {attachment.fileName}
      <Text className="text-[11px]">{formatBytes(attachment.size)}</Text>
      {attachment.availability === "available" ? t("mail.downloaded") : null}
    </Button>
  );
}

function formatAddresses(addresses: MessageAddress[]) {
  return addresses.map((address) => address.name || address.email).join(", ") || "—";
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

import {
  Archive,
  CloudUpload,
  Copy,
  Download,
  FilePenLine,
  FileText,
  FolderInput,
  Forward,
  Mail,
  MailOpen,
  MoreHorizontal,
  Paperclip,
  Reply,
  ReplyAll,
  Star,
  Trash2,
} from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AttachmentSummary, MailboxSummary, MessageAddress, MessageComposeAction } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/dialog";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { SafeMailFrame } from "./SafeMailFrame";
import { MessageAttachment } from "./MessageAttachment";
import { activateMessageAttachment } from "./message-attachment-actions";
import { messageQueryKeys } from "./message-query-keys";

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
  const readingPreferences = useQuery({
    queryKey: ["reading-preferences"],
    queryFn: api.getReadingPreferences,
  });

  useEffect(() => setRemoteImagesAllowed(false), [messageId]);
  const query = useQuery({
    queryKey: messageQueryKeys.detail(accountId, mailboxId, messageId),
    queryFn: () => api.getMessageDetail(accountId, messageId, mailboxId),
    enabled: Boolean(accountId && mailboxId && messageId),
  });
  const attachmentMutation = useMutation({
    mutationFn: async (attachment: AttachmentSummary) => {
      const autoOpenAfterDownload = attachment.availability === "available"
        ? true
        : (readingPreferences.data ?? await api.getReadingPreferences()).autoOpenDownloadedAttachments;
      await activateMessageAttachment(attachment, autoOpenAfterDownload, {
        download: (attachmentId) => api.requestAttachment(accountId, attachmentId),
        open: (attachmentId) => api.openMessageAttachment(accountId, attachmentId),
      });
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: messageQueryKeys.detail(accountId, mailboxId, messageId) }),
  });
  const saveAttachmentMutation = useMutation({
    mutationFn: (attachment: AttachmentSummary) => api.saveMessageAttachmentAs(accountId, attachment.id),
    onSettled: () => queryClient.invalidateQueries({ queryKey: messageQueryKeys.detail(accountId, mailboxId, messageId) }),
  });
  const bodyMutation = useMutation({
    mutationFn: () => api.requestMessageBody(accountId, messageId, mailboxId),
    onSuccess: (detail) => queryClient.setQueryData(messageQueryKeys.detail(accountId, mailboxId, messageId), detail),
  });
  const rawMutation = useMutation({ mutationFn: () => api.requestRawMessage(accountId, messageId), onSuccess: setRawSource });
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
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) });
      if (["move", "archive", "delete"].includes(kind)) onMessageRemoved();
    },
  });
  const editDraftMutation = useMutation({ mutationFn: () => api.openRemoteDraft(accountId, messageId) });
  const composeMutation = useMutation({
    mutationFn: (action: MessageComposeAction) => api.openMessageActionComposer(accountId, messageId, action),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["drafts", accountId] }),
  });

  if (!messageId) return <EmptyState icon={<MailOpen size={28} />} title={t("mail.selectMessage")} />;
  if (query.isPending) return <Stack className="m-auto items-center"><Spinner size={24} /></Stack>;
  if (query.isError || !query.data) {
    const error = normalizeCommandError(query.error);
    return <Alert className="m-5" tone="danger" title={t("errors.title")}>{t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>;
  }

  const message = query.data;
  const allowRemoteImages = remoteImagesAllowed || readingPreferences.data?.autoLoadRemoteImages === true;
  const operationError = bodyMutation.error ?? rawMutation.error ?? attachmentMutation.error ?? saveAttachmentMutation.error ?? messageOperation.error ?? editDraftMutation.error ?? composeMutation.error;
  const normalizedOperationError = operationError ? normalizeCommandError(operationError) : null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(message.receivedAt * 1000));
  const sender = message.from[0];
  const senderLabel = sender?.name || sender?.email || "—";
  const senderInitial = senderLabel.trim().charAt(0).toLocaleUpperCase();
  const isDraft = mailboxes.find((mailbox) => mailbox.id === mailboxId)?.role === "drafts";

  async function showRemoteImages() {
    if (message.safeHtml && /<img[^>]+src=["']https?:\/\//i.test(message.safeHtml)) {
      setRemoteImagesAllowed(true);
      return;
    }
    try {
      await bodyMutation.mutateAsync();
      setRemoteImagesAllowed(true);
    } catch {
      // The shared operation alert renders the error.
    }
  }

  return (
    <Stack className="min-h-0 flex-1 bg-card" gap="none">
      <Stack className="px-8 pt-7 pb-5" gap="lg">
        <Inline className="items-start gap-3">
          <span className="grid size-11 shrink-0 place-items-center rounded-full bg-primary/12 text-sm font-bold text-primary">{senderInitial}</span>
          <Stack className="min-w-0 flex-1" gap="xs">
            <Inline className="flex-wrap gap-x-3 gap-y-1">
              <LabelText className="text-[15px]">{senderLabel}</LabelText>
              <Text className="text-xs">{sender?.email !== senderLabel ? sender?.email : null}</Text>
            </Inline>
            <Text className="text-xs">{t("mail.toRecipients", { recipients: formatAddresses(message.to) })}</Text>
          </Stack>
          <Stack className="shrink-0 items-end" gap="sm">
            <Text className="text-[length:var(--ui-font-caption)]">{date}</Text>
            <Inline className="gap-0.5" role="toolbar" aria-label={t("mail.messageActions") }>
              <IconAction label={message.flagged ? t("mail.removeStar") : t("mail.addStar")} onClick={() => messageOperation.mutate({ kind: "flag" })}>
                <Star size={18} className={message.flagged ? "fill-current text-[#f2b84b]" : undefined} />
              </IconAction>
              <IconAction label={t("mail.reply")} loading={composeMutation.isPending && composeMutation.variables === "reply"} onClick={() => composeMutation.mutate("reply")}><Reply size={18} /></IconAction>
              <IconAction label={t("mail.replyAll")} loading={composeMutation.isPending && composeMutation.variables === "reply_all"} onClick={() => composeMutation.mutate("reply_all")}><ReplyAll size={18} /></IconAction>
              <IconAction label={t("mail.forward")} loading={composeMutation.isPending && composeMutation.variables === "forward"} onClick={() => composeMutation.mutate("forward")}><Forward size={18} /></IconAction>
              {mailboxes.some((mailbox) => mailbox.role === "archive" && mailbox.id !== mailboxId) ? (
                <IconAction label={t("mail.archive")} onClick={() => messageOperation.mutate({ kind: "archive" })}><Archive size={18} /></IconAction>
              ) : null}
              <MailboxActionMenu
                icon={<FolderInput size={18} />}
                label={t("mail.moveTo")}
                mailboxes={mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== mailboxId)}
                onSelect={(destination) => messageOperation.mutate({ kind: "move", destination })}
              />
              <MailboxActionMenu
                icon={<Copy size={18} />}
                label={t("mail.copyTo")}
                mailboxes={mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== mailboxId)}
                onSelect={(destination) => messageOperation.mutate({ kind: "copy", destination })}
              />
              <IconAction label={t("mail.delete")} danger onClick={() => messageOperation.mutate({ kind: "delete" })}><Trash2 size={18} /></IconAction>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" aria-label={t("mail.moreActions")} title={t("mail.moreActions")}><MoreHorizontal size={18} /></Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onSelect={() => messageOperation.mutate({ kind: "read" })}>
                    {message.unread ? <MailOpen size={16} /> : <Mail size={16} />}
                    {message.unread ? t("mail.markRead") : t("mail.markUnread")}
                  </DropdownMenuItem>
                  {isDraft ? (
                    <DropdownMenuItem onSelect={() => editDraftMutation.mutate()}><FilePenLine size={16} />{t("mail.editDraft")}</DropdownMenuItem>
                  ) : null}
                  <DropdownMenuItem onSelect={() => rawMutation.mutate()}><FileText size={16} />{t("mail.viewSource")}</DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </Inline>
          </Stack>
        </Inline>
        <Heading level={1} className="max-w-none text-[28px] leading-tight lg:text-[30px]">{message.subject || t("mail.noSubject")}</Heading>
        {message.pendingOperation ? (
          <Inline className="text-muted-foreground"><CloudUpload size={14} /><Text className="text-xs">{t("mail.pendingSync")}</Text></Inline>
        ) : null}
        {message.remoteImagesBlocked && !allowRemoteImages ? (
          <Alert tone="warning" title={t("mail.remoteImagesBlocked")}>
            <Inline className="flex-wrap justify-between">
              <Text className="text-xs text-current">{t("mail.remoteImagesBlockedDescription")}</Text>
              <Button variant="secondary" size="sm" loading={bodyMutation.isPending} onClick={() => void showRemoteImages()}>{t("mail.showRemoteImages")}</Button>
            </Inline>
          </Alert>
        ) : null}
        {normalizedOperationError ? (
          <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedOperationError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>
        ) : null}
      </Stack>

      <Stack className="min-h-0 flex-1" gap="none">
        {message.safeHtml ? (
          <SafeMailFrame document={message.safeHtml} title={message.subject || t("mail.messageBody")} allowRemoteImages={allowRemoteImages} />
        ) : message.plainText ? (
          <Text className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap px-8 py-5 text-sm leading-[1.75] text-foreground">{message.plainText}</Text>
        ) : (
          <EmptyState
            icon={<MailOpen size={24} />}
            title={t("mail.bodyUnavailable")}
            description={bodyMutation.isPending ? t("mail.downloadingBody") : t("mail.bodyUnavailableDescription")}
            action={<Button loading={bodyMutation.isPending} onClick={() => bodyMutation.mutate()}><Download size={14} />{t("mail.downloadBody")}</Button>}
          />
        )}
      </Stack>

      {message.attachments.length ? (
        <Stack className="mx-5 mt-2 mb-4 rounded-lg bg-muted/60 p-3" gap="sm">
          <Inline><Paperclip size={15} /><LabelText>{t("mail.attachments")}</LabelText></Inline>
          <Inline className="flex-wrap gap-1.5">
            {message.attachments.map((attachment) => (
              <MessageAttachment
                key={attachment.id}
                attachment={attachment}
                opening={attachmentMutation.isPending && attachmentMutation.variables?.id === attachment.id}
                saving={saveAttachmentMutation.isPending && saveAttachmentMutation.variables?.id === attachment.id}
                onOpen={() => attachmentMutation.mutate(attachment)}
                onSaveAs={() => saveAttachmentMutation.mutate(attachment)}
              />
            ))}
          </Inline>
        </Stack>
      ) : null}
      <Modal open={rawSource !== null} onOpenChange={(open) => { if (!open) setRawSource(null); }} title={t("mail.sourceTitle")} closeLabel={t("common.close")}>
        <Text className="mt-4 max-h-[65vh] overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted p-3 font-mono text-xs text-foreground">{rawSource ?? ""}</Text>
      </Modal>
    </Stack>
  );
}

function IconAction({ label, loading, danger, onClick, children }: {
  label: string;
  loading?: boolean;
  danger?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={danger ? "text-muted-foreground hover:bg-destructive/10 hover:text-destructive" : undefined}
      aria-label={label}
      title={label}
      loading={loading}
      onClick={onClick}
    >
      {children}
    </Button>
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
        <Button variant="ghost" size="icon" disabled={!mailboxes.length} aria-label={label} title={label}>{icon}</Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="overflow-hidden">
        <OverlayScrollArea
          className="-mr-1.5"
          viewportClassName="pr-1.5"
          trackClassName="right-0"
          style={{
            height: `${Math.min(276, mailboxes.length * 36)}px`,
            maxHeight: "calc(var(--radix-dropdown-menu-content-available-height) - 12px)",
          }}
        >
          {mailboxes.map((mailbox) => (
            <DropdownMenuItem key={mailbox.id} onSelect={() => onSelect(mailbox.id)}>
              {mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)}
            </DropdownMenuItem>
          ))}
        </OverlayScrollArea>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function formatAddresses(addresses: MessageAddress[]) {
  return addresses.map((address) => address.name || address.email).join(", ") || "—";
}

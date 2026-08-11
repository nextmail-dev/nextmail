import {
  Archive,
  ChevronDown,
  ChevronUp,
  CloudUpload,
  Copy,
  Download,
  ExternalLink,
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
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AddressPresentation, AttachmentSummary, MailboxSummary, MessageComposeAction } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { SafeMailFrame } from "./SafeMailFrame";
import { formatBytes, MessageAttachment } from "./MessageAttachment";
import { activateMessageAttachment } from "./message-attachment-actions";
import { mailQueryKeys, messageQueryKeys } from "./mail-query-keys";
import { ContactIdentity } from "@/features/contacts/ContactIdentity";

export function MessageViewer({ accountId, mailboxId, messageId, mailboxes, allowOpenInNewWindow = true, onMessageRemoved, onOpenContact, onEditContact }: {
  accountId: string;
  mailboxId: string;
  messageId: string;
  mailboxes: MailboxSummary[];
  allowOpenInNewWindow?: boolean;
  onMessageRemoved: (messageId: string) => void;
  onOpenContact?: (contactId: string) => void;
  onEditContact?: (contactId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
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
  // Auto-fetch the body when a message without one is opened, so the user sees
  // a loading state instead of a manual "download body" button. The ref guards
  // against re-triggering (e.g. on error) for the same message.
  const bodyRequestedRef = useRef<string | null>(null);
  useEffect(() => {
    if (!query.data || query.data.bodyAvailability === "available" || bodyMutation.isPending) return;
    if (bodyRequestedRef.current === messageId) return;
    bodyRequestedRef.current = messageId;
    bodyMutation.mutate();
  }, [messageId, query.data?.bodyAvailability, bodyMutation.isPending]);
  const rawWindowMutation = useMutation({
    mutationFn: () => api.openRawMessageWindow(accountId, messageId),
  });
  const revealAttachmentMutation = useMutation({
    mutationFn: (attachment: AttachmentSummary) => api.revealMessageAttachment(accountId, attachment.id),
    onSettled: () => queryClient.invalidateQueries({ queryKey: messageQueryKeys.detail(accountId, mailboxId, messageId) }),
  });
  const previewWindowMutation = useMutation({
    mutationFn: () => api.openMessagePreviewWindow(accountId, mailboxId, messageId),
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
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) });
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) });
      if (["move", "archive", "delete"].includes(kind)) onMessageRemoved(messageId);
    },
  });
  const editDraftMutation = useMutation({ mutationFn: () => api.openRemoteDraft(accountId, messageId) });
  const composeMutation = useMutation({
    mutationFn: (action: MessageComposeAction) => api.openMessageActionComposer(accountId, messageId, action),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: mailQueryKeys.drafts(accountId) }),
  });

  if (!messageId) return <EmptyState icon={<MailOpen size={28} />} title={t("mail.selectMessage")} />;
  if (query.isPending) return <Stack className="m-auto items-center"><Spinner size={24} /></Stack>;
  if (query.isError || !query.data) {
    const error = normalizeCommandError(query.error);
    return <Alert className="m-5" tone="danger" title={t("errors.title")}>{t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>;
  }

  const message = query.data;
  const allowRemoteImages = remoteImagesAllowed || readingPreferences.data?.autoLoadRemoteImages === true;
  const operationError = bodyMutation.error ?? rawWindowMutation.error ?? previewWindowMutation.error ?? attachmentMutation.error ?? saveAttachmentMutation.error ?? revealAttachmentMutation.error ?? messageOperation.error ?? editDraftMutation.error ?? composeMutation.error;
  const normalizedOperationError = operationError ? normalizeCommandError(operationError) : null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(message.receivedAt * 1000));
  const sender = message.from[0];
  const senderName = sender?.name?.trim() || null;
  const senderLabel = sender?.name || sender?.email || "—";
  const senderInitial = senderLabel.trim().charAt(0).toLocaleUpperCase();
  const isDraft = mailboxes.find((mailbox) => mailbox.id === mailboxId)?.role === "drafts";
  const attachmentBytes = message.attachments.reduce((total, attachment) => total + attachment.size, 0);

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
    <Stack className="min-h-0 flex-1 overflow-hidden bg-card" gap="none">
      <Stack className="shrink-0 border-b border-border/70 px-5 py-4" gap="sm">
        <Inline className="flex-wrap items-start gap-x-4 gap-y-2">
          <Stack className="min-w-[220px] flex-1" gap="xs">
            <Heading level={1} className="select-text max-w-none text-lg leading-tight lg:text-lg">{message.subject || t("mail.noSubject")}</Heading>
            {message.pendingOperation ? (
              <Inline className="text-muted-foreground"><CloudUpload size={14} /><Text className="text-xs">{t("mail.pendingSync")}</Text></Inline>
            ) : null}
          </Stack>
          <Inline className="max-w-full flex-wrap justify-end gap-0.5" role="toolbar" aria-label={t("mail.messageActions")}>
            <IconAction label={message.flagged ? t("mail.removeStar") : t("mail.addStar")} onClick={() => messageOperation.mutate({ kind: "flag" })}>
              <Star size={18} className={message.flagged ? "fill-current text-[#f2b84b]" : undefined} />
            </IconAction>
            {!isDraft ? (
              <>
                <IconAction label={t("mail.reply")} loading={composeMutation.isPending && composeMutation.variables === "reply"} onClick={() => composeMutation.mutate("reply")}><Reply size={18} /></IconAction>
                <IconAction label={t("mail.replyAll")} loading={composeMutation.isPending && composeMutation.variables === "reply_all"} onClick={() => composeMutation.mutate("reply_all")}><ReplyAll size={18} /></IconAction>
                <IconAction label={t("mail.forward")} loading={composeMutation.isPending && composeMutation.variables === "forward"} onClick={() => composeMutation.mutate("forward")}><Forward size={18} /></IconAction>
              </>
            ) : null}
            {mailboxes.some((mailbox) => mailbox.role === "archive" && mailbox.id !== mailboxId) ? (
              <IconAction label={t("mail.archive")} onClick={() => messageOperation.mutate({ kind: "archive" })}><Archive size={18} /></IconAction>
            ) : null}
            <IconAction label={t("mail.delete")} danger onClick={() => messageOperation.mutate({ kind: "delete" })}><Trash2 size={18} /></IconAction>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" aria-label={t("mail.moreActions")} title={t("mail.moreActions")}><MoreHorizontal size={18} /></Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {allowOpenInNewWindow ? (
                  <DropdownMenuItem onSelect={() => previewWindowMutation.mutate()}><ExternalLink size={16} />{t("mail.openInNewWindow")}</DropdownMenuItem>
                ) : null}
                <DropdownMenuItem onSelect={() => messageOperation.mutate({ kind: "read" })}>
                  {message.unread ? <MailOpen size={16} /> : <Mail size={16} />}
                  {message.unread ? t("mail.markRead") : t("mail.markUnread")}
                </DropdownMenuItem>
                {isDraft ? (
                  <DropdownMenuItem onSelect={() => editDraftMutation.mutate()}><FilePenLine size={16} />{t("mail.editDraft")}</DropdownMenuItem>
                ) : null}
                {mailboxes.some((mailbox) => mailbox.selectable && mailbox.id !== mailboxId) ? (
                  <>
                    <DropdownMenuSeparator />
                    <MailboxActionSubMenu
                      icon={<FolderInput size={16} />}
                      label={t("mail.moveTo")}
                      mailboxes={mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== mailboxId)}
                      onSelect={(destination) => messageOperation.mutate({ kind: "move", destination })}
                    />
                    <MailboxActionSubMenu
                      icon={<Copy size={16} />}
                      label={t("mail.copyTo")}
                      mailboxes={mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== mailboxId)}
                      onSelect={(destination) => messageOperation.mutate({ kind: "copy", destination })}
                    />
                  </>
                ) : null}
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={() => rawWindowMutation.mutate()}><FileText size={16} />{t("mail.viewSource")}</DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </Inline>
        </Inline>

        <Inline className="items-start gap-3">
          <span className="grid size-10 shrink-0 place-items-center rounded-full bg-primary/12 text-sm font-bold text-primary">{senderInitial}</span>
          <Stack className="min-w-0 flex-1" gap="xs">
            <Inline className="flex-wrap gap-x-3 gap-y-1">
              {sender ? (
                <>
                  {senderName ? <LabelText className="select-text text-[15px]">{senderName}</LabelText> : null}
                  <ContactIdentity address={sender} onOpenContact={onOpenContact} onEditContact={onEditContact} tag>
                    <span className="select-text text-xs text-muted-foreground">{sender.email}</span>
                  </ContactIdentity>
                </>
              ) : <LabelText className="text-[15px]">—</LabelText>}
            </Inline>
            <AddressList label={t("composer.to")} addresses={message.to} onOpenContact={onOpenContact} onEditContact={onEditContact} />
            {message.cc.length ? (
              <AddressList label={t("composer.cc")} addresses={message.cc} onOpenContact={onOpenContact} onEditContact={onEditContact} />
            ) : null}
            {message.attachments.length ? (
              <Inline className="text-muted-foreground">
                <Paperclip size={13} />
                <Text className="text-xs">
                  {t("mail.attachmentOverview", {
                    count: message.attachments.length,
                    size: formatBytes(attachmentBytes),
                  })}
                </Text>
              </Inline>
            ) : null}
          </Stack>
          <Text className="shrink-0 pt-0.5 text-[length:var(--ui-font-caption)]">{date}</Text>
        </Inline>
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
          <div className="min-h-0 flex-1 overflow-hidden px-4 py-3">
            <SafeMailFrame document={message.safeHtml} title={message.subject || t("mail.messageBody")} allowRemoteImages={allowRemoteImages} />
          </div>
        ) : message.plainText ? (
          <OverlayScrollArea
            className="min-h-0 flex-1"
            viewportClassName="px-5 py-5"
          >
            <Text className="select-text whitespace-pre-wrap text-sm leading-[1.75] text-foreground">
              {message.plainText}
            </Text>
          </OverlayScrollArea>
        ) : message.bodyAvailability !== "available" && !bodyMutation.isError ? (
          <Stack className="m-auto items-center"><Spinner size={24} /></Stack>
        ) : (
          <Stack className="m-auto w-full max-w-md items-center px-8">
            <EmptyState
              icon={<MailOpen size={24} />}
              title={bodyMutation.isError ? t("errors.title") : t("mail.bodyUnavailable")}
              description={bodyMutation.isError
                ? t(`errors.${normalizeCommandError(bodyMutation.error).code}`, { defaultValue: t("common.unexpectedError") })
                : t("mail.bodyUnavailableDescription")}
              action={bodyMutation.isError
                ? <Button loading={bodyMutation.isPending} onClick={() => bodyMutation.mutate()}><Download size={14} />{t("mail.downloadBody")}</Button>
                : undefined}
            />
          </Stack>
        )}
      </Stack>

      {message.attachments.length ? (
        <Stack className="shrink-0 border-t border-border/70 bg-muted/20 px-5 py-3.5" gap="sm">
          <Inline><Paperclip size={15} /><LabelText>{t("mail.attachments")}</LabelText></Inline>
          <OverlayScrollArea
            intrinsic
            className="max-h-[168px]"
            trackClassName="right-0"
          >
            <Inline className="flex-wrap gap-2.5">
              {message.attachments.map((attachment) => (
                <MessageAttachment
                  key={attachment.id}
                  attachment={attachment}
                  opening={attachmentMutation.isPending && attachmentMutation.variables?.id === attachment.id}
                  saving={saveAttachmentMutation.isPending && saveAttachmentMutation.variables?.id === attachment.id}
                  revealing={revealAttachmentMutation.isPending && revealAttachmentMutation.variables?.id === attachment.id}
                  onOpen={() => attachmentMutation.mutate(attachment)}
                  onSaveAs={() => saveAttachmentMutation.mutate(attachment)}
                  onReveal={() => revealAttachmentMutation.mutate(attachment)}
                />
              ))}
            </Inline>
          </OverlayScrollArea>
        </Stack>
      ) : null}
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

function MailboxActionSubMenu({ icon, label, mailboxes, onSelect }: {
  icon: ReactNode;
  label: string;
  mailboxes: MailboxSummary[];
  onSelect: (mailboxId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>
        {icon}
        {label}
      </DropdownMenuSubTrigger>
      <DropdownMenuSubContent className="overflow-hidden">
        <OverlayScrollArea
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
      </DropdownMenuSubContent>
    </DropdownMenuSub>
  );
}

function AddressList({ label, addresses, onOpenContact, onEditContact }: {
  label: string;
  addresses: AddressPresentation[];
  onOpenContact?: (contactId: string) => void;
  onEditContact?: (contactId: string) => void;
}) {
  const { t } = useTranslation();
  const contentRef = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [overflowing, setOverflowing] = useState(false);
  const addressesKey = addresses.map((address) => `${address.email}\0${address.name ?? ""}`).join("\u0001");

  useEffect(() => setExpanded(false), [addressesKey]);
  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const measure = () => setOverflowing(content.scrollHeight > 28);
    measure();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(content);
    window.addEventListener("resize", measure);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [addressesKey]);

  return (
    <Inline className="select-text items-start gap-1.5 text-xs text-muted-foreground">
      <span className="shrink-0 py-1">{label}:</span>
      <div className={expanded ? "min-w-0 flex-1" : "min-w-0 max-h-7 flex-1 overflow-hidden"}>
        <div ref={contentRef} className="flex min-w-0 flex-wrap gap-1.5">
          {addresses.length ? addresses.map((address, index) => (
            <span key={`${address.email}-${index}`} className="inline-flex">
              <ContactIdentity
                address={address}
                onOpenContact={onOpenContact}
                onEditContact={onEditContact}
                focusable={expanded || !overflowing}
                tag
              />
            </span>
          )) : <span className="py-1">—</span>}
        </div>
      </div>
      {overflowing ? (
        <Button
          variant="ghost"
          size="sm"
          className="h-7 shrink-0 gap-1 px-2 text-xs"
          aria-expanded={expanded}
          onClick={() => setExpanded((value) => !value)}
        >
          {expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
          {expanded ? t("mail.hideRecipients") : t("mail.showRecipients")}
        </Button>
      ) : null}
    </Inline>
  );
}

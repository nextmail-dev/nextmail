import {
  Archive,
  CloudUpload,
  Copy,
  FilePenLine,
  FolderInput,
  Forward,
  Inbox,
  Mail,
  MailOpen,
  Paperclip,
  Reply,
  ReplyAll,
  Star,
  Trash2,
} from "lucide-react";
import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { forwardRef, useEffect, type HTMLAttributes, type ReactElement, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { MailboxSummary, MessageListItem } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { UnreadDot } from "@/components/ui/icon-tile";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { SearchField } from "@/components/ui/search-field";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";
import { cn } from "@/lib/utils";
import { formatMessageListTimestamp } from "./messageDate";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { mailQueryKeys, messageQueryKeys } from "./mail-query-keys";

interface MessageListPaneProps {
  accountId: string;
  mailboxId: string;
  mailbox?: MailboxSummary;
  mailboxes: MailboxSummary[];
  selectedMessageId: string;
  onSelect: (messageId: string) => void;
  onVisibleMessageIdsChange: (messageIds: string[]) => void;
  onMessageRemoved: (messageId: string) => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
}

export function MessageListPane({
  accountId,
  mailboxId,
  mailbox,
  mailboxes,
  selectedMessageId,
  onSelect,
  onVisibleMessageIdsChange,
  onMessageRemoved,
  searchQuery,
  onSearchChange,
}: MessageListPaneProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const normalizedSearch = searchQuery.trim();
  const debouncedSearch = useDebouncedValue(normalizedSearch, 250);
  const query = useInfiniteQuery({
    queryKey: debouncedSearch
      ? mailQueryKeys.messageSearch(accountId, mailboxId, debouncedSearch)
      : mailQueryKeys.messagesForMailbox(accountId, mailboxId),
    queryFn: ({ pageParam }) => debouncedSearch
      ? api.searchMessages(accountId, mailboxId, debouncedSearch, pageParam, 50)
      : api.listMessages(accountId, mailboxId, pageParam, 50),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: Boolean(accountId && mailboxId),
  });
  const allItems = query.data?.pages.flatMap((page) => page.items) ?? [];
  const items = allItems;
  const visibleMessageIds = items.map((message) => message.id);
  const visibleMessageKey = visibleMessageIds.join("\0");
  useEffect(() => {
    onVisibleMessageIdsChange(visibleMessageIds);
  }, [onVisibleMessageIdsChange, visibleMessageKey]);
  const operation = useMutation({
    mutationFn: async (input: MessageListOperation) => {
      const { message, kind, destination } = input;
      if (kind === "read") await api.setMessageRead(accountId, mailboxId, [message.id], message.unread);
      if (kind === "flag") await api.setMessageFlagged(accountId, mailboxId, [message.id], !message.flagged);
      if (kind === "move" && destination) await api.moveMessages(accountId, mailboxId, destination, [message.id]);
      if (kind === "copy" && destination) await api.copyMessages(accountId, mailboxId, destination, [message.id]);
      if (kind === "archive") await api.archiveMessages(accountId, mailboxId, [message.id]);
      if (kind === "delete") await api.deleteMessages(accountId, mailboxId, [message.id]);
      return input;
    },
    onSuccess: ({ kind, message }) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) });
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) });
      if (["move", "archive", "delete"].includes(kind)) onMessageRemoved(message.id);
    },
  });
  const composeOperation = useMutation({
    mutationFn: ({ message, action }: { message: MessageListItem; action: "reply" | "reply_all" | "forward" }) =>
      api.openMessageActionComposer(accountId, message.id, action),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: mailQueryKeys.drafts(accountId) }),
  });
  const editDraftOperation = useMutation({
    mutationFn: (message: MessageListItem) => api.openRemoteDraft(accountId, message.id),
  });
  const mailboxName = mailbox
    ? mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)
    : t("mail.messages");
  const actionError = operation.error ?? composeOperation.error ?? editDraftOperation.error;

  return (
    <Stack className="min-h-0 flex-1 bg-card" gap="none">
      <Stack className="px-6 pt-6 pb-5" gap="md">
        <Stack gap="xs">
          <Heading level={2} className="text-xl">{mailboxName}</Heading>
          <Text className="text-xs">
            {t("mail.folderSummary", { total: mailbox?.totalCount ?? allItems.length, unread: mailbox?.unreadCount ?? 0 })}
          </Text>
        </Stack>
        <SearchField
          className="h-11 w-full rounded-lg bg-muted px-4"
          value={searchQuery}
          placeholder={t("mail.searchPlaceholder")}
          clearLabel={t("mail.clearSearch")}
          maxLength={256}
          onValueChange={onSearchChange}
        />
      </Stack>
      {actionError ? <MessageListError error={actionError} /> : null}
      {items.length ? (
        <OverlayScrollArea alwaysVisible className="min-h-0 flex-1" viewportClassName="pr-3">
          {items.map((message) => (
            <MessageActionsContextMenu
              key={message.id}
              message={message}
              currentMailbox={mailbox}
              mailboxes={mailboxes}
              pending={operation.isPending || composeOperation.isPending || editDraftOperation.isPending}
              onCompose={(action) => composeOperation.mutate({ message, action })}
              onOperate={(kind, destination) => operation.mutate({ message, kind, destination })}
              onEditDraft={() => editDraftOperation.mutate(message)}
            >
              <MessageRow
                message={message}
                selected={message.id === selectedMessageId}
                yesterdayLabel={t("mail.yesterday")}
                noSubject={t("mail.noSubject")}
                starLabel={message.flagged ? t("mail.removeStar") : t("mail.addStar")}
                onContextMenu={() => onSelect(message.id)}
                onClick={() => {
                  onSelect(message.id === selectedMessageId ? "" : message.id);
                  if (message.unread) operation.mutate({ message, kind: "read" });
                }}
                onToggleFlag={() => operation.mutate({ message, kind: "flag" })}
              />
            </MessageActionsContextMenu>
          ))}
          {query.hasNextPage ? (
            <Button variant="ghost" className="mx-auto my-3" loading={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
              {t("mail.loadMore")}
            </Button>
          ) : null}
        </OverlayScrollArea>
      ) : query.isPending ? (
        <Stack className="m-auto items-center"><Spinner size={22} /></Stack>
      ) : query.isError ? (
        <MessageListError error={query.error} />
      ) : (
        <EmptyState
          icon={<Inbox size={24} />}
          title={debouncedSearch ? t("mail.noSearchResults") : t("mail.noMessages")}
          description={debouncedSearch ? t("mail.noSearchResultsDescription") : t("mail.noMessagesDescription")}
        />
      )}
    </Stack>
  );
}

interface MessageRowProps extends Omit<HTMLAttributes<HTMLDivElement>, "onClick"> {
  message: MessageListItem;
  selected: boolean;
  yesterdayLabel: string;
  noSubject: string;
  starLabel: string;
  onClick: () => void;
  onToggleFlag: () => void;
}

const MessageRow = forwardRef<HTMLDivElement, MessageRowProps>(function MessageRow({
  message,
  selected,
  yesterdayLabel,
  noSubject,
  starLabel,
  onClick,
  onToggleFlag,
  className,
  ...props
}, ref) {
  const sender = message.from[0];
  const date = formatMessageListTimestamp(message.receivedAt, yesterdayLabel);
  return (
    <Inline
      ref={ref}
      className={cn(selected
        ? "group relative gap-0 bg-selection before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:rounded-r-full before:bg-primary"
        : "group relative gap-0 bg-card transition-colors hover:bg-muted/75", className)}
      {...props}
    >
      <Button
        variant="ghost"
        className="h-auto min-w-0 flex-1 items-start rounded-none bg-transparent px-6 py-4 pr-12 text-left hover:bg-transparent"
        onClick={onClick}
      >
        <Stack className="min-w-0 flex-1" gap="xs">
          <Inline className="w-full">
            {message.unread ? <UnreadDot /> : null}
            <Text className="min-w-0 flex-1 truncate font-semibold text-foreground">{sender?.name || sender?.email || "—"}</Text>
            <Text className="shrink-0 text-[length:var(--ui-font-caption)]">{date}</Text>
          </Inline>
          <Text className="truncate text-[length:var(--ui-font-control)] font-medium text-foreground">{message.subject || noSubject}</Text>
          <Inline className="w-full text-muted-foreground">
            <Text className="min-w-0 flex-1 truncate text-xs">{message.preview}</Text>
            {message.hasAttachments ? <Paperclip size={13} /> : null}
            {message.pendingOperation ? <CloudUpload size={13} /> : null}
          </Inline>
        </Stack>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="absolute right-3 top-1/2 size-8 -translate-y-1/2 bg-transparent hover:bg-foreground/7"
        aria-label={starLabel}
        title={starLabel}
        onClick={onToggleFlag}
      >
        <Star size={16} className={message.flagged ? "fill-current text-[#f2b84b]" : undefined} />
      </Button>
    </Inline>
  );
});

type MessageListOperationKind = "read" | "flag" | "move" | "copy" | "archive" | "delete";

interface MessageListOperation {
  message: MessageListItem;
  kind: MessageListOperationKind;
  destination?: string;
}

function MessageActionsContextMenu({
  message,
  currentMailbox,
  mailboxes,
  pending,
  onCompose,
  onOperate,
  onEditDraft,
  children,
}: {
  message: MessageListItem;
  currentMailbox?: MailboxSummary;
  mailboxes: MailboxSummary[];
  pending: boolean;
  onCompose: (action: "reply" | "reply_all" | "forward") => void;
  onOperate: (kind: MessageListOperationKind, destination?: string) => void;
  onEditDraft: () => void;
  children: ReactElement;
}) {
  const { t } = useTranslation();
  const destinations = mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== message.mailboxId);
  const canArchive = mailboxes.some((mailbox) => mailbox.role === "archive" && mailbox.id !== message.mailboxId);
  const isDraft = currentMailbox?.role === "drafts";
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem disabled={pending} onSelect={() => onCompose("reply")}><Reply size={16} />{t("mail.reply")}</ContextMenuItem>
        <ContextMenuItem disabled={pending} onSelect={() => onCompose("reply_all")}><ReplyAll size={16} />{t("mail.replyAll")}</ContextMenuItem>
        <ContextMenuItem disabled={pending} onSelect={() => onCompose("forward")}><Forward size={16} />{t("mail.forward")}</ContextMenuItem>
        {isDraft ? <ContextMenuItem disabled={pending} onSelect={onEditDraft}><FilePenLine size={16} />{t("mail.editDraft")}</ContextMenuItem> : null}
        <ContextMenuSeparator />
        <ContextMenuItem disabled={pending} onSelect={() => onOperate("read")}>
          {message.unread ? <MailOpen size={16} /> : <Mail size={16} />}
          {message.unread ? t("mail.markRead") : t("mail.markUnread")}
        </ContextMenuItem>
        <ContextMenuItem disabled={pending} onSelect={() => onOperate("flag")}>
          <Star size={16} className={message.flagged ? "fill-current text-[#f2b84b]" : undefined} />
          {message.flagged ? t("mail.removeStar") : t("mail.addStar")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        {canArchive ? (
          <ContextMenuItem disabled={pending} onSelect={() => onOperate("archive")}><Archive size={16} />{t("mail.archive")}</ContextMenuItem>
        ) : null}
        <MailboxContextSubmenu
          icon={<FolderInput size={16} />}
          label={t("mail.moveTo")}
          mailboxes={destinations}
          disabled={pending}
          onSelect={(destination) => onOperate("move", destination)}
        />
        <MailboxContextSubmenu
          icon={<Copy size={16} />}
          label={t("mail.copyTo")}
          mailboxes={destinations}
          disabled={pending}
          onSelect={(destination) => onOperate("copy", destination)}
        />
        <ContextMenuItem className="text-destructive focus:bg-destructive/10 focus:text-destructive" disabled={pending} onSelect={() => onOperate("delete")}>
          <Trash2 size={16} />{t("mail.delete")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function MailboxContextSubmenu({
  icon,
  label,
  mailboxes,
  disabled,
  onSelect,
}: {
  icon: ReactNode;
  label: string;
  mailboxes: MailboxSummary[];
  disabled: boolean;
  onSelect: (mailboxId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <ContextMenuSub>
      <ContextMenuSubTrigger disabled={disabled || !mailboxes.length}>{icon}{label}</ContextMenuSubTrigger>
      <ContextMenuSubContent>
        <OverlayScrollArea
          className="-mr-1.5"
          viewportClassName="pr-1.5"
          trackClassName="right-0"
          style={{ height: `${Math.min(276, mailboxes.length * 36)}px`, maxHeight: "calc(100vh - 32px)" }}
        >
          {mailboxes.map((mailbox) => (
            <ContextMenuItem key={mailbox.id} onSelect={() => onSelect(mailbox.id)}>
              {mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)}
            </ContextMenuItem>
          ))}
        </OverlayScrollArea>
      </ContextMenuSubContent>
    </ContextMenuSub>
  );
}

function MessageListError({ error }: { error: unknown }) {
  const { t } = useTranslation();
  const normalized = normalizeCommandError(error);
  return <Alert className="m-4" tone="danger" title={t("errors.title")}>{t(`errors.${normalized.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>;
}

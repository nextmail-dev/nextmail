import {
  Archive,
  CloudUpload,
  Copy,
  ExternalLink,
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
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { forwardRef, memo, useEffect, useState, type HTMLAttributes, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type ReactElement, type ReactNode, type UIEvent } from "react";
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
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { SearchField } from "@/components/ui/search-field";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";
import { useListSelection } from "@/components/ui/use-list-selection";
import { ContactIdentity } from "@/features/contacts/ContactIdentity";
import { cn } from "@/lib/utils";
import { formatMessageListTimestamp } from "./messageDate";
import { mailQueryKeys, messageQueryKeys, STARRED_MAILBOX_ID, UNREAD_MAILBOX_ID } from "./mail-query-keys";

interface MessageListPaneProps {
  accountId: string;
  mailboxId: string;
  mailbox?: MailboxSummary;
  mailboxes: MailboxSummary[];
  selectedMessageId: string;
  onSelect: (messageId: string, mailboxId: string) => void;
  onVisibleMessageIdsChange: (messageIds: string[]) => void;
  onMessagesRemoved: (messageIds: string[]) => void;
  onOpenContact?: (contactId: string) => void;
  onEditContact?: (contactId: string) => void;
  searchQuery: string;
  submittedSearchQuery: string;
  onSearchChange: (value: string) => void;
  onSearchSubmit: (value: string) => void;
}

function MessageListPaneBase({
  accountId,
  mailboxId,
  mailbox,
  mailboxes,
  selectedMessageId,
  onSelect,
  onVisibleMessageIdsChange,
  onMessagesRemoved,
  onOpenContact,
  onEditContact,
  searchQuery,
  submittedSearchQuery,
  onSearchChange,
  onSearchSubmit,
}: MessageListPaneProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const unreadView = mailboxId === UNREAD_MAILBOX_ID;
  const starredView = mailboxId === STARRED_MAILBOX_ID;
  const activeSearch = submittedSearchQuery.trim();
  const [retainedUnreadItems, setRetainedUnreadItems] = useState<Array<{
    index: number;
    message: MessageListItem;
  }>>([]);
  const readingPreferences = useQuery({
    queryKey: ["reading-preferences"],
    queryFn: api.getReadingPreferences,
  });
  const query = useInfiniteQuery({
    queryKey: unreadView
      ? mailQueryKeys.unreadMessages(accountId)
      : starredView
      ? mailQueryKeys.starredMessages(accountId)
      : activeSearch
      ? mailQueryKeys.messageSearch(accountId, mailboxId, activeSearch)
      : mailQueryKeys.messagesForMailbox(accountId, mailboxId),
    queryFn: ({ pageParam }) => unreadView
      ? api.listUnreadMessages(accountId, pageParam, 50)
      : starredView
      ? api.listStarredMessages(accountId, pageParam, 50)
      : activeSearch
      ? api.searchMessages(accountId, mailboxId, activeSearch, pageParam, 50)
      : api.listMessages(accountId, mailboxId, pageParam, 50),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: Boolean(accountId && mailboxId),
  });
  const allItems = query.data?.pages.flatMap((page) => page.items) ?? [];
  const items = unreadView
    ? retainedUnreadItems.reduce((current, retained) => {
      const existingIndex = current.findIndex((message) =>
        message.id === retained.message.id && message.mailboxId === retained.message.mailboxId,
      );
      const next = [...current];
      if (existingIndex >= 0) next[existingIndex] = retained.message;
      else next.splice(Math.max(0, Math.min(retained.index, next.length)), 0, retained.message);
      return next;
    }, allItems)
    : allItems;
  const visibleMessageIds = items.map((message) => message.id);
  const visibleMessageKey = visibleMessageIds.join("\0");
  useEffect(() => {
    onVisibleMessageIdsChange(visibleMessageIds);
  }, [onVisibleMessageIdsChange, visibleMessageKey]);
  const selection = useListSelection({
    itemIds: visibleMessageIds,
    primaryId: selectedMessageId,
    resetKey: `${accountId}:${mailboxId}:${activeSearch}`,
    onPrimaryChange: (messageId) => onSelect(
      messageId,
      items.find((message) => message.id === messageId)?.mailboxId ?? "",
    ),
  });
  const selectedMessageIdSet = new Set(selection.orderedSelectedIds);
  const selectedMessages = items.filter((message) => selectedMessageIdSet.has(message.id));
  const operation = useMutation({
    mutationFn: async (input: MessageListOperation) => {
      const { messages, reference, kind, destination } = input;
      const groups = new Map<string, MessageListItem[]>();
      for (const message of messages) {
        groups.set(message.mailboxId, [...(groups.get(message.mailboxId) ?? []), message]);
      }
      await Promise.all([...groups].map(async ([sourceMailboxId, sourceMessages]) => {
        const messageIds = sourceMessages.map((message) => message.id);
        if (kind === "read") await api.setMessageRead(accountId, sourceMailboxId, messageIds, reference.unread);
        if (kind === "flag") await api.setMessageFlagged(accountId, sourceMailboxId, messageIds, !reference.flagged);
        if (kind === "move" && destination) await api.moveMessages(accountId, sourceMailboxId, destination, messageIds);
        if (kind === "copy" && destination) await api.copyMessages(accountId, sourceMailboxId, destination, messageIds);
        if (kind === "archive") await api.archiveMessages(accountId, sourceMailboxId, messageIds);
        if (kind === "delete") await api.deleteMessages(accountId, sourceMailboxId, messageIds);
      }));
      return input;
    },
    onSuccess: ({ kind, messages, reference }) => {
      if (unreadView && kind === "read") {
        setRetainedUnreadItems((current) => {
          const operated = new Set(messages.map((message) => `${message.mailboxId}\0${message.id}`));
          return [
            ...current.filter(({ message }) => !operated.has(`${message.mailboxId}\0${message.id}`)),
            ...messages.map((message) => ({
              index: items.findIndex((item) => item.id === message.id && item.mailboxId === message.mailboxId),
              message: { ...message, unread: !reference.unread },
            })),
          ].sort((left, right) => left.index - right.index);
        });
      }
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) });
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) });
      if (["move", "archive", "delete"].includes(kind)
        || starredView && kind === "flag") {
        selection.clear();
        onMessagesRemoved(messages.map((message) => message.id));
      }
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
  const previewOperation = useMutation({
    mutationFn: (message: MessageListItem) => api.openMessagePreviewWindow(accountId, message.mailboxId, message.id),
  });
  const mailboxName = mailbox
    ? mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)
    : unreadView ? t("mailboxNames.unread")
    : starredView ? t("mailboxNames.starred")
    : t("mail.messages");
  const actionError = operation.error ?? composeOperation.error ?? editDraftOperation.error ?? previewOperation.error;
  const autoLoadMore = readingPreferences.data?.autoLoadMoreMessages ?? true;

  function loadNextPageNearEnd(event: UIEvent<HTMLDivElement>) {
    if (!autoLoadMore || !query.hasNextPage || query.isFetchingNextPage) return;
    const viewport = event.currentTarget;
    if (viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= 160) {
      void query.fetchNextPage();
    }
  }

  function handleMessageArrowKey(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    const nextRow = event.key === "ArrowDown"
      ? event.currentTarget.nextElementSibling
      : event.currentTarget.previousElementSibling;
    if (!(nextRow instanceof HTMLElement)) return;
    const messageId = nextRow.dataset.messageSelectionId;
    const message = items.find((item) => item.id === messageId);
    if (!message) return;
    selection.select(message.id, { ctrlKey: false, metaKey: false, shiftKey: false });
    if (message.unread) operation.mutate({ messages: [message], reference: message, kind: "read" });
    nextRow.querySelector<HTMLElement>("[data-message-select-button]")?.focus();
  }

  return (
    <Stack className="min-h-0 flex-1 bg-card" gap="none">
      <Stack className="px-5 pt-5 pb-4" gap="sm">
        <Stack gap="xs">
          <Heading level={2} className="text-lg">{mailboxName}</Heading>
          <Text className="text-xs">
            {selection.orderedSelectedIds.length > 1
              ? t("mail.selectedCount", { count: selection.orderedSelectedIds.length })
              : unreadView
                ? t("mail.unreadSummary", { unread: mailboxes.reduce((total, item) => total + item.unreadCount, 0) })
                : t("mail.folderSummary", { total: mailbox?.totalCount ?? allItems.length, unread: mailbox?.unreadCount ?? 0 })}
          </Text>
        </Stack>
          {unreadView || starredView ? null : <SearchField
          className="h-10 w-full rounded-lg bg-muted px-3.5"
          value={searchQuery}
          placeholder={t("mail.searchPlaceholder")}
          clearLabel={t("mail.clearSearch")}
          submitLabel={t("mail.searchCurrentFolder")}
          maxLength={256}
          aria-label={t("mail.searchCurrentFolder")}
          onValueChange={(value) => {
            onSearchChange(value);
            if (!value.trim()) onSearchSubmit("");
          }}
          onSubmit={() => {
            const query = searchQuery.trim();
            onSearchChange(query);
            onSearchSubmit(query);
          }}
        />}
      </Stack>
      {actionError ? <MessageListError error={actionError} /> : null}
      {items.length ? (
        <OverlayScrollArea
          key={`${accountId}:${mailboxId}`}
          className="min-h-0 flex-1"
          trackClassName="right-2 w-3"
          onViewportScroll={loadNextPageNearEnd}
        >
          {items.map((message, index) => {
            const operationMessages = selectedMessageIdSet.has(message.id) ? selectedMessages : [message];
            return (
              <MessageActionsContextMenu
                key={message.id}
                message={message}
                selectionCount={operationMessages.length}
                currentMailbox={unreadView || starredView
                  ? mailboxes.find((item) => item.id === message.mailboxId)
                  : mailbox}
                mailboxes={mailboxes}
                pending={operation.isPending || composeOperation.isPending || editDraftOperation.isPending || previewOperation.isPending}
                onCompose={(action) => composeOperation.mutate({ message, action })}
                onOperate={(kind, destination) => operation.mutate({ messages: operationMessages, reference: message, kind, destination })}
                onEditDraft={() => editDraftOperation.mutate(message)}
                onOpenInNewWindow={() => previewOperation.mutate(message)}
              >
                <MessageRow
                  message={message}
                  selected={selection.isSelected(message.id)}
                  divider={index < items.length - 1}
                  yesterdayLabel={t("mail.yesterday")}
                  noSubject={t("mail.noSubject")}
                  starLabel={message.flagged ? t("mail.removeStar") : t("mail.addStar")}
                  readLabel={message.unread ? t("mail.markRead") : t("mail.markUnread")}
                  pending={operation.isPending}
                  onContextMenu={() => selection.selectForContextMenu(message.id)}
                  data-message-selection-id={message.id}
                  onKeyDown={handleMessageArrowKey}
                  onClick={(event) => {
                    if (event.detail > 1) return;
                    selection.select(message.id, event);
                    if (message.unread) operation.mutate({ messages: [message], reference: message, kind: "read" });
                  }}
                  onOpenInNewWindow={() => previewOperation.mutate(message)}
                  onToggleRead={() => operation.mutate({ messages: [message], reference: message, kind: "read" })}
                  onToggleFlag={() => operation.mutate({ messages: [message], reference: message, kind: "flag" })}
                  onOpenContact={onOpenContact}
                  onEditContact={onEditContact}
                />
              </MessageActionsContextMenu>
            );
          })}
          {query.hasNextPage && !autoLoadMore ? (
            <Button variant="ghost" className="mx-auto my-3" loading={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
              {t("mail.loadMore")}
            </Button>
          ) : query.isFetchingNextPage ? (
            <span className="mx-auto my-3"><Spinner size={18} /></span>
          ) : null}
        </OverlayScrollArea>
      ) : query.isPending ? (
        <Stack className="m-auto items-center"><Spinner size={22} /></Stack>
      ) : query.isError ? (
        <MessageListError error={query.error} />
      ) : (
        <EmptyState
          icon={<Inbox size={24} />}
          title={activeSearch ? t("mail.noSearchResults") : t("mail.noMessages")}
          description={activeSearch ? t("mail.noSearchResultsDescription") : t("mail.noMessagesDescription")}
        />
      )}
    </Stack>
  );
}

// Memoized so MainShell state changes (e.g. dragging a splitter) don't
// re-render the message list.
export const MessageListPane = memo(MessageListPaneBase);

interface MessageRowProps extends Omit<HTMLAttributes<HTMLDivElement>, "onClick"> {
  message: MessageListItem;
  selected: boolean;
  divider: boolean;
  yesterdayLabel: string;
  noSubject: string;
  starLabel: string;
  readLabel: string;
  pending: boolean;
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  onOpenInNewWindow: () => void;
  onToggleRead: () => void;
  onToggleFlag: () => void;
  onOpenContact?: (contactId: string) => void;
  onEditContact?: (contactId: string) => void;
}

const MessageRow = forwardRef<HTMLDivElement, MessageRowProps>(function MessageRow({
  message,
  selected,
  divider,
  yesterdayLabel,
  noSubject,
  starLabel,
  readLabel,
  pending,
  onClick,
  onOpenInNewWindow,
  onToggleRead,
  onToggleFlag,
  onOpenContact,
  onEditContact,
  className,
  ...props
}, ref) {
  const sender = message.from[0];
  const date = formatMessageListTimestamp(message.receivedAt, yesterdayLabel);
  return (
    <Inline
      ref={ref}
      className={cn(
        "group relative gap-0 after:pointer-events-none after:absolute after:inset-x-5 after:bottom-0 after:h-px after:bg-border/80 after:content-['']",
        !divider && "after:hidden",
        selected
          ? "bg-selection before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-primary"
          : message.unread
            ? "bg-primary/[0.035] transition-colors hover:bg-primary/[0.065]"
            : "bg-card transition-colors hover:bg-muted/65",
        className,
      )}
      {...props}
    >
      <Button
        variant="ghost"
        size="icon"
        className="group/read-state absolute top-3.5 left-3 z-10 size-5 rounded-full bg-transparent hover:bg-transparent"
        aria-label={readLabel}
        title={readLabel}
        aria-pressed={message.unread}
        disabled={pending}
        onClick={(event) => {
          event.stopPropagation();
          onToggleRead();
        }}
      >
        <span className={cn(
          "size-2 rounded-full transition-shadow group-hover/read-state:ring-2 group-hover/read-state:ring-foreground/10",
          message.unread ? "bg-primary" : "border border-foreground/15",
        )} />
      </Button>
      <Button
        variant="ghost"
        aria-pressed={selected}
        data-message-select-button="true"
        className="h-auto min-w-0 flex-1 items-start rounded-none bg-transparent py-2.5 pr-12 pl-10 text-left hover:bg-transparent"
        onClick={onClick}
        onDoubleClick={(event) => {
          event.preventDefault();
          onOpenInNewWindow();
        }}
      >
        <Stack className="min-w-0 flex-1" gap="xs">
          <Inline className="w-full">
            {sender ? (
              <ContactIdentity address={sender} className="min-w-0 flex-1" onOpenContact={onOpenContact} onEditContact={onEditContact} focusable={false}>
                <span className={cn(
                  "block truncate text-sm text-foreground",
                  message.unread ? "font-semibold" : "font-medium text-foreground/80",
                )}>{sender.name || sender.email}</span>
              </ContactIdentity>
            ) : <Text className={cn("min-w-0 flex-1 truncate text-foreground", message.unread ? "font-semibold" : "font-medium text-foreground/80")}>—</Text>}
            <Text className={cn("shrink-0 text-[length:var(--ui-font-caption)]", message.unread && "font-medium text-foreground/75")}>{date}</Text>
          </Inline>
          <Text className={cn(
            "truncate text-[length:var(--ui-font-control)] text-foreground",
            message.unread ? "font-semibold" : "font-normal text-foreground/85",
          )}>{message.subject || noSubject}</Text>
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
        className="absolute right-4 top-1/2 size-8 -translate-y-1/2 bg-transparent hover:bg-foreground/7"
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
  messages: MessageListItem[];
  reference: MessageListItem;
  kind: MessageListOperationKind;
  destination?: string;
}

function MessageActionsContextMenu({
  message,
  selectionCount,
  currentMailbox,
  mailboxes,
  pending,
  onCompose,
  onOperate,
  onEditDraft,
  onOpenInNewWindow,
  children,
}: {
  message: MessageListItem;
  selectionCount: number;
  currentMailbox?: MailboxSummary;
  mailboxes: MailboxSummary[];
  pending: boolean;
  onCompose: (action: "reply" | "reply_all" | "forward") => void;
  onOperate: (kind: MessageListOperationKind, destination?: string) => void;
  onEditDraft: () => void;
  onOpenInNewWindow: () => void;
  children: ReactElement;
}) {
  const { t } = useTranslation();
  const destinations = mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== message.mailboxId);
  const canArchive = mailboxes.some((mailbox) => mailbox.role === "archive" && mailbox.id !== message.mailboxId);
  const isDraft = currentMailbox?.role === "drafts";
  const single = selectionCount === 1;
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        {single ? <ContextMenuItem disabled={pending} onSelect={onOpenInNewWindow}><ExternalLink size={16} />{t("mail.openInNewWindow")}</ContextMenuItem> : null}
        {single && !isDraft ? (
          <>
            <ContextMenuItem disabled={pending} onSelect={() => onCompose("reply")}><Reply size={16} />{t("mail.reply")}</ContextMenuItem>
            <ContextMenuItem disabled={pending} onSelect={() => onCompose("reply_all")}><ReplyAll size={16} />{t("mail.replyAll")}</ContextMenuItem>
            <ContextMenuItem disabled={pending} onSelect={() => onCompose("forward")}><Forward size={16} />{t("mail.forward")}</ContextMenuItem>
          </>
        ) : null}
        {single && isDraft ? <ContextMenuItem disabled={pending} onSelect={onEditDraft}><FilePenLine size={16} />{t("mail.editDraft")}</ContextMenuItem> : null}
        {single ? <ContextMenuSeparator /> : null}
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

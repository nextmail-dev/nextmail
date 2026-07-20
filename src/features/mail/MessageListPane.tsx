import { CloudUpload, Inbox, Paperclip, Star } from "lucide-react";
import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { MailboxSummary, MessageListItem } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { UnreadDot } from "@/components/ui/icon-tile";
import { Inline, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { SearchField } from "@/components/ui/search-field";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";
import { formatMessageListTimestamp } from "./messageDate";
import { mailQueryKeys, messageQueryKeys } from "./mail-query-keys";

interface MessageListPaneProps {
  accountId: string;
  mailboxId: string;
  mailbox?: MailboxSummary;
  selectedMessageId: string;
  onSelect: (messageId: string) => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
}

export function MessageListPane({
  accountId,
  mailboxId,
  mailbox,
  selectedMessageId,
  onSelect,
  searchQuery,
  onSearchChange,
}: MessageListPaneProps) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const query = useInfiniteQuery({
    queryKey: mailQueryKeys.messagesForMailbox(accountId, mailboxId),
    queryFn: ({ pageParam }) => api.listMessages(accountId, mailboxId, pageParam, 50),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: Boolean(accountId && mailboxId),
  });
  const allItems = query.data?.pages.flatMap((page) => page.items) ?? [];
  const normalizedSearch = searchQuery.trim().toLocaleLowerCase(i18n.language);
  const items = normalizedSearch
    ? allItems.filter((message) => [
      message.subject,
      message.preview,
      ...message.from.flatMap((address) => [address.name ?? "", address.email]),
    ].some((value) => value.toLocaleLowerCase(i18n.language).includes(normalizedSearch)))
    : allItems;
  const operation = useMutation({
    mutationFn: async ({ message, kind }: { message: MessageListItem; kind: "read" | "flag" }) => {
      if (kind === "read") await api.setMessageRead(accountId, mailboxId, [message.id], true);
      else await api.setMessageFlagged(accountId, mailboxId, [message.id], !message.flagged);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForMailbox(accountId, mailboxId) });
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) });
    },
  });
  const mailboxName = mailbox
    ? mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)
    : t("mail.messages");

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
          onValueChange={onSearchChange}
        />
      </Stack>
      {items.length ? (
        <OverlayScrollArea className="min-h-0 flex-1" viewportClassName="pr-3">
          {items.map((message) => (
            <MessageRow
              key={message.id}
              message={message}
              selected={message.id === selectedMessageId}
              yesterdayLabel={t("mail.yesterday")}
              noSubject={t("mail.noSubject")}
              starLabel={message.flagged ? t("mail.removeStar") : t("mail.addStar")}
              onClick={() => {
                onSelect(message.id);
                if (message.unread) operation.mutate({ message, kind: "read" });
              }}
              onToggleFlag={() => operation.mutate({ message, kind: "flag" })}
            />
          ))}
          {query.hasNextPage ? (
            <Button variant="ghost" className="mx-auto my-3" loading={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
              {t("mail.loadMore")}
            </Button>
          ) : null}
        </OverlayScrollArea>
      ) : query.isPending ? (
        <Stack className="m-auto items-center"><Spinner size={22} /></Stack>
      ) : query.isError || operation.isError ? (
        <MessageListError error={query.error ?? operation.error} />
      ) : (
        <EmptyState
          icon={<Inbox size={24} />}
          title={normalizedSearch ? t("mail.noSearchResults") : t("mail.noMessages")}
          description={normalizedSearch ? t("mail.noSearchResultsDescription") : t("mail.noMessagesDescription")}
        />
      )}
    </Stack>
  );
}

function MessageRow({
  message,
  selected,
  yesterdayLabel,
  noSubject,
  starLabel,
  onClick,
  onToggleFlag,
}: {
  message: MessageListItem;
  selected: boolean;
  yesterdayLabel: string;
  noSubject: string;
  starLabel: string;
  onClick: () => void;
  onToggleFlag: () => void;
}) {
  const sender = message.from[0];
  const date = formatMessageListTimestamp(message.receivedAt, yesterdayLabel);
  return (
    <Inline className={selected
      ? "group relative gap-0 bg-selection before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:rounded-r-full before:bg-primary"
      : "group relative gap-0 bg-card transition-colors hover:bg-muted/75"}>
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
}

function MessageListError({ error }: { error: unknown }) {
  const { t } = useTranslation();
  const normalized = normalizeCommandError(error);
  return <Alert className="m-4" tone="danger" title={t("errors.title")}>{t(`errors.${normalized.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>;
}

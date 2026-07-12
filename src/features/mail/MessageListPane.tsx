import { Inbox, Paperclip, Star } from "lucide-react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { MessageListItem } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Alert } from "@/components/ui/alert";
import { EmptyState } from "@/components/ui/empty-state";
import { UnreadDot } from "@/components/ui/icon-tile";
import { Inline, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";

interface MessageListPaneProps {
  accountId: string;
  mailboxId: string;
  selectedMessageId: string;
  onSelect: (messageId: string) => void;
  searchQuery: string;
}

export function MessageListPane({
  accountId,
  mailboxId,
  selectedMessageId,
  onSelect,
  searchQuery,
}: MessageListPaneProps) {
  const { t, i18n } = useTranslation();
  const query = useInfiniteQuery({
    queryKey: ["messages", accountId, mailboxId],
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

  return (
    <Stack className="min-h-0 flex-1" gap="xs">
      <Inline className="min-h-14 border-b border-border px-4">
        <Heading level={2}>{t("mail.messages")}</Heading>
        {query.isFetching ? <Inline className="ml-auto"><Spinner size={15} /></Inline> : null}
      </Inline>
      {items.length ? (
        <Stack className="min-h-0 flex-1 gap-0 overflow-auto">
          {items.map((message) => (
            <MessageRow
              key={message.id}
              message={message}
              selected={message.id === selectedMessageId}
              locale={i18n.language}
              noSubject={t("mail.noSubject")}
              onClick={() => onSelect(message.id)}
            />
          ))}
          {query.hasNextPage ? (
            <Button
              variant="ghost"
              className="mx-auto my-2"
              loading={query.isFetchingNextPage}
              onClick={() => void query.fetchNextPage()}
            >
              {t("mail.loadMore")}
            </Button>
          ) : null}
        </Stack>
      ) : query.isPending ? (
        <Stack className="m-auto items-center"><Spinner size={22} /></Stack>
      ) : query.isError ? (
        <MessageListError error={query.error} />
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
  locale,
  noSubject,
  onClick,
}: {
  message: MessageListItem;
  selected: boolean;
  locale: string;
  noSubject: string;
  onClick: () => void;
}) {
  const sender = message.from[0];
  const date = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric" }).format(
    new Date(message.receivedAt * 1000),
  );
  return (
    <Button
      variant="list"
      className={
        selected
          ? "h-auto w-full items-start rounded-none bg-accent px-4 py-3.5 text-left"
          : "h-auto w-full items-start rounded-none px-4 py-3.5 text-left"
      }
      onClick={onClick}
    >
      <Stack className="min-w-0 flex-1" gap="xs">
        <Inline className="w-full">
          {message.unread ? <UnreadDot /> : null}
          <Text className="min-w-0 flex-1 truncate font-semibold text-foreground">
            {sender?.name || sender?.email || "—"}
          </Text>
          <Text className="shrink-0 text-[11px]">{date}</Text>
        </Inline>
        <Text className="truncate text-[13px] font-medium text-foreground">
          {message.subject || noSubject}
        </Text>
        <Inline className="w-full text-muted-foreground">
          <Text className="min-w-0 flex-1 truncate text-xs">{message.preview}</Text>
          {message.hasAttachments ? <Paperclip size={13} /> : null}
          {message.flagged ? <Star size={13} className="fill-current" /> : null}
        </Inline>
      </Stack>
    </Button>
  );
}

function MessageListError({ error }: { error: unknown }) {
  const { t } = useTranslation();
  const normalized = normalizeCommandError(error);
  return (
    <Alert className="m-4" tone="danger" title={t("errors.title")}>
      {t(`errors.${normalized.code}`, { defaultValue: t("common.unexpectedError") })}
    </Alert>
  );
}

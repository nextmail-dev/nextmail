import { CloudUpload, Inbox, Paperclip, Star } from "lucide-react";
import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
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
  const queryClient = useQueryClient();
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
  const operation = useMutation({
    mutationFn: async ({ message, kind }: { message: MessageListItem; kind: "read" | "flag" }) => {
      if (kind === "read") {
        await api.setMessageRead(accountId, mailboxId, [message.id], true);
      } else {
        await api.setMessageFlagged(accountId, mailboxId, [message.id], !message.flagged);
      }
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["mailboxes", accountId] });
      void queryClient.invalidateQueries({ queryKey: ["messages", accountId, mailboxId] });
      void queryClient.invalidateQueries({ queryKey: ["message", accountId] });
    },
  });

  return (
    <Stack className="min-h-0 flex-1" gap="none">
      <Inline className="min-h-14 border-b border-border px-4">
        <Heading level={2}>{t("mail.messages")}</Heading>
      </Inline>
      {items.length ? (
        <Stack className="min-h-0 flex-1 overflow-auto" gap="none">
          {items.map((message) => (
            <MessageRow
              key={message.id}
              message={message}
              selected={message.id === selectedMessageId}
              locale={i18n.language}
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
  locale,
  noSubject,
  starLabel,
  onClick,
  onToggleFlag,
}: {
  message: MessageListItem;
  selected: boolean;
  locale: string;
  noSubject: string;
  starLabel: string;
  onClick: () => void;
  onToggleFlag: () => void;
}) {
  const sender = message.from[0];
  const date = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric" }).format(
    new Date(message.receivedAt * 1000),
  );
  return (
    <Inline className={selected
      ? "group gap-0 border-b border-border bg-accent transition-colors"
      : "group gap-0 border-b border-border transition-colors hover:bg-accent/70"}>
      <Button
        variant="ghost"
        className="h-auto min-w-0 flex-1 items-start rounded-none px-4 py-3.5 text-left hover:bg-transparent"
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
            {message.pendingOperation ? <CloudUpload size={13} /> : null}
          </Inline>
        </Stack>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="mr-2 size-8 self-center bg-transparent hover:bg-accent"
        aria-label={starLabel}
        onClick={onToggleFlag}
      >
        <Star size={15} className={message.flagged ? "fill-current text-primary" : undefined} />
      </Button>
    </Inline>
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

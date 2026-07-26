import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import type {
  MessageListItem,
  MessageListPage,
  NotificationNavigationTarget,
} from "@/app/types";
import { reportCaughtError } from "@/app/errorReporting";
import { mailQueryKeys, messageQueryKeys } from "../mail-query-keys";

interface SentNotice {
  id: string;
  subject: string;
}

interface UseMailRuntimeEventsOptions {
  selectedAccountId: string;
  selectedMailboxId: string;
  onSent: (notice: SentNotice) => void;
  onNavigate: (target: NotificationNavigationTarget) => void;
}

interface MailboxRefreshQueue {
  pending: number;
  running: boolean;
}

interface MessageListData {
  pages: MessageListPage[];
  pageParams: (string | null)[];
}

const MESSAGE_PAGE_SIZE = 50;

// Inserts a freshly synced message into the first page of the infinite query
// cache in the same order the server returns (receivedAt desc, then id desc),
// so each message appears as it arrives instead of waiting for a snapshot
// refetch that may already read a whole burst of committed messages.
function applyArrivedMessage(
  data: MessageListData | undefined,
  item: MessageListItem,
): MessageListData | undefined {
  if (!data || data.pages.length === 0) return data;
  const firstPage = data.pages[0];
  const items = firstPage.items;
  const existingIndex = items.findIndex((message) => message.id === item.id);
  if (existingIndex !== -1) {
    const nextItems = items.map((message, index) =>
      index === existingIndex ? item : message,
    );
    return { ...data, pages: [{ ...firstPage, items: nextItems }, ...data.pages.slice(1)] };
  }
  let insertAt = items.findIndex(
    (message) =>
      message.receivedAt < item.receivedAt ||
      (message.receivedAt === item.receivedAt && message.id < item.id),
  );
  if (insertAt === -1) insertAt = items.length;
  // The message belongs beyond the first page's window; leave it for a refetch
  // or "load more" so cursors stay consistent.
  if (insertAt >= MESSAGE_PAGE_SIZE && firstPage.nextCursor !== null) {
    return data;
  }
  let nextItems = [...items.slice(0, insertAt), item, ...items.slice(insertAt)];
  let nextCursor = firstPage.nextCursor;
  // Cap the first page at the page size. Without this, a long sync grows the
  // first page without bound (thousands of rows) and every arriving message
  // re-renders the whole list - the UI freezes once a folder has a few hundred
  // messages. Items pushed past the boundary are re-fetched via "load more"
  // using the new cursor.
  if (nextItems.length > MESSAGE_PAGE_SIZE) {
    nextItems = nextItems.slice(0, MESSAGE_PAGE_SIZE);
    const last = nextItems[nextItems.length - 1];
    nextCursor = `${last.receivedAt}:${last.id}`;
  }
  return {
    ...data,
    pages: [{ ...firstPage, items: nextItems, nextCursor }, ...data.pages.slice(1)],
  };
}

export function useMailRuntimeEvents({
  selectedAccountId,
  selectedMailboxId,
  onSent,
  onNavigate,
}: UseMailRuntimeEventsOptions) {
  const queryClient = useQueryClient();
  const selectedAccountIdRef = useRef(selectedAccountId);
  const selectedMailboxIdRef = useRef(selectedMailboxId);
  const mailboxRefreshQueuesRef = useRef(new Map<string, MailboxRefreshQueue>());
  const onSentRef = useRef(onSent);
  const onNavigateRef = useRef(onNavigate);
  selectedAccountIdRef.current = selectedAccountId;
  selectedMailboxIdRef.current = selectedMailboxId;
  onSentRef.current = onSent;
  onNavigateRef.current = onNavigate;

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const register = <T,>(eventName: string, handler: (payload: T) => void) => (
      listen<T>(eventName, (event) => handler(event.payload))
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch((error) => reportCaughtError(`event.listen.${eventName}`, error))
    );

    // Coalesce rapid message-arrived events (a sync with several workers fires
    // many per second) into a single setQueryData per ~100ms. Each setQueryData
    // re-renders the list, so applying them one-by-one would re-render dozens
    // of times per second; buffering keeps the UI responsive during large syncs.
    const arrivedBuffer = new Map<string, MessageListItem[]>();
    let arrivedTimer: ReturnType<typeof setTimeout> | null = null;
    const flushArrived = () => {
      arrivedTimer = null;
      if (arrivedBuffer.size === 0) return;
      for (const [key, items] of arrivedBuffer) {
        const [accountId, mailboxId] = key.split("\0");
        const queryKey = mailQueryKeys.messagesForMailbox(accountId, mailboxId);
        queryClient.setQueryData<MessageListData>(queryKey, (old) => {
          let data = old;
          for (const item of items) data = applyArrivedMessage(data, item);
          return data;
        });
      }
      arrivedBuffer.clear();
    };

    void register<{ accountId: string; mailboxId: string }>("mailbox-changed", (payload) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(payload.accountId) });
      const queryKey = mailQueryKeys.messagesForMailbox(payload.accountId, payload.mailboxId);
      if (payload.accountId === selectedAccountIdRef.current
        && payload.mailboxId === selectedMailboxIdRef.current) {
        const queueId = `${payload.accountId}\0${payload.mailboxId}`;
        const queue = mailboxRefreshQueuesRef.current.get(queueId) ?? { pending: 0, running: false };
        queue.pending += 1;
        mailboxRefreshQueuesRef.current.set(queueId, queue);
        if (!queue.running) {
          queue.running = true;
          void (async () => {
            while (queue.pending > 0) {
              queue.pending -= 1;
              await queryClient
                .refetchQueries({ queryKey, exact: true, type: "active" })
                .catch((error) => reportCaughtError("mailbox.active-refetch", error));
            }
            queue.running = false;
            mailboxRefreshQueuesRef.current.delete(queueId);
          })();
        }
      } else {
        void queryClient.invalidateQueries({ queryKey });
      }
    });
    void register<{ accountId: string; mailboxId: string; item: MessageListItem }>("message-arrived", (payload) => {
      if (payload.accountId === selectedAccountIdRef.current
        && payload.mailboxId === selectedMailboxIdRef.current) {
        const key = `${payload.accountId}\0${payload.mailboxId}`;
        const items = arrivedBuffer.get(key) ?? [];
        items.push(payload.item);
        arrivedBuffer.set(key, items);
        if (arrivedTimer === null) {
          arrivedTimer = setTimeout(flushArrived, 100);
        }
      } else {
        const queryKey = mailQueryKeys.messagesForMailbox(payload.accountId, payload.mailboxId);
        void queryClient.invalidateQueries({ queryKey });
      }
    });
    void register<{ accountId: string }>("sync-progress", (payload) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.syncProgress(payload.accountId) });
    });
    void register<{ accountId: string }>("account-runtime-status-changed", () => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.accountRuntimes });
    });
    void register<{ accountId: string; messageId: string }>("message-content-changed", (payload) => {
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(payload.accountId) });
    });
    void register<{ accountId: string; jobId: string; status: string; subject: string }>("send-job-changed", (payload) => {
      if (payload.accountId !== selectedAccountIdRef.current || payload.status !== "sent") return;
      onSentRef.current({ id: payload.jobId, subject: payload.subject });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.drafts(payload.accountId) });
    });
    void register<{ accountId: string }>("pending-operation-changed", (payload) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(payload.accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(payload.accountId) });
      void queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(payload.accountId) });
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.pendingOperations(payload.accountId) });
    });
    void register<NotificationNavigationTarget>("open-mail-location", (payload) => {
      onNavigateRef.current(payload);
    });

    return () => {
      disposed = true;
      if (arrivedTimer !== null) clearTimeout(arrivedTimer);
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [queryClient]);
}

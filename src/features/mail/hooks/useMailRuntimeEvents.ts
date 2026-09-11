import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import type {
  MessageListItem,
  MessageListPage,
  NotificationNavigationTarget,
  SyncProgress,
} from "@/app/types";
import { reportCaughtError } from "@/app/errorReporting";
import { mailQueryKeys, messageQueryKeys } from "../mail-query-keys";
import { isDemoMode } from "@/app/demo/session";

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
  dirty: boolean;
  running: boolean;
}

interface MessageListData {
  pages: MessageListPage[];
  pageParams: (string | null)[];
}

const MESSAGE_PAGE_SIZE = 50;
const MAX_ARRIVED_QUEUE = 256;

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
  if (insertAt >= MESSAGE_PAGE_SIZE && firstPage.nextCursor !== null) {
    return data;
  }
  let nextItems = [...items.slice(0, insertAt), item, ...items.slice(insertAt)];
  let nextCursor = firstPage.nextCursor;
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
    if (isDemoMode()) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const register = <T,>(eventName: string, handler: (payload: T) => void) => (
      listen<T>(eventName, (event) => { if (!disposed) handler(event.payload); })
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch((error) => reportCaughtError(`event.listen.${eventName}`, error))
    );

    // Preserve one-message-at-a-time arrival semantics without letting Tauri
    // events drive React synchronously. The selected mailbox consumes one
    // committed item per animation frame; background mailboxes only need one
    // snapshot invalidation. The bounded overflow path falls back to the DB
    // snapshot instead of retaining an unbounded in-memory event backlog.
    const arrivedBuffer = new Map<string, MessageListItem[]>();
    const arrivedOverflow = new Set<string>();
    let arrivedFrame: number | null = null;
    const scheduleFrame = (callback: FrameRequestCallback) => (
      typeof window.requestAnimationFrame === "function"
        ? window.requestAnimationFrame(callback)
        : window.setTimeout(() => callback(performance.now()), 16)
    );
    const flushArrived = () => {
      arrivedFrame = null;
      if (arrivedBuffer.size === 0) return;
      let remaining = false;
      for (const [key, items] of [...arrivedBuffer]) {
        const [accountId, mailboxId] = key.split("\0");
        const queryKey = mailQueryKeys.messagesForMailbox(accountId, mailboxId);
        const selected = accountId === selectedAccountIdRef.current
          && mailboxId === selectedMailboxIdRef.current;
        if (arrivedOverflow.delete(key)) {
          arrivedBuffer.delete(key);
          void queryClient.refetchQueries({ queryKey, exact: true, type: "active" }, { cancelRefetch: false });
        } else if (selected && items.length > 0) {
          const item = items.shift();
          queryClient.setQueryData<MessageListData>(queryKey, (old) => {
            return item ? applyArrivedMessage(old, item) : old;
          });
          if (items.length === 0) arrivedBuffer.delete(key);
          else remaining = true;
        } else {
          arrivedBuffer.delete(key);
          void queryClient.invalidateQueries({ queryKey });
        }
      }
      if (remaining || arrivedBuffer.size > 0) arrivedFrame = scheduleFrame(flushArrived);
    };
    // Coalesce sync-progress the same way: keep only the latest payload per
    // account and write it once per ~100ms. A sync commits many messages per
    // second and every cache write re-renders the progress subscribers;
    // intermediate revisions are safe to drop, and the revision guard below
    // still rejects late events after the flush.
    const progressBuffer = new Map<string, SyncProgress>();
    let progressTimer: ReturnType<typeof setTimeout> | null = null;
    const flushProgress = () => {
      progressTimer = null;
      for (const [accountId, payload] of progressBuffer) {
        queryClient.setQueryData<SyncProgress>(
          mailQueryKeys.syncProgress(accountId),
          (current) => current && current.revision >= payload.revision ? current : payload,
        );
      }
      progressBuffer.clear();
    };
    void register<{ accountId: string; mailboxId: string }>("mailbox-changed", (payload) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(payload.accountId) });
      const queryKey = mailQueryKeys.messagesForMailbox(payload.accountId, payload.mailboxId);
      if (payload.accountId === selectedAccountIdRef.current
        && payload.mailboxId === selectedMailboxIdRef.current) {
        const queueId = `${payload.accountId}\0${payload.mailboxId}`;
        const queue = mailboxRefreshQueuesRef.current.get(queueId) ?? { dirty: false, running: false };
        queue.dirty = true;
        mailboxRefreshQueuesRef.current.set(queueId, queue);
        if (!queue.running) {
          queue.running = true;
          void (async () => {
            while (queue.dirty && !disposed) {
              queue.dirty = false;
              await queryClient
                .refetchQueries({ queryKey, exact: true, type: "active" }, { cancelRefetch: false })
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
      const key = `${payload.accountId}\0${payload.mailboxId}`;
      // Background mailboxes need only a dirty marker, never retained content.
      if (payload.accountId !== selectedAccountIdRef.current
        || payload.mailboxId !== selectedMailboxIdRef.current) {
        arrivedBuffer.set(key, []);
        if (arrivedFrame === null) arrivedFrame = scheduleFrame(flushArrived);
        return;
      }
      if (arrivedOverflow.has(key)) return;
      const items = arrivedBuffer.get(key) ?? [];
      items.push(payload.item);
      if (items.length > MAX_ARRIVED_QUEUE) {
        items.length = 0;
        arrivedOverflow.add(key);
      }
      arrivedBuffer.set(key, items);
      if (arrivedFrame === null) arrivedFrame = scheduleFrame(flushArrived);
    });
    void register<SyncProgress>("sync-progress", (payload) => {
      const buffered = progressBuffer.get(payload.accountId);
      if (!buffered || buffered.revision < payload.revision) progressBuffer.set(payload.accountId, payload);
      if (progressTimer === null) {
        progressTimer = setTimeout(flushProgress, 100);
      }
    });
    void register<{ accountId: string }>("account-runtime-status-changed", () => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.accountRuntimes });
    });
    void register<{ accountId: string; messageId: string }>("message-content-changed", (payload) => {
      void queryClient.invalidateQueries({
        queryKey: messageQueryKeys.account(payload.accountId),
        predicate: (query) => query.queryKey[3] === payload.messageId,
      }, { cancelRefetch: false });
    });
    const contactAccounts = new Set<string>();
    let contactsTimer: ReturnType<typeof setTimeout> | null = null;
    void register<{ accountId: string }>("contacts-changed", (payload) => {
      contactAccounts.add(payload.accountId);
      if (contactsTimer !== null) return;
      contactsTimer = setTimeout(() => {
        contactsTimer = null;
        for (const accountId of contactAccounts) {
          void queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) });
          void queryClient.invalidateQueries({
            queryKey: mailQueryKeys.messagesForAccount(accountId),
            refetchType: "none",
          });
          void queryClient.invalidateQueries({
            queryKey: messageQueryKeys.account(accountId),
            refetchType: "none",
          });
        }
        contactAccounts.clear();
      }, 300);
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
      if (arrivedFrame !== null) {
        if (typeof window.cancelAnimationFrame === "function") window.cancelAnimationFrame(arrivedFrame);
        else clearTimeout(arrivedFrame);
      }
      if (progressTimer !== null) clearTimeout(progressTimer);
      if (contactsTimer !== null) clearTimeout(contactsTimer);
      arrivedBuffer.clear();
      arrivedOverflow.clear();
      progressBuffer.clear();
      contactAccounts.clear();
      mailboxRefreshQueuesRef.current.clear();
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [queryClient]);
}

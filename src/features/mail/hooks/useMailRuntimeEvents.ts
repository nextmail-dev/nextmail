import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import type { NotificationNavigationTarget } from "@/app/types";
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
        .catch(() => undefined)
    );

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
              await queryClient.refetchQueries({ queryKey, exact: true, type: "active" }).catch(() => undefined);
            }
            queue.running = false;
            mailboxRefreshQueuesRef.current.delete(queueId);
          })();
        }
      } else {
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
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [queryClient]);
}

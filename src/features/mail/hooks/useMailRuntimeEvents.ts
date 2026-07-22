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
  onSent: (notice: SentNotice) => void;
  onNavigate: (target: NotificationNavigationTarget) => void;
}

export function useMailRuntimeEvents({
  selectedAccountId,
  onSent,
  onNavigate,
}: UseMailRuntimeEventsOptions) {
  const queryClient = useQueryClient();
  const selectedAccountIdRef = useRef(selectedAccountId);
  const onSentRef = useRef(onSent);
  const onNavigateRef = useRef(onNavigate);
  selectedAccountIdRef.current = selectedAccountId;
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
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForMailbox(payload.accountId, payload.mailboxId) });
    });
    void register<{ accountId: string }>("sync-progress", (payload) => {
      void queryClient.invalidateQueries({ queryKey: mailQueryKeys.syncProgress(payload.accountId) });
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

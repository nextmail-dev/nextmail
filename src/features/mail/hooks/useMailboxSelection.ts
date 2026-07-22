import { useCallback, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountSummary, NotificationNavigationTarget } from "@/app/types";
import { mailQueryKeys } from "../mail-query-keys";

interface UseMailboxSelectionOptions {
  accounts: AccountSummary[];
  lastSelectedAccountId: string | null;
  onError: (errorCode: string) => void;
}

export function useMailboxSelection({
  accounts,
  lastSelectedAccountId,
  onError,
}: UseMailboxSelectionOptions) {
  const [selectedAccountId, setSelectedAccountId] = useState(() => (
    accounts.some((account) => account.id === lastSelectedAccountId)
      ? lastSelectedAccountId ?? ""
      : accounts[0]?.id ?? ""
  ));
  const [selectedMailboxId, setSelectedMailboxId] = useState("");
  const [selectedMessageId, setSelectedMessageId] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [pendingNavigation, setPendingNavigation] = useState<NotificationNavigationTarget | null>(null);
  const mailboxesQuery = useQuery({
    queryKey: mailQueryKeys.mailboxes(selectedAccountId),
    queryFn: () => api.listMailboxes(selectedAccountId),
    enabled: Boolean(selectedAccountId),
  });

  useEffect(() => {
    if (selectedAccountId && accounts.some((account) => account.id === selectedAccountId)) return;
    setSelectedAccountId(accounts[0]?.id ?? "");
  }, [accounts, selectedAccountId]);

  useEffect(() => {
    setSelectedMailboxId("");
    setSelectedMessageId("");
    setSearchQuery("");
  }, [selectedAccountId]);

  useEffect(() => {
    const mailboxes = mailboxesQuery.data ?? [];
    const pending = pendingNavigation?.accountId === selectedAccountId ? pendingNavigation : null;
    if (pending && !mailboxesQuery.data) return;
    if (pending) {
      const requested = mailboxes.find((mailbox) => mailbox.id === pending.mailboxId && mailbox.selectable);
      const fallback = mailboxes.find((mailbox) => mailbox.role === "inbox" && mailbox.selectable)
        ?? mailboxes.find((mailbox) => mailbox.selectable);
      setSelectedMailboxId(requested?.id ?? fallback?.id ?? "");
      setSelectedMessageId(requested ? pending.messageId ?? "" : "");
      setSearchQuery("");
      setPendingNavigation(null);
      return;
    }
    if (selectedMailboxId && mailboxes.some((mailbox) => mailbox.id === selectedMailboxId)) return;
    const initial = mailboxes.find((mailbox) => mailbox.role === "inbox" && mailbox.selectable)
      ?? mailboxes.find((mailbox) => mailbox.selectable);
    setSelectedMailboxId(initial?.id ?? "");
  }, [mailboxesQuery.data, pendingNavigation, selectedAccountId, selectedMailboxId]);

  const selectAccount = useCallback((accountId: string) => {
    setPendingNavigation(null);
    setSelectedAccountId(accountId);
    void api.setLastSelectedAccount(accountId).catch((error) => {
      onError(normalizeCommandError(error).code);
    });
  }, [onError]);

  const selectMailbox = useCallback((mailboxId: string) => {
    setPendingNavigation(null);
    setSelectedMailboxId(mailboxId);
    setSelectedMessageId("");
    setSearchQuery("");
  }, []);

  const navigateToMailLocation = useCallback((target: NotificationNavigationTarget) => {
    if (!accounts.some((account) => account.id === target.accountId)) return;
    setPendingNavigation(target);
    setSearchQuery("");
    setSelectedMessageId("");
    setSelectedAccountId(target.accountId);
    void api.setLastSelectedAccount(target.accountId).catch((error) => {
      onError(normalizeCommandError(error).code);
    });
  }, [accounts, onError]);

  return {
    mailboxesQuery,
    navigateToMailLocation,
    searchQuery,
    selectAccount,
    selectMailbox,
    selectedAccountId,
    selectedMailboxId,
    selectedMessageId,
    setSearchQuery,
    setSelectedMessageId,
  };
}

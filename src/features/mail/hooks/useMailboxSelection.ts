import { useCallback, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountSummary } from "@/app/types";
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
    if (selectedMailboxId && mailboxes.some((mailbox) => mailbox.id === selectedMailboxId)) return;
    const initial = mailboxes.find((mailbox) => mailbox.role === "inbox" && mailbox.selectable)
      ?? mailboxes.find((mailbox) => mailbox.selectable);
    setSelectedMailboxId(initial?.id ?? "");
  }, [mailboxesQuery.data, selectedMailboxId]);

  const selectAccount = useCallback((accountId: string) => {
    setSelectedAccountId(accountId);
    void api.setLastSelectedAccount(accountId).catch((error) => {
      onError(normalizeCommandError(error).code);
    });
  }, [onError]);

  const selectMailbox = useCallback((mailboxId: string) => {
    setSelectedMailboxId(mailboxId);
    setSelectedMessageId("");
    setSearchQuery("");
  }, []);

  return {
    mailboxesQuery,
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

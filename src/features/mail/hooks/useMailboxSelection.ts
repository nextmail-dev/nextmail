import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountSummary, NotificationNavigationTarget } from "@/app/types";
import { mailQueryKeys, STARRED_MAILBOX_ID, UNREAD_MAILBOX_ID } from "../mail-query-keys";

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
  const [submittedSearchQuery, setSubmittedSearchQuery] = useState("");
  const [pendingNavigation, setPendingNavigation] = useState<NotificationNavigationTarget | null>(null);
  const rememberedMailboxIds = useRef(new Map(
    accounts.flatMap((account) => account.lastSelectedMailboxId
      ? [[account.id, account.lastSelectedMailboxId] as const]
      : []),
  ));
  const mailboxesQuery = useQuery({
    queryKey: mailQueryKeys.mailboxes(selectedAccountId),
    queryFn: () => api.listMailboxes(selectedAccountId),
    enabled: Boolean(selectedAccountId),
  });
  const rememberMailbox = useCallback((accountId: string, mailboxId: string) => {
    rememberedMailboxIds.current.set(accountId, mailboxId);
    void api.setLastSelectedMailbox(accountId, mailboxId).catch((error) => {
      onError(normalizeCommandError(error).code);
    });
  }, [onError]);

  useEffect(() => {
    if (selectedAccountId && accounts.some((account) => account.id === selectedAccountId)) return;
    setSelectedAccountId(accounts[0]?.id ?? "");
  }, [accounts, selectedAccountId]);

  useEffect(() => {
    setSelectedMailboxId("");
    setSelectedMessageId("");
    setSearchQuery("");
    setSubmittedSearchQuery("");
  }, [selectedAccountId]);

  useEffect(() => {
    const mailboxes = mailboxesQuery.data ?? [];
    const pending = pendingNavigation?.accountId === selectedAccountId ? pendingNavigation : null;
    if (pending && !mailboxesQuery.data) return;
    if (pending) {
      const requested = mailboxes.find((mailbox) => mailbox.id === pending.mailboxId && mailbox.selectable);
      const fallback = mailboxes.find((mailbox) => mailbox.role === "inbox" && mailbox.selectable)
        ?? mailboxes.find((mailbox) => mailbox.selectable);
      const nextMailboxId = requested?.id ?? fallback?.id ?? "";
      setSelectedMailboxId(nextMailboxId);
      setSelectedMessageId(requested ? pending.messageId ?? "" : "");
      setSearchQuery("");
      setSubmittedSearchQuery("");
      setPendingNavigation(null);
      if (nextMailboxId) rememberMailbox(selectedAccountId, nextMailboxId);
      return;
    }
    if (selectedMailboxId === UNREAD_MAILBOX_ID || selectedMailboxId === STARRED_MAILBOX_ID
      || selectedMailboxId && mailboxes.some((mailbox) => mailbox.id === selectedMailboxId && mailbox.selectable)) return;
    const rememberedMailboxId = rememberedMailboxIds.current.get(selectedAccountId)
      ?? accounts.find((account) => account.id === selectedAccountId)?.lastSelectedMailboxId;
    const rememberedMailboxExists = rememberedMailboxId === UNREAD_MAILBOX_ID
      || rememberedMailboxId === STARRED_MAILBOX_ID
      || mailboxes.some((mailbox) => mailbox.id === rememberedMailboxId && mailbox.selectable);
    const fallbackMailboxId = mailboxes.find((mailbox) => mailbox.role === "inbox" && mailbox.selectable)?.id
      ?? mailboxes.find((mailbox) => mailbox.selectable)?.id
      ?? "";
    const nextMailboxId = rememberedMailboxExists ? rememberedMailboxId ?? "" : fallbackMailboxId;
    setSelectedMailboxId(nextMailboxId);
    if (nextMailboxId && nextMailboxId !== rememberedMailboxId) {
      rememberMailbox(selectedAccountId, nextMailboxId);
    }
  }, [accounts, mailboxesQuery.data, pendingNavigation, rememberMailbox, selectedAccountId, selectedMailboxId]);

  const selectAccount = useCallback((accountId: string) => {
    setPendingNavigation(null);
    setSelectedAccountId(accountId);
    void api.setLastSelectedAccount(accountId).catch((error) => {
      onError(normalizeCommandError(error).code);
    });
  }, [onError]);

  const selectMailbox = useCallback((mailboxId: string) => {
    setPendingNavigation(null);
    if (mailboxId === selectedMailboxId) return;
    setSelectedMailboxId(mailboxId);
    setSelectedMessageId("");
    setSearchQuery("");
    setSubmittedSearchQuery("");
    rememberMailbox(selectedAccountId, mailboxId);
  }, [rememberMailbox, selectedAccountId, selectedMailboxId]);

  const navigateToMailLocation = useCallback((target: NotificationNavigationTarget) => {
    if (!accounts.some((account) => account.id === target.accountId)) return;
    setPendingNavigation(target);
    setSearchQuery("");
    setSubmittedSearchQuery("");
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
    submittedSearchQuery,
    selectAccount,
    selectMailbox,
    selectedAccountId,
    selectedMailboxId,
    selectedMessageId,
    setSearchQuery,
    setSubmittedSearchQuery,
    setSelectedMessageId,
  };
}

import { memo, useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleUserRound, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { afterFirstPaint } from "@/app/startup";
import type { AccountRuntimeSummary, AccountSummary, MailboxSummary, SyncProgress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { ResizeHandle } from "@/components/ui/resize-handle";
import { Toast } from "@/components/ui/toast";
import { Text } from "@/components/ui/typography";
import { ContactsWorkspace } from "@/features/contacts/ContactsWorkspace";
import { AccountSwitcher } from "./AccountSwitcher";
import { MailboxPane } from "./MailboxPane";
import { MessageListPane } from "./MessageListPane";
import { MessageViewer } from "./MessageViewer";
import { nextMessageIdAfterRemoval, nextMessageIdAfterRemovals } from "./message-selection";
import { useMailboxSelection } from "./hooks/useMailboxSelection";
import { useMailRuntimeEvents } from "./hooks/useMailRuntimeEvents";
import { useMailboxActions } from "./hooks/useMailboxActions";
import { usePaneLayout } from "./hooks/usePaneLayout";
import { mailQueryKeys, UNREAD_MAILBOX_ID } from "./mail-query-keys";

interface MainShellProps {
  accounts: AccountSummary[];
  lastSelectedAccountId: string | null;
}

// Stable fallbacks: `data ?? []` would create a fresh array each render and
// defeat React.memo on the panes below.
const EMPTY_ACCOUNTS: AccountSummary[] = [];
const EMPTY_RUNTIMES: AccountRuntimeSummary[] = [];
const EMPTY_MAILBOXES: MailboxSummary[] = [];

type SidebarPaneProps = Omit<
  Parameters<typeof MailboxPane>[0],
  "progress" | "receiving" | "folderActionBusy"
> & {
  accountId: string;
  runtimeSyncing: boolean;
  folderActionsBusy: boolean;
  showProgress: boolean;
  onProgressFinished: (accountId: string) => void;
};

// Owns the sync-progress subscription so progress updates only re-render the
// sidebar subtree. MainShell previously held this query, re-rendering the whole
// window (message list and reader included) once per synced message.
const SidebarPane = memo(function SidebarPane({
  accountId,
  runtimeSyncing,
  folderActionsBusy,
  showProgress,
  onProgressFinished,
  ...paneProps
}: SidebarPaneProps) {
  const progressQuery = useQuery({
    queryKey: mailQueryKeys.syncProgress(accountId),
    queryFn: () => api.getSyncProgress(accountId),
    enabled: Boolean(accountId),
    refetchInterval: (query) => ["complete", "failed"].includes(query.state.data?.phase ?? "idle") ? false : 1_500,
  });
  const receiving = runtimeSyncing
    || !["idle", "complete", "failed"].includes(progressQuery.data?.phase ?? "idle");
  const activeProgress = !["idle", "complete", "failed"].includes(progressQuery.data?.phase ?? "idle");
  const progressWasActive = useRef(false);
  useEffect(() => {
    if (!showProgress) {
      progressWasActive.current = false;
    } else if (activeProgress) {
      progressWasActive.current = true;
    } else if (progressWasActive.current) {
      progressWasActive.current = false;
      onProgressFinished(accountId);
    }
  }, [accountId, activeProgress, onProgressFinished, showProgress]);
  return (
    <MailboxPane
      {...paneProps}
      progress={progressQuery.data}
      showProgress={showProgress}
      receiving={receiving}
      folderActionBusy={folderActionsBusy || receiving}
    />
  );
});

export function MainShell({ accounts: initialAccounts, lastSelectedAccountId }: MainShellProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const accountsQuery = useQuery({
    queryKey: mailQueryKeys.accounts,
    queryFn: api.listAccountSummaries,
    initialData: initialAccounts,
  });
  const runtimeQuery = useQuery({
    queryKey: mailQueryKeys.accountRuntimes,
    queryFn: api.listAccountRuntimeSummaries,
    refetchInterval: 10_000,
  });
  const accounts = accountsQuery.data ?? EMPTY_ACCOUNTS;
  const [composeError, setComposeError] = useState<string | null>(null);
  const [manualSyncAccountId, setManualSyncAccountId] = useState<string | null>(null);
  const [selectedMessageMailboxId, setSelectedMessageMailboxId] = useState("");
  const [workspace, setWorkspace] = useState<"mail" | "contacts">("mail");
  const [requestedContactId, setRequestedContactId] = useState("");
  const [requestedContactEdit, setRequestedContactEdit] = useState<{ contactId: string; requestId: number } | null>(null);
  const contactEditRequestIdRef = useRef(0);
  const [sentNotice, setSentNotice] = useState<{ id: string; subject: string } | null>(null);
  // Kept in a ref (not state): it only feeds selectAfterRemoval, so updating it
  // must not re-render MainShell (and the heavy MessageViewer) on every arrival.
  const visibleMessageIdsRef = useRef<string[]>([]);
  const handleVisibleMessageIdsChange = useCallback((ids: string[]) => {
    visibleMessageIdsRef.current = ids;
  }, []);
  const {
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
  } = useMailboxSelection({
    accounts,
    lastSelectedAccountId,
    onError: setComposeError,
  });
  const navigateToMessage = useCallback((target: Parameters<typeof navigateToMailLocation>[0]) => {
    setWorkspace("mail");
    navigateToMailLocation(target);
  }, [navigateToMailLocation]);
  const openContact = useCallback((contactId: string) => {
    setRequestedContactId(contactId);
    setWorkspace("contacts");
  }, []);
  const editContact = useCallback((contactId: string) => {
    contactEditRequestIdRef.current += 1;
    setRequestedContactId(contactId);
    setRequestedContactEdit({ contactId, requestId: contactEditRequestIdRef.current });
    setWorkspace("contacts");
  }, []);
  const {
    folderPaneCollapsed,
    folderPaneMax,
    folderPaneWidth,
    messagePaneMax,
    messagePaneWidth,
    setFolderPaneCollapsed,
    setFolderPaneWidth,
    setMessagePaneWidth,
    visibleFolderWidth,
  } = usePaneLayout();
  useMailRuntimeEvents({
    selectedAccountId,
    selectedMailboxId,
    onSent: setSentNotice,
    onNavigate: navigateToMessage,
  });
  const pendingOperationsQuery = useQuery({
    queryKey: mailQueryKeys.pendingOperations(selectedAccountId),
    queryFn: () => api.listPendingOperationStatus(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: 5_000,
  });
  const pendingIssue = pendingOperationsQuery.data?.find((operation) =>
    operation.cleanupPending || operation.status === "failed" || operation.status === "needs_reconcile");
  const selectedMailbox = mailboxesQuery.data?.find((mailbox) => mailbox.id === selectedMailboxId);
  const runtimeSyncing = runtimeQuery.data?.some((runtime) =>
    runtime.accountId === selectedAccountId && runtime.state === "syncing") ?? false;
  const mailboxActions = useMailboxActions({
    accountId: selectedAccountId,
    onError: setComposeError,
  });
  const finishManualSyncProgress = useCallback((accountId: string) => {
    setManualSyncAccountId((current) => current === accountId ? null : current);
  }, []);
  const markAllUnreadRead = useCallback(() => mailboxActions.markAllUnreadRead(
    (mailboxesQuery.data ?? EMPTY_MAILBOXES)
      .filter((mailbox) => mailbox.selectable && mailbox.unreadCount > 0)
      .map((mailbox) => mailbox.id),
  ), [mailboxActions.markAllUnreadRead, mailboxesQuery.data]);
  const selectAfterRemoval = useCallback((removedMessageId: string) => {
    if (selectedMailboxId === UNREAD_MAILBOX_ID) {
      setSelectedMessageId((current) => current === removedMessageId ? "" : current);
      return;
    }
    setSelectedMessageId((current) => current === removedMessageId
      ? nextMessageIdAfterRemoval(visibleMessageIdsRef.current, removedMessageId)
      : current);
  }, [selectedMailboxId, setSelectedMessageId]);
  const selectAfterRemovals = useCallback((removedMessageIds: string[]) => {
    if (selectedMailboxId === UNREAD_MAILBOX_ID) {
      setSelectedMessageId("");
      return;
    }
    setSelectedMessageId((current) => nextMessageIdAfterRemovals(
      visibleMessageIdsRef.current,
      removedMessageIds,
      current,
    ));
  }, [selectedMailboxId, setSelectedMessageId]);
  const selectMessage = useCallback((messageId: string, sourceMailboxId: string) => {
    setSelectedMessageId(messageId);
    setSelectedMessageMailboxId(messageId ? sourceMailboxId : "");
  }, [setSelectedMessageId]);

  useEffect(() => setSelectedMessageMailboxId(""), [selectedMailboxId]);

  useEffect(() => {
    if (!selectedAccountId) return;
    return afterFirstPaint(() => {
      void api.startBackgroundServices()
        .then(() => queryClient.invalidateQueries({ queryKey: mailQueryKeys.syncProgress(selectedAccountId) }))
        .catch((error) => setComposeError(normalizeCommandError(error).code));
    });
  }, [queryClient, selectedAccountId]);

  useEffect(() => {
    if (!sentNotice) return;
    const timeout = window.setTimeout(() => setSentNotice(null), 4_500);
    return () => window.clearTimeout(timeout);
  }, [sentNotice]);

  // Handlers below are wrapped in useCallback (or come from stable setters) so
  // the memoized panes are not re-rendered by unrelated MainShell state
  // changes such as dragging a splitter.
  const receive = useCallback(() => {
    if (!selectedAccountId) return;
    setComposeError(null);
    void api.syncNow(selectedAccountId)
      .then(async () => {
        await queryClient.invalidateQueries({ queryKey: mailQueryKeys.syncProgress(selectedAccountId) });
        const progress = queryClient.getQueryData<SyncProgress>(mailQueryKeys.syncProgress(selectedAccountId));
        if (progress && !["idle", "complete", "failed"].includes(progress.phase)) {
          setManualSyncAccountId(selectedAccountId);
        }
      })
      .catch((error) => {
        finishManualSyncProgress(selectedAccountId);
        setComposeError(normalizeCommandError(error).code);
      });
  }, [finishManualSyncProgress, queryClient, selectedAccountId]);

  const openAccountManagement = useCallback(() => {
    setComposeError(null);
    void api.openAccountManagementWindow()
      .catch((error) => setComposeError(normalizeCommandError(error).code));
  }, []);

  const handleSelectMailbox = useCallback((mailboxId: string) => {
    setWorkspace("mail");
    selectMailbox(mailboxId);
  }, [selectMailbox]);

  const handleCompose = useCallback(() => {
    if (!selectedAccountId) return;
    setComposeError(null);
    void api.openComposer(selectedAccountId)
      .catch((error) => setComposeError(normalizeCommandError(error).code));
  }, [selectedAccountId]);

  const handleSelectContacts = useCallback(() => {
    setRequestedContactId("");
    setWorkspace("contacts");
  }, []);

  const handleOpenSettings = useCallback(() => {
    void api.openSettingsWindow().catch((error) => setComposeError(normalizeCommandError(error).code));
  }, []);

  if (!accounts.length) {
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <EmptyAccountState
          title={t("accounts.noAccount")}
          description={t("accounts.noAccountDescription")}
          actionLabel={t("accounts.add")}
          onAdd={openAccountManagement}
        />
      </AppShell>
    );
  }

  return (
    <AppShell
      className="grid overflow-hidden bg-card"
      style={{ gridTemplateColumns: `${visibleFolderWidth}px 0 minmax(0,1fr)` }}
    >
      <Page className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-sidebar">
        <AccountSwitcher
          accounts={accounts}
          runtimeSummaries={runtimeQuery.data ?? EMPTY_RUNTIMES}
          selectedAccountId={selectedAccountId}
          onAccountChange={selectAccount}
          onManageAccounts={openAccountManagement}
          collapsed={folderPaneCollapsed}
        />
        <SidebarPane
          accountId={selectedAccountId}
          runtimeSyncing={runtimeSyncing}
          folderActionsBusy={mailboxActions.busy}
          showProgress={manualSyncAccountId === selectedAccountId}
          onProgressFinished={finishManualSyncProgress}
          mailboxes={mailboxesQuery.data ?? EMPTY_MAILBOXES}
          selectedMailboxId={workspace === "mail" ? selectedMailboxId : ""}
          onSelect={handleSelectMailbox}
          error={mailboxesQuery.error}
          onCompose={handleCompose}
          contactsSelected={workspace === "contacts"}
          onSelectContacts={handleSelectContacts}
          onReceive={receive}
          onCreateFolder={mailboxActions.createMailbox}
          onRenameFolder={mailboxActions.renameMailbox}
          onMoveFolder={mailboxActions.moveMailbox}
          onDeleteFolder={mailboxActions.deleteMailbox}
          onMarkFolderAllRead={mailboxActions.markMailboxAllRead}
          onSetFavorite={mailboxActions.setMailboxFavorite}
          onMarkAllUnreadRead={markAllUnreadRead}
          onReorderFolders={mailboxActions.reorderMailboxes}
          onOpenSettings={handleOpenSettings}
          collapsed={folderPaneCollapsed}
        />
      </Page>
      <ResizeHandle
        value={folderPaneWidth}
        min={220}
        max={folderPaneMax}
        onValueChange={setFolderPaneWidth}
        label={t("mail.resizeFolderPane")}
        collapsed={folderPaneCollapsed}
        onCollapsedChange={setFolderPaneCollapsed}
        collapseLabel={t("mail.collapseFolderPane")}
        expandLabel={t("mail.expandFolderPane")}
      />
      {workspace === "contacts" ? (
        <ContactsWorkspace
          accountId={selectedAccountId}
          listPaneWidth={messagePaneWidth}
          listPaneMax={messagePaneMax}
          onListPaneWidthChange={setMessagePaneWidth}
          onNavigateToMessage={navigateToMessage}
          requestedContactId={requestedContactId}
          requestedContactEdit={requestedContactEdit}
        />
      ) : (
        <Page className="grid min-h-0 bg-card" style={{ gridTemplateColumns: `${messagePaneWidth}px 0 minmax(360px,1fr)` }}>
          <Page className="flex min-h-0 flex-col bg-card">
            <MessageListPane
              accountId={selectedAccountId}
              mailboxId={selectedMailboxId}
              mailbox={selectedMailbox}
              mailboxes={mailboxesQuery.data ?? EMPTY_MAILBOXES}
              selectedMessageId={selectedMessageId}
              onSelect={selectMessage}
              onVisibleMessageIdsChange={handleVisibleMessageIdsChange}
              onMessagesRemoved={selectAfterRemovals}
              onOpenContact={openContact}
              onEditContact={editContact}
              searchQuery={searchQuery}
              submittedSearchQuery={submittedSearchQuery}
              onSearchChange={setSearchQuery}
              onSearchSubmit={setSubmittedSearchQuery}
            />
          </Page>
          <ResizeHandle value={messagePaneWidth} min={310} max={messagePaneMax} onValueChange={setMessagePaneWidth} label={t("mail.resizeMessagePane")} />
          <Page className="flex min-h-0 flex-col bg-card">
            <MessageViewer
              accountId={selectedAccountId}
              mailboxId={selectedMessageMailboxId || selectedMailboxId}
              messageId={selectedMessageId}
              mailboxes={mailboxesQuery.data ?? EMPTY_MAILBOXES}
              onMessageRemoved={selectAfterRemoval}
              onOpenContact={openContact}
              onEditContact={editContact}
            />
          </Page>
        </Page>
      )}

      {composeError ? (
        <Alert className="fixed right-4 bottom-4 z-40 max-w-sm bg-popover shadow-xl" tone="danger">
          {t(`errors.${composeError}`, { defaultValue: t("common.unexpectedError") })}
          <Button variant="ghost" size="icon" aria-label={t("common.close")} onClick={() => setComposeError(null)}><X size={15} /></Button>
        </Alert>
      ) : null}
      {sentNotice ? <Toast title={t("composer.sent")} description={sentNotice.subject || t("mail.noSubject")} closeLabel={t("common.close")} onClose={() => setSentNotice(null)} /> : null}
      {pendingIssue ? (
        <Alert className="fixed right-4 bottom-20 z-40 max-w-sm bg-popover shadow-xl" tone="warning" title={t("mail.syncActionNeedsAttention")}>
          <Stack gap="sm">
            <Text className="text-xs text-current">
              {pendingIssue.cleanupPending ? t("mail.serverCleanupPending") : t(`errors.${pendingIssue.errorCode}`, { defaultValue: t("mail.syncActionFailed") })}
            </Text>
            {!pendingIssue.cleanupPending ? (
              <Button variant="secondary" size="sm" onClick={() => void api.retryPendingOperation(selectedAccountId, pendingIssue.id).then(() => queryClient.invalidateQueries({ queryKey: mailQueryKeys.pendingOperations(selectedAccountId) }))}>{t("common.retry")}</Button>
            ) : null}
          </Stack>
        </Alert>
      ) : null}
    </AppShell>
  );
}

function EmptyAccountState({ title, description, actionLabel, onAdd }: { title: string; description: string; actionLabel: string; onAdd: () => void }) {
  return (
    <Stack className="max-w-md items-center text-center" gap="md">
      <span className="grid size-14 place-items-center rounded-full bg-primary/10 text-primary"><CircleUserRound size={26} /></span>
      <Text className="text-lg font-semibold text-foreground">{title}</Text>
      <Text>{description}</Text>
      <Button onClick={onAdd}><Plus size={16} />{actionLabel}</Button>
    </Stack>
  );
}

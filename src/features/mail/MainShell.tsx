import { useCallback, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleUserRound, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { afterFirstPaint } from "@/app/startup";
import type { AccountSummary } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { ResizeHandle } from "@/components/ui/resize-handle";
import { Toast } from "@/components/ui/toast";
import { Text } from "@/components/ui/typography";
import { AccountManagementDialog } from "@/features/accounts/AccountManagementDialog";
import { AccountSwitcher } from "./AccountSwitcher";
import { MailboxPane } from "./MailboxPane";
import { MessageListPane } from "./MessageListPane";
import { MessageViewer } from "./MessageViewer";
import { nextMessageIdAfterRemoval } from "./message-selection";
import { useMailboxSelection } from "./hooks/useMailboxSelection";
import { useMailRuntimeEvents } from "./hooks/useMailRuntimeEvents";
import { usePaneLayout } from "./hooks/usePaneLayout";
import { mailQueryKeys } from "./mail-query-keys";

interface MainShellProps {
  accounts: AccountSummary[];
  lastSelectedAccountId: string | null;
}

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
  const accounts = accountsQuery.data ?? [];
  const [composeError, setComposeError] = useState<string | null>(null);
  const [accountManagementOpen, setAccountManagementOpen] = useState(false);
  const [sentNotice, setSentNotice] = useState<{ id: string; subject: string } | null>(null);
  const [visibleMessageIds, setVisibleMessageIds] = useState<string[]>([]);
  const {
    mailboxesQuery,
    searchQuery,
    selectAccount,
    selectMailbox,
    selectedAccountId,
    selectedMailboxId,
    selectedMessageId,
    setSearchQuery,
    setSelectedMessageId,
  } = useMailboxSelection({
    accounts,
    lastSelectedAccountId,
    onError: setComposeError,
  });
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
  } = usePaneLayout(accounts.length > 0);
  useMailRuntimeEvents({ selectedAccountId, onSent: setSentNotice });
  const progressQuery = useQuery({
    queryKey: mailQueryKeys.syncProgress(selectedAccountId),
    queryFn: () => api.getSyncProgress(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: (query) => ["complete", "failed"].includes(query.state.data?.phase ?? "idle") ? false : 1_500,
  });
  const draftsQuery = useQuery({
    queryKey: mailQueryKeys.drafts(selectedAccountId),
    queryFn: () => api.listDrafts(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: 3_000,
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
  const receiving = !["idle", "complete", "failed"].includes(progressQuery.data?.phase ?? "idle");
  const selectAfterRemoval = useCallback((removedMessageId: string) => {
    setSelectedMessageId((current) => current === removedMessageId
      ? nextMessageIdAfterRemoval(visibleMessageIds, removedMessageId)
      : current);
  }, [setSelectedMessageId, visibleMessageIds]);

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

  function receive() {
    if (!selectedAccountId) return;
    setComposeError(null);
    void api.syncNow(selectedAccountId)
      .then(() => queryClient.invalidateQueries({ queryKey: mailQueryKeys.syncProgress(selectedAccountId) }))
      .catch((error) => setComposeError(normalizeCommandError(error).code));
  }

  if (!accounts.length) {
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <EmptyAccountState
          title={t("accounts.noAccount")}
          description={t("accounts.noAccountDescription")}
          actionLabel={t("accounts.add")}
          onAdd={() => setAccountManagementOpen(true)}
        />
        <AccountManagementDialog
          open={accountManagementOpen}
          onOpenChange={setAccountManagementOpen}
          accounts={accounts}
          selectedAccountId={selectedAccountId}
          onSelectedAccountChange={selectAccount}
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
          runtimeSummaries={runtimeQuery.data ?? []}
          selectedAccountId={selectedAccountId}
          onAccountChange={selectAccount}
          onManageAccounts={() => setAccountManagementOpen(true)}
          collapsed={folderPaneCollapsed}
        />
        <MailboxPane
          mailboxes={mailboxesQuery.data ?? []}
          selectedMailboxId={selectedMailboxId}
          onSelect={selectMailbox}
          progress={progressQuery.data}
          error={mailboxesQuery.error}
          onCompose={() => {
            if (!selectedAccountId) return;
            setComposeError(null);
            void api.openComposer(selectedAccountId)
              .then(() => queryClient.invalidateQueries({ queryKey: mailQueryKeys.drafts(selectedAccountId) }))
              .catch((error) => setComposeError(normalizeCommandError(error).code));
          }}
          drafts={draftsQuery.data ?? []}
          onOpenDraft={(draftId) => void api.openExistingComposer(selectedAccountId, draftId).catch((error) => setComposeError(normalizeCommandError(error).code))}
          onDeleteDraft={async (draftId) => {
            try {
              await api.deleteDraft(selectedAccountId, draftId);
              await queryClient.invalidateQueries({ queryKey: mailQueryKeys.drafts(selectedAccountId) });
            } catch (error) {
              setComposeError(normalizeCommandError(error).code);
              throw error;
            }
          }}
          onReceive={receive}
          receiving={receiving}
          onOpenSettings={() => void api.openSettingsWindow().catch((error) => setComposeError(normalizeCommandError(error).code))}
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
      <Page className="grid min-h-0 bg-card" style={{ gridTemplateColumns: `${messagePaneWidth}px 0 minmax(360px,1fr)` }}>
        <Page className="flex min-h-0 flex-col bg-card">
          <MessageListPane
            accountId={selectedAccountId}
            mailboxId={selectedMailboxId}
            mailbox={selectedMailbox}
            mailboxes={mailboxesQuery.data ?? []}
            selectedMessageId={selectedMessageId}
            onSelect={setSelectedMessageId}
            onVisibleMessageIdsChange={setVisibleMessageIds}
            onMessageRemoved={selectAfterRemoval}
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
          />
        </Page>
        <ResizeHandle value={messagePaneWidth} min={310} max={messagePaneMax} onValueChange={setMessagePaneWidth} label={t("mail.resizeMessagePane")} />
        <Page className="flex min-h-0 flex-col bg-card">
          <MessageViewer
            accountId={selectedAccountId}
            mailboxId={selectedMailboxId}
            messageId={selectedMessageId}
            mailboxes={mailboxesQuery.data ?? []}
            onMessageRemoved={selectAfterRemoval}
          />
        </Page>
      </Page>

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
      <AccountManagementDialog
        open={accountManagementOpen}
        onOpenChange={setAccountManagementOpen}
        accounts={accounts}
        selectedAccountId={selectedAccountId}
        onSelectedAccountChange={selectAccount}
      />
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

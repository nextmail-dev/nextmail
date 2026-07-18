import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useState } from "react";
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
import { AccountSwitcher } from "./AccountSwitcher";
import { MailboxPane } from "./MailboxPane";
import { MessageListPane } from "./MessageListPane";
import { MessageViewer } from "./MessageViewer";

interface MainShellProps {
  accounts: AccountSummary[];
  lastSelectedAccountId: string | null;
}

export function MainShell({ accounts: initialAccounts, lastSelectedAccountId }: MainShellProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    queryFn: api.listAccountSummaries,
    initialData: initialAccounts,
  });
  const runtimeQuery = useQuery({
    queryKey: ["account-runtimes"],
    queryFn: api.listAccountRuntimeSummaries,
    refetchInterval: 10_000,
  });
  const accounts = accountsQuery.data ?? [];
  const initialAccountId = accounts.some((account) => account.id === lastSelectedAccountId)
    ? lastSelectedAccountId ?? ""
    : accounts[0]?.id ?? "";
  const [selectedAccountId, setSelectedAccountId] = useState(initialAccountId);
  const [selectedMailboxId, setSelectedMailboxId] = useState("");
  const [selectedMessageId, setSelectedMessageId] = useState("");
  const [composeError, setComposeError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [sentNotice, setSentNotice] = useState<{ id: string; subject: string } | null>(null);
  const [folderPaneWidth, setFolderPaneWidth] = useState(250);
  const [messagePaneWidth, setMessagePaneWidth] = useState(370);
  const [folderPaneCollapsed, setFolderPaneCollapsed] = useState(false);
  const [windowWidth, setWindowWidth] = useState(() => window.innerWidth);
  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", selectedAccountId],
    queryFn: () => api.listMailboxes(selectedAccountId),
    enabled: Boolean(selectedAccountId),
  });
  const progressQuery = useQuery({
    queryKey: ["sync-progress", selectedAccountId],
    queryFn: () => api.getSyncProgress(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: (query) => ["complete", "failed"].includes(query.state.data?.phase ?? "idle") ? false : 1_500,
  });
  const draftsQuery = useQuery({
    queryKey: ["drafts", selectedAccountId],
    queryFn: () => api.listDrafts(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: 3_000,
  });
  const pendingOperationsQuery = useQuery({
    queryKey: ["pending-operations", selectedAccountId],
    queryFn: () => api.listPendingOperationStatus(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: 5_000,
  });
  const pendingIssue = pendingOperationsQuery.data?.find((operation) =>
    operation.cleanupPending || operation.status === "failed" || operation.status === "needs_reconcile");
  const visibleFolderWidth = folderPaneCollapsed ? 72 : folderPaneWidth;
  const titlebarSidebarWidth = accounts.length ? visibleFolderWidth : 0;
  const selectedMailbox = mailboxesQuery.data?.find((mailbox) => mailbox.id === selectedMailboxId);
  const receiving = !["idle", "complete", "failed"].includes(progressQuery.data?.phase ?? "idle");

  useEffect(() => {
    if (!selectedAccountId) return;
    return afterFirstPaint(() => {
      void api.startBackgroundServices()
        .then(() => queryClient.invalidateQueries({ queryKey: ["sync-progress", selectedAccountId] }))
        .catch((error) => setComposeError(normalizeCommandError(error).code));
    });
  }, [queryClient, selectedAccountId]);

  useEffect(() => {
    if (selectedAccountId && accounts.some((account) => account.id === selectedAccountId)) return;
    setSelectedAccountId(accounts[0]?.id ?? "");
  }, [accounts, selectedAccountId]);

  function selectAccount(accountId: string) {
    setSelectedAccountId(accountId);
    void api.setLastSelectedAccount(accountId).catch((error) => {
      setComposeError(normalizeCommandError(error).code);
    });
  }

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

  useEffect(() => {
    const unlisteners = Promise.all([
      listen<{ accountId: string; mailboxId: string }>("mailbox-changed", (event) => {
        void queryClient.invalidateQueries({ queryKey: ["mailboxes", event.payload.accountId] });
        void queryClient.invalidateQueries({ queryKey: ["messages", event.payload.accountId, event.payload.mailboxId] });
      }),
      listen<{ accountId: string }>("sync-progress", (event) => {
        void queryClient.invalidateQueries({ queryKey: ["sync-progress", event.payload.accountId] });
      }),
      listen<{ accountId: string; messageId: string }>("message-content-changed", (event) => {
        void queryClient.invalidateQueries({ queryKey: ["message", event.payload.accountId, event.payload.messageId] });
      }),
      listen<{ accountId: string; jobId: string; status: string; subject: string }>("send-job-changed", (event) => {
        if (event.payload.accountId !== selectedAccountId || event.payload.status !== "sent") return;
        setSentNotice({ id: event.payload.jobId, subject: event.payload.subject });
        void queryClient.invalidateQueries({ queryKey: ["drafts", selectedAccountId] });
      }),
      listen<{ accountId: string }>("pending-operation-changed", (event) => {
        void queryClient.invalidateQueries({ queryKey: ["mailboxes", event.payload.accountId] });
        void queryClient.invalidateQueries({ queryKey: ["messages", event.payload.accountId] });
        void queryClient.invalidateQueries({ queryKey: ["message", event.payload.accountId] });
        void queryClient.invalidateQueries({ queryKey: ["pending-operations", event.payload.accountId] });
      }),
    ]);
    return () => { void unlisteners.then((values) => values.forEach((unlisten) => unlisten())); };
  }, [queryClient, selectedAccountId]);

  useEffect(() => {
    if (!sentNotice) return;
    const timeout = window.setTimeout(() => setSentNotice(null), 4_500);
    return () => window.clearTimeout(timeout);
  }, [sentNotice]);

  useEffect(() => {
    const handleResize = () => setWindowWidth(window.innerWidth);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  useEffect(() => {
    const available = Math.max(500, windowWidth - 372);
    let folder = folderPaneCollapsed ? 72 : Math.min(350, Math.max(220, folderPaneWidth));
    let messages = Math.min(520, Math.max(310, messagePaneWidth));
    let overflow = folder + messages - available;
    if (overflow > 0) {
      const messageReduction = Math.min(overflow, messages - 310);
      messages -= messageReduction;
      overflow -= messageReduction;
    }
    if (overflow > 0 && !folderPaneCollapsed) folder -= Math.min(overflow, folder - 220);
    if (!folderPaneCollapsed) setFolderPaneWidth(folder);
    setMessagePaneWidth(messages);
  }, [windowWidth, folderPaneCollapsed]);

  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--shell-sidebar-width", `${titlebarSidebarWidth}px`);
    return () => {
      document.documentElement.style.removeProperty("--shell-sidebar-width");
    };
  }, [titlebarSidebarWidth]);

  const folderPaneMax = Math.max(220, Math.min(350, windowWidth - messagePaneWidth - 372));
  const messagePaneMax = Math.max(310, Math.min(520, windowWidth - visibleFolderWidth - 372));

  function receive() {
    if (!selectedAccountId) return;
    setComposeError(null);
    void api.syncNow(selectedAccountId)
      .then(() => queryClient.invalidateQueries({ queryKey: ["sync-progress", selectedAccountId] }))
      .catch((error) => setComposeError(normalizeCommandError(error).code));
  }

  if (!accounts.length) {
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <EmptyAccountState
          title={t("accounts.noAccount")}
          description={t("accounts.noAccountDescription")}
          actionLabel={t("accounts.add")}
          onAdd={() => void api.openSettingsWindow().catch((error) => setComposeError(normalizeCommandError(error).code))}
        />
      </AppShell>
    );
  }

  return (
    <AppShell
      className="grid overflow-hidden bg-card"
      style={{ gridTemplateColumns: `${visibleFolderWidth}px 1px minmax(0,1fr)` }}
    >
      <Page className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] bg-sidebar">
        <AccountSwitcher
          accounts={accounts}
          runtimeSummaries={runtimeQuery.data ?? []}
          selectedAccountId={selectedAccountId}
          onAccountChange={selectAccount}
          collapsed={folderPaneCollapsed}
        />
        <MailboxPane
          mailboxes={mailboxesQuery.data ?? []}
          selectedMailboxId={selectedMailboxId}
          onSelect={(mailboxId) => {
            setSelectedMailboxId(mailboxId);
            setSelectedMessageId("");
            setSearchQuery("");
          }}
          progress={progressQuery.data}
          error={mailboxesQuery.error}
          onCompose={() => {
            if (!selectedAccountId) return;
            setComposeError(null);
            void api.openComposer(selectedAccountId)
              .then(() => queryClient.invalidateQueries({ queryKey: ["drafts", selectedAccountId] }))
              .catch((error) => setComposeError(normalizeCommandError(error).code));
          }}
          drafts={draftsQuery.data ?? []}
          onOpenDraft={(draftId) => void api.openExistingComposer(selectedAccountId, draftId).catch((error) => setComposeError(normalizeCommandError(error).code))}
          onDeleteDraft={async (draftId) => {
            try {
              await api.deleteDraft(selectedAccountId, draftId);
              await queryClient.invalidateQueries({ queryKey: ["drafts", selectedAccountId] });
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
      <Page className="grid min-h-0 bg-card" style={{ gridTemplateColumns: `${messagePaneWidth}px 1px minmax(360px,1fr)` }}>
        <Page className="flex min-h-0 flex-col bg-card">
          <MessageListPane
            accountId={selectedAccountId}
            mailboxId={selectedMailboxId}
            mailbox={selectedMailbox}
            selectedMessageId={selectedMessageId}
            onSelect={setSelectedMessageId}
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
            onMessageRemoved={() => setSelectedMessageId("")}
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
              <Button variant="secondary" size="sm" onClick={() => void api.retryPendingOperation(selectedAccountId, pendingIssue.id).then(() => queryClient.invalidateQueries({ queryKey: ["pending-operations", selectedAccountId] }))}>{t("common.retry")}</Button>
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

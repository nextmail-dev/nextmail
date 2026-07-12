import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountSummary, AppearancePreferences } from "@/app/types";
import { AboutDialog } from "@/features/about/AboutDialog";
import { AccountManagementDialog } from "@/features/accounts/AccountManagementDialog";
import { SettingsDialog } from "@/features/preferences/SettingsDialog";
import { AppShell, Page } from "@/components/ui/layout";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { MailToolbar } from "./MailToolbar";
import { MailboxPane } from "./MailboxPane";
import { MessageListPane } from "./MessageListPane";
import { MessageViewer } from "./MessageViewer";

interface MainShellProps {
  accounts: AccountSummary[];
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
}

export function MainShell({ accounts, preferences, onPreferencesChange }: MainShellProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [selectedAccountId, setSelectedAccountId] = useState(accounts[0]?.id ?? "");
  const [selectedMailboxId, setSelectedMailboxId] = useState("");
  const [selectedMessageId, setSelectedMessageId] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [accountsOpen, setAccountsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [composeError, setComposeError] = useState<string | null>(null);
  const selectedAccount =
    accounts.find((account) => account.id === selectedAccountId) ?? accounts[0];
  const aboutQuery = useQuery({
    queryKey: ["about"],
    queryFn: api.getAppAbout,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", selectedAccountId],
    queryFn: () => api.listMailboxes(selectedAccountId),
    enabled: Boolean(selectedAccountId),
  });
  const progressQuery = useQuery({
    queryKey: ["sync-progress", selectedAccountId],
    queryFn: () => api.getSyncProgress(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: (query) =>
      ["complete", "failed"].includes(query.state.data?.phase ?? "idle") ? false : 1_500,
  });
  const draftsQuery = useQuery({
    queryKey: ["drafts", selectedAccountId],
    queryFn: () => api.listDrafts(selectedAccountId),
    enabled: Boolean(selectedAccountId),
    refetchInterval: 3_000,
  });

  useEffect(() => {
    if (selectedAccountId && accounts.some((account) => account.id === selectedAccountId)) return;
    setSelectedAccountId(accounts[0]?.id ?? "");
  }, [accounts, selectedAccountId]);

  useEffect(() => {
    setSelectedMailboxId("");
    setSelectedMessageId("");
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
        if (event.payload.accountId !== selectedAccountId) return;
        void queryClient.invalidateQueries({ queryKey: ["mailboxes", selectedAccountId] });
        void queryClient.invalidateQueries({
          queryKey: ["messages", selectedAccountId, event.payload.mailboxId],
        });
      }),
      listen<{ accountId: string }>("sync-progress", (event) => {
        if (event.payload.accountId !== selectedAccountId) return;
        void queryClient.invalidateQueries({ queryKey: ["sync-progress", selectedAccountId] });
      }),
      listen<{ accountId: string; messageId: string }>("message-content-changed", (event) => {
        if (event.payload.accountId !== selectedAccountId) return;
        void queryClient.invalidateQueries({
          queryKey: ["message", selectedAccountId, event.payload.messageId],
        });
      }),
    ]);
    return () => {
      void unlisteners.then((values) => values.forEach((unlisten) => unlisten()));
    };
  }, [queryClient, selectedAccountId]);

  return (
    <AppShell className="grid grid-rows-[3.5rem_minmax(0,1fr)] overflow-hidden">
      <MailToolbar
        accounts={accounts}
        selectedAccountId={selectedAccountId}
        onAccountChange={setSelectedAccountId}
        onCompose={() => {
          if (!selectedAccountId) return;
          setComposeError(null);
          void api.openComposer(selectedAccountId)
            .then(() => queryClient.invalidateQueries({ queryKey: ["drafts", selectedAccountId] }))
            .catch((error) => setComposeError(normalizeCommandError(error).code));
        }}
        drafts={draftsQuery.data ?? []}
        onOpenDraft={(draftId) => {
          setComposeError(null);
          void api.openExistingComposer(selectedAccountId, draftId)
            .catch((error) => setComposeError(normalizeCommandError(error).code));
        }}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenAccounts={() => setAccountsOpen(true)}
        onOpenAbout={() => setAboutOpen(true)}
        onQuit={() => void api.quitApp()}
      />

      <Page className="grid min-h-0 grid-cols-[15.625rem_minmax(20rem,24.375rem)_minmax(24rem,1fr)]">
        <Page className="flex min-h-0 flex-col border-r border-border bg-muted/40">
          <MailboxPane
            mailboxes={mailboxesQuery.data ?? []}
            selectedMailboxId={selectedMailboxId}
            onSelect={(mailboxId) => {
              setSelectedMailboxId(mailboxId);
              setSelectedMessageId("");
            }}
            progress={progressQuery.data}
            error={mailboxesQuery.error}
          />
        </Page>

        <Page className="flex min-h-0 flex-col border-r border-border bg-background">
          <MessageListPane
            accountId={selectedAccountId}
            mailboxId={selectedMailboxId}
            selectedMessageId={selectedMessageId}
            onSelect={setSelectedMessageId}
          />
        </Page>

        <Page className="flex min-h-0 flex-col bg-background">
          <MessageViewer accountId={selectedAccountId} messageId={selectedMessageId} />
        </Page>
      </Page>

      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        preferences={preferences}
        onChange={onPreferencesChange}
      />
      <AccountManagementDialog
        open={accountsOpen}
        onOpenChange={setAccountsOpen}
        accountId={selectedAccount?.id ?? ""}
      />
      <AboutDialog
        open={aboutOpen}
        onOpenChange={setAboutOpen}
        version={aboutQuery.data?.version ?? "0.1.0"}
      />
      {composeError ? (
        <Alert className="fixed right-4 bottom-4 z-40 max-w-sm bg-popover shadow-xl" tone="danger">
          {t(`errors.${composeError}`, { defaultValue: t("common.unexpectedError") })}
          <Button variant="ghost" size="icon" aria-label={t("common.close")} onClick={() => setComposeError(null)}>
            <X size={15} />
          </Button>
        </Alert>
      ) : null}
    </AppShell>
  );
}

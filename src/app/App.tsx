import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Component, lazy, Suspense, useEffect, useState, type ErrorInfo, type ReactNode } from "react";
import { QueryClient, QueryClientProvider, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Mail } from "lucide-react";
import { useTranslation } from "react-i18next";
import i18n from "./i18n";
import { api, normalizeCommandError } from "./api";
import { reportCaughtError } from "./errorReporting";
import {
  useAppearanceEventBridge,
  useAppearancePreferences,
  useUpdateAppearancePreferences,
} from "./appearance";
import type { AppearancePreferences, DesktopPreferences, MainCloseAction, ReadingPreferences } from "./types";
import { AccountStep } from "../features/onboarding/AccountStep";
import { DataDirectoryStep } from "../features/onboarding/DataDirectoryStep";
import { MainShell } from "../features/mail/MainShell";
import { WelcomeStep } from "../features/onboarding/WelcomeStep";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Modal } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { IconTile } from "@/components/ui/icon-tile";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Text } from "@/components/ui/typography";
import { WindowTitlebar, type WindowKind } from "@/components/window/WindowTitlebar";
import { StartupUpdateChecker } from "@/features/preferences/UpdateSettings";

const ComposerApp = lazy(() =>
  import("@/features/composer/ComposerApp").then((module) => ({ default: module.ComposerApp })),
);
const SettingsApp = lazy(() =>
  import("@/features/preferences/SettingsApp").then((module) => ({ default: module.SettingsApp })),
);
const AccountManagementApp = lazy(() =>
  import("@/features/accounts/AccountManagementApp").then((module) => ({ default: module.AccountManagementApp })),
);
const RawMessageApp = lazy(() =>
  import("@/features/mail/RawMessageApp").then((module) => ({ default: module.RawMessageApp })),
);
const MessagePreviewApp = lazy(() =>
  import("@/features/mail/MessagePreviewApp").then((module) => ({ default: module.MessagePreviewApp })),
);
const CompositionDefinitionEditorApp = lazy(() =>
  import("@/features/preferences/CompositionDefinitionEditorApp")
    .then((module) => ({ default: module.CompositionDefinitionEditorApp })),
);
const NotificationApp = lazy(() =>
  import("@/features/notifications/NotificationApp").then((module) => ({ default: module.NotificationApp })),
);
const UpdateWindowApp = lazy(() =>
  import("@/features/preferences/UpdateWindowApp").then((module) => ({ default: module.UpdateWindowApp })),
);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 15_000 },
  },
});

export function App() {
  const params = new URLSearchParams(window.location.search);
  const composer = params.get("window") === "composer";
  const settings = params.get("window") === "settings";
  const accountManagement = params.get("window") === "accounts";
  const rawMessage = params.get("window") === "raw-message";
  const messagePreview = params.get("window") === "message-preview";
  const definitionEditor = params.get("window") === "definition";
  const notification = params.get("window") === "notification";
  const update = params.get("window") === "update";
  const notificationId = params.get("notificationId") ?? "";
  const accountId = params.get("accountId") ?? "";
  const messageId = params.get("messageId") ?? "";
  const mailboxId = params.get("mailboxId") ?? "";
  const draftId = params.get("draftId") ?? "";
  const definitionKind = params.get("kind");
  const definitionId = params.get("definitionId");
  if (notification && notificationId) {
    return (
      <QueryClientProvider client={queryClient}>
        <AppearanceEventBridge />
        <Suspense fallback={<AppShell className="grid place-items-center bg-card"><Spinner size={20} /></AppShell>}>
          <NotificationApp notificationId={notificationId} />
        </Suspense>
      </QueryClientProvider>
    );
  }
  const kind: WindowKind = composer
    ? "composer"
    : settings
      ? "settings"
      : accountManagement
        ? "accounts"
        : messagePreview
          ? "message-preview"
          : rawMessage
            ? "raw-message"
            : definitionEditor
              ? "definition"
              : update
                ? "update"
                : "main";
  const windowContent = composer && accountId && draftId
    ? <ComposerApp accountId={accountId} draftId={draftId} />
    : settings
      ? <SettingsApp />
      : accountManagement
        ? <AccountManagementApp />
        : messagePreview && accountId && mailboxId && messageId
          ? <MessagePreviewApp accountId={accountId} mailboxId={mailboxId} messageId={messageId} />
          : rawMessage && accountId && messageId
            ? <RawMessageApp accountId={accountId} messageId={messageId} />
            : definitionEditor && (definitionKind === "template" || definitionKind === "signature")
              ? (
                <CompositionDefinitionEditorApp
                  accountId={accountId || null}
                  kind={definitionKind}
                  definitionId={definitionId}
                />
              )
              : update
                ? <UpdateWindowApp />
                : <AppContent />;
  return (
    <QueryClientProvider client={queryClient}>
      <AppearanceEventBridge />
      <ReadingPreferencesEventBridge />
      <DesktopPreferencesEventBridge />
      <AccountsEventBridge />
      {kind === "main" ? (
        <WindowFrame kind={kind}>
          <WindowContentBoundary kind={kind}>
            {windowContent}
          </WindowContentBoundary>
          <MainCloseDialog />
          <StartupUpdateChecker />
        </WindowFrame>
      ) : (
        <WindowContentBoundary kind={kind}>
          <Suspense fallback={<AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>}>
            <WindowFrame kind={kind}>{windowContent}</WindowFrame>
          </Suspense>
        </WindowContentBoundary>
      )}
    </QueryClientProvider>
  );
}

function WindowFrame({ kind, children }: { kind: WindowKind; children: ReactNode }) {
  return (
    <>
      <WindowTitlebar kind={kind} />
      <div className="h-full pt-[var(--titlebar-height)]">{children}</div>
    </>
  );
}

class WindowContentBoundary extends Component<
  { kind: WindowKind; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    reportCaughtError(
      `react.error-boundary.${this.props.kind}`,
      new Error(`${error.message}\n${info.componentStack ?? ""}`),
    );
    if (this.props.kind !== "main" && "__TAURI_INTERNALS__" in globalThis) {
      const appWindow = getCurrentWindow();
      void appWindow.show()
        .then(() => appWindow.setFocus())
        .catch((revealError) => reportCaughtError("window.reveal-error-state", revealError));
    }
  }

  private closeWindow = () => {
    const appWindow = getCurrentWindow();
    void (["settings", "accounts", "message-preview", "raw-message", "definition", "update"].includes(this.props.kind)
      ? appWindow.destroy()
      : appWindow.close());
  };

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <EmptyState
          icon={<AlertTriangle size={28} />}
          title={i18n.t("errors.title")}
          description={i18n.t("common.unexpectedError")}
          action={<Button onClick={this.closeWindow}>{i18n.t("common.close")}</Button>}
        />
      </AppShell>
    );
  }
}

function AppearanceEventBridge() {
  useAppearancePreferences();
  useAppearanceEventBridge();
  return null;
}

function ReadingPreferencesEventBridge() {
  const queryCache = useQueryClient();
  useEffect(() => {
    const unlisten = listen<ReadingPreferences>("reading-preferences-changed", (event) => {
      queryCache.setQueryData(["reading-preferences"], event.payload);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [queryCache]);
  return null;
}

function DesktopPreferencesEventBridge() {
  const queryCache = useQueryClient();
  useEffect(() => {
    const unlisten = listen<DesktopPreferences>("desktop-preferences-changed", (event) => {
      queryCache.setQueryData(["desktop-preferences"], event.payload);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [queryCache]);
  return null;
}

function MainCloseDialog() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);
  const [closeError, setCloseError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen("main-close-confirmation-requested", () => {
      setRemember(false);
      setCloseError(null);
      setOpen(true);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);

  async function resolve(action: MainCloseAction) {
    setBusy(true);
    setCloseError(null);
    try {
      await api.resolveMainClose(action, remember);
      setOpen(false);
    } catch (error) {
      reportCaughtError("window.main-close-resolution", error);
      setCloseError(normalizeCommandError(error).code);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={(nextOpen) => { if (!busy) setOpen(nextOpen); }}
      title={t("desktop.closeTitle")}
      closeLabel={t("common.close")}
    >
      <Stack className="pt-4" gap="lg">
        <Text>{t("desktop.closeDescription")}</Text>
        <Checkbox
          checked={remember}
          label={t("desktop.rememberCloseChoice")}
          onCheckedChange={setRemember}
        />
        {closeError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${closeError}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={busy} onClick={() => setOpen(false)}>
            {t("common.cancel")}
          </Button>
          <Button variant="secondary" disabled={busy} onClick={() => void resolve("quit")}>
            {t("desktop.quit")}
          </Button>
          <Button disabled={busy} onClick={() => void resolve("minimize_to_tray")}>
            {t("desktop.minimizeToTrayAction")}
          </Button>
        </div>
      </Stack>
    </Modal>
  );
}

function AccountsEventBridge() {
  const queryCache = useQueryClient();
  useEffect(() => {
    const changes = listen<{ revision: number }>("accounts-changed", () => {
      void queryCache.invalidateQueries({ queryKey: ["accounts"] });
      void queryCache.invalidateQueries({ queryKey: ["bootstrap"] });
      void queryCache.invalidateQueries({ queryKey: ["account-runtimes"] });
    });
    const runtime = listen<{ accountId: string }>("account-runtime-status-changed", () => {
      void queryCache.invalidateQueries({ queryKey: ["account-runtimes"] });
    });
    return () => {
      void changes.then((dispose) => dispose());
      void runtime.then((dispose) => dispose());
    };
  }, [queryCache]);
  return null;
}

function AppContent() {
  const { t } = useTranslation();
  const queryCache = useQueryClient();
  const [welcomeCompleted, setWelcomeCompleted] = useState(false);
  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: api.getBootstrapStatus,
  });
  const preferencesQuery = useAppearancePreferences();
  const preferencesMutation = useUpdateAppearancePreferences();

  function changePreferences(preferences: AppearancePreferences) {
    preferencesMutation.mutate(preferences);
  }

  async function refreshBootstrap() {
    await queryCache.invalidateQueries({ queryKey: ["bootstrap"] });
  }

  if (bootstrapQuery.isPending || preferencesQuery.isPending) {
    return (
      <AppShell className="grid place-items-center">
        <Page className="grid min-h-full place-items-center">
          <Stack className="items-center text-center" gap="md">
            <IconTile large>
              <Mail size={26} />
            </IconTile>
            <Spinner size={26} />
            <Text>{t("common.loading")}</Text>
          </Stack>
        </Page>
      </AppShell>
    );
  }

  if (bootstrapQuery.isError || preferencesQuery.isError || !bootstrapQuery.data) {
    const error = normalizeCommandError(bootstrapQuery.error ?? preferencesQuery.error);
    return (
      <AppShell className="grid place-items-center p-8">
        <Page className="flex max-w-md flex-col items-center gap-4">
          <EmptyState
            icon={<AlertTriangle size={28} />}
            title={t("errors.title")}
            description={t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          />
          <Alert tone="danger">{t("common.unexpectedError")}</Alert>
          <Button onClick={() => void refreshBootstrap()}>{t("common.retry")}</Button>
        </Page>
      </AppShell>
    );
  }

  const status = bootstrapQuery.data;
  if (status.stage === "needs_data_directory" && !welcomeCompleted) {
    return (
      <WelcomeStep
        preferences={preferencesQuery.data}
        onPreferencesChange={changePreferences}
        onContinue={() => setWelcomeCompleted(true)}
      />
    );
  }
  if (status.stage === "needs_data_directory" || status.stage === "data_directory_missing") {
    return (
      <DataDirectoryStep
        status={status}
        preferences={preferencesQuery.data}
        onPreferencesChange={changePreferences}
        onCompleted={() => void refreshBootstrap()}
      />
    );
  }
  if (status.stage === "needs_account") {
    return (
      <AccountStep
        preferences={preferencesQuery.data}
        onPreferencesChange={changePreferences}
        onCompleted={() => void refreshBootstrap()}
      />
    );
  }
  return (
    <MainShell accounts={status.accounts} lastSelectedAccountId={status.lastSelectedAccountId} />
  );
}

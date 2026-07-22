import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Component, lazy, Suspense, useEffect, useState, type ErrorInfo, type ReactNode } from "react";
import { QueryClient, QueryClientProvider, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Mail } from "lucide-react";
import { useTranslation } from "react-i18next";
import i18n from "./i18n";
import { api, normalizeCommandError } from "./api";
import {
  useAppearanceEventBridge,
  useAppearancePreferences,
  useUpdateAppearancePreferences,
} from "./appearance";
import type { AppearancePreferences, ReadingPreferences } from "./types";
import { AccountStep } from "../features/onboarding/AccountStep";
import { DataDirectoryStep } from "../features/onboarding/DataDirectoryStep";
import { MainShell } from "../features/mail/MainShell";
import { WelcomeStep } from "../features/onboarding/WelcomeStep";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { IconTile } from "@/components/ui/icon-tile";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Text } from "@/components/ui/typography";
import { WindowTitlebar, type WindowKind } from "@/components/window/WindowTitlebar";

const ComposerApp = lazy(() =>
  import("@/features/composer/ComposerApp").then((module) => ({ default: module.ComposerApp })),
);
const SettingsApp = lazy(() =>
  import("@/features/preferences/SettingsApp").then((module) => ({ default: module.SettingsApp })),
);
const NotificationApp = lazy(() =>
  import("@/features/notifications/NotificationApp").then((module) => ({ default: module.NotificationApp })),
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
  const notification = params.get("window") === "notification";
  const notificationId = params.get("notificationId") ?? "";
  const accountId = params.get("accountId") ?? "";
  const draftId = params.get("draftId") ?? "";
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
  const kind: WindowKind = composer ? "composer" : settings ? "settings" : "main";
  return (
    <QueryClientProvider client={queryClient}>
      <WindowTitlebar kind={kind} title={kind === "main" ? "" : "NextMail"} />
      <AppearanceEventBridge />
      <ReadingPreferencesEventBridge />
      <AccountsEventBridge />
      <ScrollActivityBridge />
      <div className="h-full pt-[var(--titlebar-height)]">
        <WindowContentBoundary kind={kind}>
          {composer && accountId && draftId ? (
            <Suspense fallback={<AppShell className="grid place-items-center"><Spinner size={24} /></AppShell>}>
              <ComposerApp accountId={accountId} draftId={draftId} />
            </Suspense>
          ) : settings ? (
            <Suspense fallback={<AppShell className="grid place-items-center"><Spinner size={24} /></AppShell>}>
              <SettingsApp />
            </Suspense>
          ) : (
            <AppContent />
          )}
        </WindowContentBoundary>
      </div>
    </QueryClientProvider>
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
    console.error("NextMail window content failed to render", error, info);
  }

  private closeWindow = () => {
    const appWindow = getCurrentWindow();
    void (this.props.kind === "settings" ? appWindow.destroy() : appWindow.close());
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

function ScrollActivityBridge() {
  useEffect(() => {
    const timers = new Map<Element, number>();
    const markActive = (event: Event) => {
      const target = event.target instanceof Element
        ? event.target
        : document.scrollingElement;
      if (!target) return;
      target.classList.add("is-scrolling");
      const previous = timers.get(target);
      if (previous !== undefined) window.clearTimeout(previous);
      timers.set(target, window.setTimeout(() => {
        target.classList.remove("is-scrolling");
        timers.delete(target);
      }, 700));
    };
    window.addEventListener("scroll", markActive, true);
    return () => {
      window.removeEventListener("scroll", markActive, true);
      timers.forEach((timer, target) => {
        window.clearTimeout(timer);
        target.classList.remove("is-scrolling");
      });
    };
  }, []);
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

import { lazy, Suspense, useEffect, useState } from "react";
import { QueryClient, QueryClientProvider, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Mail } from "lucide-react";
import { useTranslation } from "react-i18next";
import i18n from "./i18n";
import { api, normalizeCommandError } from "./api";
import { applyAppearance, useAppearanceStore } from "./appearance";
import type { AppearancePreferences } from "./types";
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

const ComposerApp = lazy(() =>
  import("@/features/composer/ComposerApp").then((module) => ({ default: module.ComposerApp })),
);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 15_000 },
  },
});

export function App() {
  const params = new URLSearchParams(window.location.search);
  const composer = params.get("window") === "composer";
  const accountId = params.get("accountId") ?? "";
  const draftId = params.get("draftId") ?? "";
  return (
    <QueryClientProvider client={queryClient}>
      {composer && accountId && draftId ? (
        <Suspense fallback={<AppShell className="grid place-items-center"><Spinner size={24} /></AppShell>}>
          <ComposerApp accountId={accountId} draftId={draftId} />
        </Suspense>
      ) : (
        <AppContent />
      )}
    </QueryClientProvider>
  );
}

function AppContent() {
  const { t } = useTranslation();
  const queryCache = useQueryClient();
  const preferencesState = useAppearanceStore();
  const [welcomeCompleted, setWelcomeCompleted] = useState(false);
  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: api.getBootstrapStatus,
  });
  const preferencesQuery = useQuery({
    queryKey: ["preferences"],
    queryFn: api.getPreferences,
  });
  const preferencesMutation = useMutation({
    mutationFn: api.setAppearancePreferences,
    onSuccess: (preferences) => {
      queryCache.setQueryData(["preferences"], preferences);
    },
  });

  useEffect(() => {
    if (!preferencesQuery.data) return;
    preferencesState.setPreferences(preferencesQuery.data);
    applyAppearance(preferencesQuery.data);
    void i18n.changeLanguage(preferencesQuery.data.language);
  }, [preferencesQuery.data, preferencesState.setPreferences]);

  function changePreferences(preferences: AppearancePreferences) {
    const previous = preferencesState.preferences;
    preferencesState.setPreferences(preferences);
    applyAppearance(preferences);
    void i18n.changeLanguage(preferences.language);
    preferencesMutation.mutate(preferences, {
      onError: () => {
        preferencesState.setPreferences(previous);
        applyAppearance(previous);
        void i18n.changeLanguage(previous.language);
      },
    });
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
        preferences={preferencesState.preferences}
        onPreferencesChange={changePreferences}
        onContinue={() => setWelcomeCompleted(true)}
      />
    );
  }
  if (status.stage === "needs_data_directory" || status.stage === "data_directory_missing") {
    return (
      <DataDirectoryStep
        status={status}
        preferences={preferencesState.preferences}
        onPreferencesChange={changePreferences}
        onCompleted={() => void refreshBootstrap()}
      />
    );
  }
  if (status.stage === "needs_account") {
    return (
      <AccountStep
        preferences={preferencesState.preferences}
        onPreferencesChange={changePreferences}
        onCompleted={() => void refreshBootstrap()}
      />
    );
  }
  return (
    <MainShell
      accounts={status.accounts}
      preferences={preferencesState.preferences}
      onPreferencesChange={changePreferences}
    />
  );
}

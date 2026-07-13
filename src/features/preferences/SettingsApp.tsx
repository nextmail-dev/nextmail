import {
  Bell,
  BookOpen,
  CircleUserRound,
  Info,
  Languages,
  Palette,
  PenLine,
  SlidersHorizontal,
} from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { applyAppearance, useAppearanceStore } from "@/app/appearance";
import i18n from "@/app/i18n";
import type { AppearancePreferences, LanguagePreference, ThemePreference } from "@/app/types";
import { AccountManagementPanel } from "@/features/accounts/AccountManagementDialog";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";

type SettingsCategory =
  | "general"
  | "appearance"
  | "accounts"
  | "reading"
  | "composer"
  | "notifications"
  | "advanced"
  | "about";

const categories: Array<{ id: SettingsCategory; icon: typeof Languages }> = [
  { id: "general", icon: Languages },
  { id: "appearance", icon: Palette },
  { id: "accounts", icon: CircleUserRound },
  { id: "reading", icon: BookOpen },
  { id: "composer", icon: PenLine },
  { id: "notifications", icon: Bell },
  { id: "advanced", icon: SlidersHorizontal },
  { id: "about", icon: Info },
];

export function SettingsApp() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const appearance = useAppearanceStore();
  const [category, setCategory] = useState<SettingsCategory>("general");
  const preferencesQuery = useQuery({ queryKey: ["preferences"], queryFn: api.getPreferences });
  const accountsQuery = useQuery({ queryKey: ["accounts"], queryFn: api.listAccountSummaries });
  const aboutQuery = useQuery({ queryKey: ["about"], queryFn: api.getAppAbout });
  const mutation = useMutation({
    mutationFn: api.setAppearancePreferences,
    onSuccess: (preferences) => queryClient.setQueryData(["preferences"], preferences),
  });
  const accountId = accountsQuery.data?.[0]?.id ?? "";

  useEffect(() => {
    if (!preferencesQuery.data) return;
    appearance.setPreferences(preferencesQuery.data);
    applyAppearance(preferencesQuery.data);
    void i18n.changeLanguage(preferencesQuery.data.language);
  }, [appearance.setPreferences, preferencesQuery.data]);

  function updatePreferences(preferences: AppearancePreferences) {
    const previous = appearance.preferences;
    appearance.setPreferences(preferences);
    applyAppearance(preferences);
    void i18n.changeLanguage(preferences.language);
    mutation.mutate(preferences, {
      onError: () => {
        appearance.setPreferences(previous);
        applyAppearance(previous);
        void i18n.changeLanguage(previous.language);
      },
    });
  }

  if (preferencesQuery.isPending || accountsQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (preferencesQuery.isError || accountsQuery.isError || !preferencesQuery.data) {
    const error = normalizeCommandError(preferencesQuery.error ?? accountsQuery.error);
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      </AppShell>
    );
  }
  const preferences = preferencesQuery.data;

  return (
    <AppShell className="grid grid-cols-[240px_minmax(0,1fr)] overflow-hidden bg-card">
      <Page className="flex min-h-0 flex-col bg-sidebar px-3 pt-5 pb-4">
        <Stack className="px-3 pb-5" gap="xs">
          <Heading level={1} className="text-xl">{t("settings.title")}</Heading>
          <Text className="text-xs">{t("settings.description")}</Text>
        </Stack>
        <nav className="flex min-h-0 flex-1 flex-col gap-1" aria-label={t("settings.categories") }>
          {categories.map((item) => {
            const Icon = item.icon;
            return (
              <Button
                key={item.id}
                variant="ghost"
                className={category === item.id
                  ? "h-10 w-full justify-start bg-card px-3 text-foreground shadow-[0_5px_18px_rgb(15_23_42/0.05)] hover:bg-card"
                  : "h-10 w-full justify-start px-3"}
                aria-current={category === item.id ? "page" : undefined}
                onClick={() => setCategory(item.id)}
              >
                <Icon size={17} />
                {t(`settings.category.${item.id}`)}
              </Button>
            );
          })}
        </nav>
      </Page>
      <Page className="min-h-0 overflow-auto bg-card px-10 py-8">
        <SettingsContent
          category={category}
          preferences={preferences}
          accountId={accountId}
          version={aboutQuery.data?.version ?? "0.1.0"}
          onChange={updatePreferences}
        />
      </Page>
    </AppShell>
  );
}

function SettingsContent({
  category,
  preferences,
  accountId,
  version,
  onChange,
}: {
  category: SettingsCategory;
  preferences: AppearancePreferences;
  accountId: string;
  version: string;
  onChange: (preferences: AppearancePreferences) => void;
}) {
  const { t } = useTranslation();
  if (category === "general") {
    return (
      <SettingsSection category={category}>
        <SelectField
          label={t("preferences.language")}
          value={preferences.language}
          options={[
            { value: "zh-CN", label: t("preferences.chinese") },
            { value: "en-US", label: t("preferences.english") },
          ]}
          onValueChange={(language) => onChange({ ...preferences, language: language as LanguagePreference })}
        />
      </SettingsSection>
    );
  }
  if (category === "appearance") {
    return (
      <SettingsSection category={category}>
        <SelectField
          label={t("preferences.theme")}
          value={preferences.theme}
          options={[
            { value: "system", label: t("preferences.system") },
            { value: "light", label: t("preferences.light") },
            { value: "dark", label: t("preferences.dark") },
          ]}
          onValueChange={(theme) => onChange({ ...preferences, theme: theme as ThemePreference })}
        />
        <SelectField
          label={t("preferences.accent")}
          value={preferences.accentColor}
          options={[
            { value: "#2563eb", label: t("preferences.blue") },
            { value: "#7c3aed", label: t("preferences.violet") },
            { value: "#0f8a7b", label: t("preferences.teal") },
            { value: "#d13c68", label: t("preferences.rose") },
          ]}
          onValueChange={(accentColor) => onChange({ ...preferences, accentColor })}
        />
      </SettingsSection>
    );
  }
  if (category === "accounts") {
    return (
      <SettingsSection category={category}>
        {accountId ? (
          <AccountManagementPanel accountId={accountId} />
        ) : (
          <EmptyState icon={<CircleUserRound size={24} />} title={t("settings.noAccount")} />
        )}
      </SettingsSection>
    );
  }
  if (category === "about") {
    return (
      <SettingsSection category={category}>
        <Stack className="rounded-lg bg-muted/60 p-5" gap="sm">
          <Heading level={2}>NextMail</Heading>
          <Text>{t("about.version", { version })}</Text>
          <Text>{t("about.description")}</Text>
        </Stack>
      </SettingsSection>
    );
  }
  return (
    <SettingsSection category={category}>
      <EmptyState
        icon={<SlidersHorizontal size={24} />}
        title={t("settings.noOptions")}
        description={t("settings.noOptionsDescription")}
      />
    </SettingsSection>
  );
}

function SettingsSection({ category, children }: { category: SettingsCategory; children: ReactNode }) {
  const { t } = useTranslation();
  return (
    <Stack className="mx-auto w-full max-w-2xl" gap="lg">
      <Stack gap="xs">
        <Heading level={1}>{t(`settings.category.${category}`)}</Heading>
        <Text>{t(`settings.categoryDescription.${category}`)}</Text>
      </Stack>
      <Stack className="pt-2" gap="lg">{children}</Stack>
    </Stack>
  );
}

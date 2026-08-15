import {
  Bell,
  BookOpen,
  Info,
  Languages,
  Palette,
  PenLine,
  SlidersHorizontal,
} from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useAppearancePreferences, useUpdateAppearancePreferences } from "@/app/appearance";
import type {
  AccountSummary,
  AppearancePreferences,
  DesktopPreferences,
  LanguagePreference,
  ReadingPreferences,
} from "@/app/types";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { EmptyState } from "@/components/ui/empty-state";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { ThemeColorPicker, type ThemeColorOption } from "@/components/ui/theme-color-picker";
import { ThemeModePicker } from "@/components/ui/theme-mode-picker";
import { Heading, Text } from "@/components/ui/typography";
import { CompositionDefinitionsSettings } from "./CompositionDefinitionsSettings";
import { NotificationSettings } from "./NotificationSettings";
import { UpdateSettings } from "./UpdateSettings";

type SettingsCategory =
  | "general"
  | "appearance"
  | "reading"
  | "composer"
  | "notifications"
  | "advanced"
  | "about";

const categories: Array<{ id: SettingsCategory; icon: typeof Languages }> = [
  { id: "general", icon: Languages },
  { id: "appearance", icon: Palette },
  { id: "reading", icon: BookOpen },
  { id: "composer", icon: PenLine },
  { id: "notifications", icon: Bell },
  { id: "advanced", icon: SlidersHorizontal },
  { id: "about", icon: Info },
];

const themeColors = [
  { value: "#2563eb", name: "blue" },
  { value: "#4f46e5", name: "indigo" },
  { value: "#7c3aed", name: "violet" },
  { value: "#9333ea", name: "purple" },
  { value: "#d13c68", name: "rose" },
  { value: "#dc2626", name: "red" },
  { value: "#ea580c", name: "orange" },
  { value: "#d97706", name: "amber" },
  { value: "#16a34a", name: "green" },
  { value: "#0f8a7b", name: "teal" },
] as const;

export function SettingsApp() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [category, setCategory] = useState<SettingsCategory>("general");
  const preferencesQuery = useAppearancePreferences();
  const readingPreferencesQuery = useQuery({ queryKey: ["reading-preferences"], queryFn: api.getReadingPreferences });
  const desktopPreferencesQuery = useQuery({ queryKey: ["desktop-preferences"], queryFn: api.getDesktopPreferences });
  const autostartQuery = useQuery({ queryKey: ["autostart"], queryFn: api.getAutostartEnabled });
  const accountsQuery = useQuery({ queryKey: ["accounts"], queryFn: api.listAccountSummaries });
  const aboutQuery = useQuery({ queryKey: ["about"], queryFn: api.getAppAbout });
  const mutation = useUpdateAppearancePreferences();
  const readingMutation = useMutation({
    mutationFn: api.setReadingPreferences,
    onSuccess: (preferences) => queryClient.setQueryData(["reading-preferences"], preferences),
  });
  const desktopMutation = useMutation({
    mutationFn: api.setDesktopPreferences,
    onSuccess: (preferences) => queryClient.setQueryData(["desktop-preferences"], preferences),
  });
  const autostartMutation = useMutation({
    mutationFn: api.setAutostartEnabled,
    onSuccess: (enabled) => queryClient.setQueryData(["autostart"], enabled),
  });
  useRevealWindowWhenReady(
    !preferencesQuery.isPending
      && !readingPreferencesQuery.isPending
      && !desktopPreferencesQuery.isPending
      && !accountsQuery.isPending,
  );

  function updatePreferences(preferences: AppearancePreferences) {
    mutation.mutate(preferences);
  }

  function updateReadingPreferences(preferences: ReadingPreferences) {
    const previous = readingPreferencesQuery.data;
    queryClient.setQueryData(["reading-preferences"], preferences);
    readingMutation.mutate(preferences, {
      onError: () => queryClient.setQueryData(["reading-preferences"], previous),
    });
  }

  function updateDesktopPreferences(preferences: DesktopPreferences) {
    const previous = desktopPreferencesQuery.data;
    queryClient.setQueryData(["desktop-preferences"], preferences);
    desktopMutation.mutate(preferences, {
      onError: () => queryClient.setQueryData(["desktop-preferences"], previous),
    });
  }

  function updateAutostart(enabled: boolean) {
    const previous = autostartQuery.data;
    queryClient.setQueryData(["autostart"], enabled);
    autostartMutation.mutate(enabled, {
      onError: () => queryClient.setQueryData(["autostart"], previous),
    });
  }

  if (
    preferencesQuery.isPending
    || readingPreferencesQuery.isPending
    || desktopPreferencesQuery.isPending
    || accountsQuery.isPending
  ) {
    return <AppShell className="fixed inset-0 z-[110] grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (
    preferencesQuery.isError
    || readingPreferencesQuery.isError
    || desktopPreferencesQuery.isError
    || accountsQuery.isError
    || !preferencesQuery.data
    || !readingPreferencesQuery.data
    || !desktopPreferencesQuery.data
  ) {
    const error = normalizeCommandError(
      preferencesQuery.error
      ?? readingPreferencesQuery.error
      ?? desktopPreferencesQuery.error
      ?? accountsQuery.error,
    );
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
    <AppShell className="grid grid-cols-[220px_minmax(0,1fr)] overflow-hidden bg-card">
      <Page className="flex min-h-0 flex-col border-r border-border-strong bg-sidebar px-3 pt-4 pb-4">
        <Stack className="px-3 pb-4" gap="xs">
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
                  ? "h-9 w-full justify-start bg-primary/10 px-3 text-primary shadow-[inset_2px_0_0_var(--primary)] hover:bg-primary/15"
                  : "h-9 w-full justify-start px-3"}
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
      <Page className="relative min-h-0 overflow-hidden bg-card">
        <OverlayScrollArea
          className="h-full"
          viewportClassName="px-8 py-7"
          trackClassName="right-2"
        >
          <SettingsContent
            category={category}
            preferences={preferences}
            readingPreferences={readingPreferencesQuery.data}
            desktopPreferences={desktopPreferencesQuery.data}
            readingError={readingMutation.error}
            desktopError={desktopMutation.error}
            accounts={accountsQuery.data ?? []}
            version={aboutQuery.data?.version ?? "0.2.3"}
            autostartEnabled={autostartQuery.data}
            autostartDisabled={autostartQuery.isPending || autostartQuery.isError}
            autostartError={autostartMutation.error}
            onChange={updatePreferences}
            onReadingChange={updateReadingPreferences}
            onDesktopChange={updateDesktopPreferences}
            onAutostartChange={updateAutostart}
          />
        </OverlayScrollArea>
      </Page>
    </AppShell>
  );
}

function SettingsContent({
  category,
  preferences,
  readingPreferences,
  desktopPreferences,
  readingError,
  desktopError,
  accounts,
  version,
  autostartEnabled,
  autostartDisabled,
  autostartError,
  onChange,
  onReadingChange,
  onDesktopChange,
  onAutostartChange,
}: {
  category: SettingsCategory;
  preferences: AppearancePreferences;
  readingPreferences: ReadingPreferences;
  desktopPreferences: DesktopPreferences;
  readingError: unknown;
  desktopError: unknown;
  accounts: AccountSummary[];
  version: string;
  autostartEnabled: boolean | undefined;
  autostartDisabled: boolean;
  autostartError: unknown;
  onChange: (preferences: AppearancePreferences) => void;
  onReadingChange: (preferences: ReadingPreferences) => void;
  onDesktopChange: (preferences: DesktopPreferences) => void;
  onAutostartChange: (enabled: boolean) => void;
}) {
  const { t } = useTranslation();
  if (category === "general") {
    const error = autostartError ? normalizeCommandError(autostartError) : null;
    return (
      <SettingsSection category={category}>
        <SettingsGroup title={t("settings.group.interface")}>
          <SelectField
            label={t("preferences.language")}
            value={preferences.language}
            options={[
              { value: "zh-CN", label: t("preferences.chinese") },
              { value: "en-US", label: t("preferences.english") },
            ]}
            onValueChange={(language) => onChange({ ...preferences, language: language as LanguagePreference })}
          />
        </SettingsGroup>
        <SettingsGroup title={t("settings.group.startup")}>
          <Checkbox
            checked={autostartEnabled ?? false}
            disabled={autostartDisabled}
            label={t("settings.launchAtStartup")}
            description={t("settings.launchAtStartupDescription")}
            onCheckedChange={onAutostartChange}
          />
        </SettingsGroup>
        {error ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
      </SettingsSection>
    );
  }
  if (category === "appearance") {
    const colorOptions: ThemeColorOption[] = themeColors.map((color) => ({
      value: color.value,
      label: t(`preferences.${color.name}`),
    }));
    if (!colorOptions.some((option) => option.value.toLowerCase() === preferences.accentColor.toLowerCase())) {
      colorOptions.push({ value: preferences.accentColor, label: t("preferences.customColor") });
    }
    return (
      <SettingsSection category={category}>
        <SettingsGroup title={t("settings.group.theme")}>
          <ThemeModePicker
            label={t("preferences.theme")}
            value={preferences.theme}
            options={[
              { value: "system", label: t("preferences.system") },
              { value: "light", label: t("preferences.light") },
              { value: "dark", label: t("preferences.dark") },
            ]}
            onValueChange={(theme) => onChange({ ...preferences, theme })}
          />
          <ThemeColorPicker
            label={t("preferences.themeColor")}
            value={preferences.accentColor}
            options={colorOptions}
            onValueChange={(accentColor) => onChange({ ...preferences, accentColor })}
          />
        </SettingsGroup>
      </SettingsSection>
    );
  }
  if (category === "reading") {
    const error = readingError ? normalizeCommandError(readingError) : null;
    return (
      <SettingsSection category={category}>
        <SettingsGroup title={t("settings.group.contentPrivacy")}>
          <Checkbox
            checked={readingPreferences.autoLoadRemoteImages}
            label={t("settings.autoLoadRemoteImages")}
            description={t("settings.autoLoadRemoteImagesDescription")}
            onCheckedChange={(autoLoadRemoteImages) => onReadingChange({ ...readingPreferences, autoLoadRemoteImages })}
          />
        </SettingsGroup>
        <SettingsGroup title={t("settings.group.listBehavior")}>
          <Checkbox
            checked={readingPreferences.autoLoadMoreMessages}
            label={t("settings.autoLoadMoreMessages")}
            description={t("settings.autoLoadMoreMessagesDescription")}
            onCheckedChange={(autoLoadMoreMessages) => onReadingChange({ ...readingPreferences, autoLoadMoreMessages })}
          />
        </SettingsGroup>
        {error ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
      </SettingsSection>
    );
  }
  if (category === "composer") {
    return (
      <SettingsSection category={category}>
        <CompositionDefinitionsSettings accounts={accounts} />
      </SettingsSection>
    );
  }
  if (category === "notifications") {
    return (
      <SettingsSection category={category}>
        <NotificationSettings accounts={accounts} />
      </SettingsSection>
    );
  }
  if (category === "advanced") {
    const error = readingError || desktopError
      ? normalizeCommandError(readingError ?? desktopError)
      : null;
    return (
      <SettingsSection category={category}>
        <SettingsGroup title={t("settings.group.tray")}>
          <Checkbox
            checked={desktopPreferences.minimizeToTray}
            label={t("settings.minimizeToTray")}
            description={t("settings.minimizeToTrayDescription")}
            onCheckedChange={(minimizeToTray) => onDesktopChange({ ...desktopPreferences, minimizeToTray })}
          />
          <Checkbox
            checked={desktopPreferences.askBeforeExit}
            label={t("settings.askBeforeExit")}
            description={t("settings.askBeforeExitDescription")}
            onCheckedChange={(askBeforeExit) => onDesktopChange({ ...desktopPreferences, askBeforeExit })}
          />
        </SettingsGroup>
        <SettingsGroup title={t("settings.group.listBehavior")}>
          <Checkbox
            checked={readingPreferences.autoLoadMoreContacts}
            label={t("settings.autoLoadMoreContacts")}
            description={t("settings.autoLoadMoreContactsDescription")}
            onCheckedChange={(autoLoadMoreContacts) => onReadingChange({ ...readingPreferences, autoLoadMoreContacts })}
          />
        </SettingsGroup>
        {error ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
      </SettingsSection>
    );
  }
  if (category === "about") {
    return (
      <SettingsSection category={category}>
        <SettingsGroup title={t("settings.group.application")}>
          <Stack gap="sm">
            <Heading level={2}>NextMail</Heading>
            <Text>{t("about.version", { version })}</Text>
            <Text>{t("about.description")}</Text>
          </Stack>
        </SettingsGroup>
        <SettingsGroup title={t("settings.group.updates")}>
          <UpdateSettings
            preferences={desktopPreferences}
            onChange={onDesktopChange}
            saveError={desktopError}
          />
        </SettingsGroup>
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
        <Heading level={1} className="text-2xl lg:text-2xl">{t(`settings.category.${category}`)}</Heading>
        <Text>{t(`settings.categoryDescription.${category}`)}</Text>
      </Stack>
      <Stack className="pt-2" gap="lg">{children}</Stack>
    </Stack>
  );
}

function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack className="rounded-lg border border-border/70 bg-muted/45 p-5 shadow-[var(--shadow-raised)]" gap="md">
      <Heading level={2} className="text-base">{title}</Heading>
      {children}
    </Stack>
  );
}

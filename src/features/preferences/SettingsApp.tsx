import {
  Bell,
  BookOpen,
  CircleUserRound,
  Info,
  Languages,
  Palette,
  PenLine,
  Plus,
  SlidersHorizontal,
} from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useAppearancePreferences, useUpdateAppearancePreferences } from "@/app/appearance";
import type { AccountDraft, AccountSummary, AppearancePreferences, LanguagePreference, ReadingPreferences, ThemePreference } from "@/app/types";
import { AccountManagementPanel } from "@/features/accounts/AccountManagementDialog";
import { PasswordAccountForm } from "@/features/accounts/PasswordAccountForm";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/dialog";
import { AppShell, Inline, Page, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { ThemeColorPicker, type ThemeColorOption } from "@/components/ui/theme-color-picker";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { CompositionDefinitionsSettings } from "./CompositionDefinitionsSettings";

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
  const [selectedAccountId, setSelectedAccountId] = useState("");
  const preferencesQuery = useAppearancePreferences();
  const readingPreferencesQuery = useQuery({ queryKey: ["reading-preferences"], queryFn: api.getReadingPreferences });
  const accountsQuery = useQuery({ queryKey: ["accounts"], queryFn: api.listAccountSummaries });
  const aboutQuery = useQuery({ queryKey: ["about"], queryFn: api.getAppAbout });
  const mutation = useUpdateAppearancePreferences();
  const readingMutation = useMutation({
    mutationFn: api.setReadingPreferences,
    onSuccess: (preferences) => queryClient.setQueryData(["reading-preferences"], preferences),
  });

  useEffect(() => {
    const accounts = accountsQuery.data ?? [];
    if (selectedAccountId && accounts.some((account) => account.id === selectedAccountId)) return;
    setSelectedAccountId(accounts[0]?.id ?? "");
  }, [accountsQuery.data, selectedAccountId]);

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

  if (preferencesQuery.isPending || readingPreferencesQuery.isPending || accountsQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (preferencesQuery.isError || readingPreferencesQuery.isError || accountsQuery.isError || !preferencesQuery.data || !readingPreferencesQuery.data) {
    const error = normalizeCommandError(preferencesQuery.error ?? readingPreferencesQuery.error ?? accountsQuery.error);
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
      <Page className="relative min-h-0 overflow-hidden bg-card">
        <OverlayScrollArea
          className="h-full"
          viewportClassName="px-10 py-8 pr-12"
          trackClassName="right-2"
        >
          <SettingsContent
            category={category}
            preferences={preferences}
            readingPreferences={readingPreferencesQuery.data}
            readingError={readingMutation.error}
            accounts={accountsQuery.data ?? []}
            selectedAccountId={selectedAccountId}
            onSelectedAccountChange={setSelectedAccountId}
            version={aboutQuery.data?.version ?? "0.1.0"}
            onChange={updatePreferences}
            onReadingChange={updateReadingPreferences}
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
  readingError,
  accounts,
  selectedAccountId,
  onSelectedAccountChange,
  version,
  onChange,
  onReadingChange,
}: {
  category: SettingsCategory;
  preferences: AppearancePreferences;
  readingPreferences: ReadingPreferences;
  readingError: unknown;
  accounts: AccountSummary[];
  selectedAccountId: string;
  onSelectedAccountChange: (accountId: string) => void;
  version: string;
  onChange: (preferences: AppearancePreferences) => void;
  onReadingChange: (preferences: ReadingPreferences) => void;
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
    const colorOptions: ThemeColorOption[] = themeColors.map((color) => ({
      value: color.value,
      label: t(`preferences.${color.name}`),
    }));
    if (!colorOptions.some((option) => option.value.toLowerCase() === preferences.accentColor.toLowerCase())) {
      colorOptions.push({ value: preferences.accentColor, label: t("preferences.customColor") });
    }
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
        <ThemeColorPicker
          label={t("preferences.themeColor")}
          value={preferences.accentColor}
          options={colorOptions}
          onValueChange={(accentColor) => onChange({ ...preferences, accentColor })}
        />
      </SettingsSection>
    );
  }
  if (category === "accounts") {
    return (
      <SettingsSection category={category}>
        <AccountsSettings
          accounts={accounts}
          selectedAccountId={selectedAccountId}
          onSelectedAccountChange={onSelectedAccountChange}
        />
      </SettingsSection>
    );
  }
  if (category === "reading") {
    const error = readingError ? normalizeCommandError(readingError) : null;
    return (
      <SettingsSection category={category}>
        <Stack className="rounded-lg bg-muted/60 p-5" gap="sm">
          <Checkbox
            checked={readingPreferences.autoLoadRemoteImages}
            label={t("settings.autoLoadRemoteImages")}
            onCheckedChange={(autoLoadRemoteImages) => onReadingChange({ ...readingPreferences, autoLoadRemoteImages })}
          />
          <Text className="pl-[28px] text-xs">{t("settings.autoLoadRemoteImagesDescription")}</Text>
        </Stack>
        <Stack className="rounded-lg bg-muted/60 p-5" gap="sm">
          <Checkbox
            checked={readingPreferences.autoOpenDownloadedAttachments}
            label={t("settings.autoOpenDownloadedAttachments")}
            onCheckedChange={(autoOpenDownloadedAttachments) => onReadingChange({ ...readingPreferences, autoOpenDownloadedAttachments })}
          />
          <Text className="pl-[28px] text-xs">{t("settings.autoOpenDownloadedAttachmentsDescription")}</Text>
        </Stack>
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

function AccountsSettings({
  accounts,
  selectedAccountId,
  onSelectedAccountChange,
}: {
  accounts: AccountSummary[];
  selectedAccountId: string;
  onSelectedAccountChange: (accountId: string) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [addOpen, setAddOpen] = useState(false);

  async function addAccount(draft: AccountDraft) {
    const account = await api.addPasswordAccount(draft);
    setAddOpen(false);
    await queryClient.invalidateQueries({ queryKey: ["accounts"] });
    await queryClient.invalidateQueries({ queryKey: ["account-runtimes"] });
    onSelectedAccountChange(account.id);
  }

  return (
    <Stack gap="lg">
      <Inline className="justify-between">
        <LabelText>{t("accounts.accountList")}</LabelText>
        <Button size="sm" onClick={() => setAddOpen(true)}><Plus size={15} />{t("accounts.add")}</Button>
      </Inline>
      {accounts.length ? (
        <Page className="grid min-h-[360px] grid-cols-[210px_minmax(0,1fr)] gap-6">
          <Stack className="self-start rounded-lg bg-muted/50 p-2" gap="xs">
            {accounts.map((account) => (
              <Button
                key={account.id}
                variant="ghost"
                className={account.id === selectedAccountId ? "h-auto justify-start bg-card px-3 py-2.5 shadow-sm hover:bg-card" : "h-auto justify-start px-3 py-2.5"}
                onClick={() => onSelectedAccountChange(account.id)}
              >
                <Stack className="min-w-0 items-start" gap="xs">
                  <Text className="max-w-full truncate text-sm font-semibold text-foreground">{account.displayName || account.email}</Text>
                  <Text className="max-w-full truncate text-xs">{account.email}</Text>
                </Stack>
              </Button>
            ))}
          </Stack>
          {selectedAccountId ? <AccountManagementPanel accountId={selectedAccountId} onRemoved={() => onSelectedAccountChange("")} /> : null}
        </Page>
      ) : (
        <EmptyState icon={<CircleUserRound size={24} />} title={t("settings.noAccount")} description={t("accounts.noAccountDescription")} action={<Button onClick={() => setAddOpen(true)}><Plus size={15} />{t("accounts.add")}</Button>} />
      )}
      <Modal open={addOpen} onOpenChange={setAddOpen} title={t("accounts.addTitle")} closeLabel={t("common.close")}>
        <Stack className="mt-5 max-h-[72vh] overflow-auto pr-1">
          <PasswordAccountForm submitLabel={t("accounts.add")} onSubmit={addAccount} />
        </Stack>
      </Modal>
    </Stack>
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

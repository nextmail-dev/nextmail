import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MoreHorizontal } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountSummary, MailboxSummary, NotificationPreferences } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { Modal } from "@/components/ui/dialog";
import { Inline, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { LabelText, Text } from "@/components/ui/typography";

export const notificationPreferencesQueryKey = ["notification-preferences"] as const;

export function NotificationSettings({ accounts }: { accounts: AccountSummary[] }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [folderAccountId, setFolderAccountId] = useState<string | null>(null);
  const preferencesQuery = useQuery({
    queryKey: notificationPreferencesQueryKey,
    queryFn: api.getNotificationPreferences,
  });
  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", folderAccountId],
    queryFn: () => api.listMailboxes(folderAccountId ?? ""),
    enabled: Boolean(folderAccountId),
  });
  const mutation = useMutation({
    mutationFn: api.setNotificationPreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(notificationPreferencesQueryKey, preferences);
    },
  });
  const preferences = preferencesQuery.data;
  const normalizedError = preferencesQuery.error || mutation.error || mailboxesQuery.error
    ? normalizeCommandError(preferencesQuery.error ?? mutation.error ?? mailboxesQuery.error)
    : null;

  function save(next: NotificationPreferences) {
    mutation.mutate(next);
  }

  if (preferencesQuery.isPending) {
    return <Stack className="items-center py-8"><Spinner size={22} /></Stack>;
  }
  if (!preferences) {
    return normalizedError ? <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert> : null;
  }

  const folderAccount = accounts.find((account) => account.id === folderAccountId);
  return (
    <Stack gap="lg">
      {normalizedError ? <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert> : null}
      <Surface className="bg-muted/60 p-5 shadow-none">
        <Inline className="justify-between">
          <Stack gap="xs">
            <LabelText>{t("notifications.enabled")}</LabelText>
            <Text className="text-xs">{t("notifications.enabledDescription")}</Text>
          </Stack>
          <Switch
            checked={preferences.enabled}
            disabled={mutation.isPending}
            label={t("notifications.enabled")}
            onCheckedChange={(enabled) => save({ ...preferences, enabled })}
          />
        </Inline>
      </Surface>

      <Surface className="grid grid-cols-2 gap-4 bg-muted/60 p-5 shadow-none">
        <SelectField
          label={t("notifications.displayMode")}
          value={preferences.displayMode}
          options={[
            { value: "stacked", label: t("notifications.stacked") },
            { value: "replace", label: t("notifications.replace") },
          ]}
          disabled={mutation.isPending}
          onValueChange={(displayMode) => save({ ...preferences, displayMode: displayMode as NotificationPreferences["displayMode"] })}
        />
        {preferences.displayMode === "stacked" ? (
          <SelectField
            label={t("notifications.maxStacked")}
            value={String(preferences.maxStacked)}
            options={Array.from({ length: 10 }, (_, index) => ({ value: String(index + 1), label: String(index + 1) }))}
            disabled={mutation.isPending}
            onValueChange={(value) => save({ ...preferences, maxStacked: Number(value) })}
          />
        ) : null}
        <SelectField
          label={t("notifications.displayDuration")}
          value={String(preferences.displayDurationSeconds)}
          options={[3, 5, 8, 10, 15, 30, 60].map((seconds) => ({
            value: String(seconds),
            label: t("notifications.seconds", { count: seconds }),
          }))}
          disabled={mutation.isPending}
          onValueChange={(value) => save({ ...preferences, displayDurationSeconds: Number(value) })}
        />
      </Surface>

      <Stack gap="sm">
        <Stack gap="xs">
          <LabelText>{t("notifications.accounts")}</LabelText>
          <Text className="text-xs">{t("notifications.accountsDescription")}</Text>
        </Stack>
        {accounts.map((account) => {
          const enabled = notificationAccountEnabled(preferences, account.id);
          return (
            <Surface key={account.id} className="bg-muted/60 p-4 shadow-none">
              <Inline className="justify-between">
                <Stack gap="xs">
                  <LabelText>{account.displayName || account.email}</LabelText>
                  {account.displayName ? <Text className="text-xs">{account.email}</Text> : null}
                </Stack>
                <Inline>
                  <Switch
                    checked={enabled}
                    disabled={mutation.isPending}
                    label={t("notifications.accountToggle", { account: account.displayName || account.email })}
                    onCheckedChange={(nextEnabled) => save(updateAccountSetting(preferences, account.id, nextEnabled))}
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={t("notifications.manageFolders", { account: account.displayName || account.email })}
                    onClick={() => setFolderAccountId(account.id)}
                  >
                    <MoreHorizontal size={18} />
                  </Button>
                </Inline>
              </Inline>
            </Surface>
          );
        })}
      </Stack>

      <Modal
        open={Boolean(folderAccountId)}
        onOpenChange={(open) => { if (!open) setFolderAccountId(null); }}
        title={t("notifications.folderTitle", { account: folderAccount?.displayName || folderAccount?.email || "" })}
        closeLabel={t("common.close")}
      >
        <Stack
          className="is-scrolling mt-5 max-h-[65vh] overflow-y-scroll pr-2 [scrollbar-gutter:stable]"
          gap="sm"
        >
          <Text>{t("notifications.folderDescription")}</Text>
          {mailboxesQuery.isPending ? <Stack className="items-center py-6"><Spinner size={22} /></Stack> : null}
          {(mailboxesQuery.data ?? []).filter((mailbox) => mailbox.selectable).map((mailbox) => (
            <Inline key={mailbox.id} className="justify-between rounded-md bg-muted/60 px-3 py-2.5">
              <Stack gap="xs">
                <LabelText>{mailbox.name}</LabelText>
                <Text className="text-xs">{t(`mailboxNames.${mailbox.role}`, { defaultValue: mailbox.role })}</Text>
              </Stack>
              <Switch
                checked={notificationFolderEnabled(preferences, folderAccountId ?? "", mailbox)}
                disabled={mutation.isPending}
                label={t("notifications.folderToggle", { folder: mailbox.name })}
                onCheckedChange={(enabled) => save(updateFolderSetting(preferences, folderAccountId ?? "", mailbox.id, enabled))}
              />
            </Inline>
          ))}
        </Stack>
      </Modal>
    </Stack>
  );
}

export function notificationAccountEnabled(preferences: NotificationPreferences, accountId: string) {
  return preferences.accounts.find((setting) => setting.accountId === accountId)?.enabled ?? true;
}

export function notificationFolderEnabled(preferences: NotificationPreferences, accountId: string, mailbox: MailboxSummary) {
  return preferences.folders.find((setting) => setting.accountId === accountId && setting.mailboxId === mailbox.id)?.enabled
    ?? mailbox.role === "inbox";
}

function updateAccountSetting(preferences: NotificationPreferences, accountId: string, enabled: boolean): NotificationPreferences {
  return {
    ...preferences,
    accounts: [...preferences.accounts.filter((setting) => setting.accountId !== accountId), { accountId, enabled }],
  };
}

function updateFolderSetting(preferences: NotificationPreferences, accountId: string, mailboxId: string, enabled: boolean): NotificationPreferences {
  return {
    ...preferences,
    folders: [
      ...preferences.folders.filter((setting) => setting.accountId !== accountId || setting.mailboxId !== mailboxId),
      { accountId, mailboxId, enabled },
    ],
  };
}

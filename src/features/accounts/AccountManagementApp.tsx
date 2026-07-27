import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";
import { AccountsManagement } from "./AccountManagement";

export function AccountManagementApp() {
  const { t } = useTranslation();
  const [selectedAccountId, setSelectedAccountId] = useState("");
  const accountsQuery = useQuery({
    queryKey: ["accounts"],
    queryFn: api.listAccountSummaries,
  });
  const lastSelectedQuery = useQuery({
    queryKey: ["last-selected-account"],
    queryFn: api.getLastSelectedAccount,
  });
  useRevealWindowWhenReady(!accountsQuery.isPending && !lastSelectedQuery.isPending);

  useEffect(() => {
    const accounts = accountsQuery.data ?? [];
    if (accounts.some((account) => account.id === selectedAccountId)) return;
    const restored = accounts.find((account) => account.id === lastSelectedQuery.data)?.id;
    setSelectedAccountId(restored ?? accounts[0]?.id ?? "");
  }, [accountsQuery.data, lastSelectedQuery.data, selectedAccountId]);

  if (accountsQuery.isPending || lastSelectedQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (accountsQuery.isError || lastSelectedQuery.isError) {
    const error = normalizeCommandError(accountsQuery.error ?? lastSelectedQuery.error);
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      </AppShell>
    );
  }

  const accounts = accountsQuery.data ?? [];
  return (
    <AppShell className="overflow-hidden bg-card">
      <Page className="flex size-full min-h-0 flex-col px-7 pt-6 pb-7">
        <Stack className="shrink-0" gap="xs">
          <Heading level={1} className="text-2xl">{t("accounts.title")}</Heading>
          <Text>{t("accounts.description")}</Text>
        </Stack>
        <AccountsManagement
          accounts={accounts}
          selectedAccountId={selectedAccountId}
          onSelectedAccountChange={setSelectedAccountId}
        />
      </Page>
    </AppShell>
  );
}

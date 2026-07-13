import { Server, ShieldCheck } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { MailboxRole, SyncPolicy } from "@/app/types";
import { Surface } from "@/components/ui/card";
import { Alert } from "@/components/ui/alert";
import { Modal } from "@/components/ui/dialog";
import { Inline, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { LabelText, Text } from "@/components/ui/typography";

interface AccountManagementDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  accountId: string;
}

export function AccountManagementDialog({
  open,
  onOpenChange,
  accountId,
}: AccountManagementDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const detailQuery = useQuery({
    queryKey: ["account-management", accountId],
    queryFn: () => api.getAccountManagementDetail(accountId),
    enabled: open && Boolean(accountId),
  });
  const progressQuery = useQuery({
    queryKey: ["sync-progress", accountId],
    queryFn: () => api.getSyncProgress(accountId),
    enabled: open && Boolean(accountId),
  });
  const mailboxesQuery = useQuery({
    queryKey: ["mailboxes", accountId],
    queryFn: () => api.listMailboxes(accountId),
    enabled: open && Boolean(accountId),
  });
  const policyMutation = useMutation({
    mutationFn: (syncPolicy: SyncPolicy) => api.setAccountSyncPolicy(accountId, syncPolicy),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["account-management", accountId] }),
  });
  const roleMutation = useMutation({
    mutationFn: ({ role, mailboxId }: { role: MailboxRole; mailboxId: string | null }) =>
      api.setMailboxRoleMapping(accountId, role, mailboxId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["mailboxes", accountId] }),
  });
  const account = detailQuery.data;
  const operationError = detailQuery.error ?? mailboxesQuery.error ?? policyMutation.error ?? roleMutation.error;
  const normalizedError = operationError ? normalizeCommandError(operationError) : null;

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={t("accounts.title")}
      closeLabel={t("common.close")}
    >
      <Stack className="mt-5" gap="lg">
        <Text>{t("accounts.description")}</Text>
        {normalizedError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
        {detailQuery.isPending ? (
          <Stack className="items-center py-8"><Spinner size={22} /></Stack>
        ) : account ? (
          <>
            <Surface className="rounded-sm p-4">
              <Stack gap="sm">
                <Inline className="text-primary">
                  <ShieldCheck size={17} />
                  <LabelText>{account.displayName || account.email}</LabelText>
                </Inline>
                {account.displayName ? <Text>{account.email}</Text> : null}
                <Inline className="mt-2 text-muted-foreground">
                  <Server size={15} />
                  <Text className="text-xs">
                    {account.incomingHost}:{account.incomingPort} · {t(`accounts.security.${account.security}`)}
                  </Text>
                </Inline>
                <Text className="text-xs">
                  {t("accounts.syncState", {
                    state: t(`sync.${progressQuery.data?.phase ?? "idle"}`),
                  })}
                </Text>
              </Stack>
            </Surface>
            <SelectField
              label={t("accounts.syncPolicy")}
              value={account.syncPolicy}
              options={[
                { value: "days30", label: t("accounts.days30") },
                { value: "days90", label: t("accounts.days90") },
                { value: "days365", label: t("accounts.days365") },
                { value: "all", label: t("accounts.all") },
              ]}
              onValueChange={(value) => policyMutation.mutate(value as SyncPolicy)}
              disabled={policyMutation.isPending}
            />
            <Stack gap="sm">
              <LabelText>{t("accounts.folderMappings")}</LabelText>
              {(["sent", "drafts", "trash", "archive"] as MailboxRole[]).map((role) => (
                <SelectField
                  key={role}
                  label={t(`accounts.folderRole.${role}`)}
                  value={mailboxesQuery.data?.find((mailbox) => mailbox.role === role)?.id ?? "__none__"}
                  options={[
                    { value: "__none__", label: t("accounts.folderUnassigned") },
                    ...(mailboxesQuery.data ?? [])
                      .filter((mailbox) => mailbox.selectable)
                      .map((mailbox) => ({ value: mailbox.id, label: mailbox.name })),
                  ]}
                  onValueChange={(value) => roleMutation.mutate({
                    role,
                    mailboxId: value === "__none__" ? null : value,
                  })}
                  disabled={roleMutation.isPending}
                />
              ))}
              <Text className="text-xs">{t("accounts.folderMappingsDescription")}</Text>
            </Stack>
          </>
        ) : null}
      </Stack>
    </Modal>
  );
}

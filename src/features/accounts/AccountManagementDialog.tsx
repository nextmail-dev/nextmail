import { CircleUserRound, KeyRound, Pencil, Plus, Server, ShieldCheck, Trash2 } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AccountDraft, AccountSummary, MailboxRole, SyncPolicy } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Modal } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { PasswordField } from "@/components/ui/input";
import { Inline, Page, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { LabelText, Text } from "@/components/ui/typography";
import { PasswordAccountForm } from "./PasswordAccountForm";

export function AccountManagementDialog({
  open,
  onOpenChange,
  accounts,
  selectedAccountId,
  onSelectedAccountChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  accounts: AccountSummary[];
  selectedAccountId: string;
  onSelectedAccountChange: (accountId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={t("accounts.title")}
      closeLabel={t("common.close")}
      contentClassName="flex max-h-[calc(100vh-40px)] w-[min(980px,calc(100vw-40px))] flex-col"
    >
      <AccountsManagement
        accounts={accounts}
        selectedAccountId={selectedAccountId}
        onSelectedAccountChange={onSelectedAccountChange}
        enabled={open}
      />
    </Modal>
  );
}

export function AccountsManagement({
  accounts,
  selectedAccountId,
  onSelectedAccountChange,
  enabled = true,
}: {
  accounts: AccountSummary[];
  selectedAccountId: string;
  onSelectedAccountChange: (accountId: string) => void;
  enabled?: boolean;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [addOpen, setAddOpen] = useState(false);
  const activeAccountId = accounts.some((account) => account.id === selectedAccountId)
    ? selectedAccountId
    : accounts[0]?.id ?? "";

  async function addAccount(draft: AccountDraft) {
    const account = await api.addPasswordAccount(draft);
    setAddOpen(false);
    await queryClient.invalidateQueries({ queryKey: ["accounts"] });
    await queryClient.invalidateQueries({ queryKey: ["account-runtimes"] });
    onSelectedAccountChange(account.id);
  }

  function selectAfterRemoval() {
    const nextAccountId = nextAccountIdAfterRemoval(accounts, activeAccountId);
    if (nextAccountId) onSelectedAccountChange(nextAccountId);
  }

  return (
    <Stack className="mt-5 min-h-0 flex-1" gap="lg">
      <Inline className="justify-between">
        <LabelText>{t("accounts.accountList")}</LabelText>
        {accounts.length ? <Button size="sm" onClick={() => setAddOpen(true)}><Plus size={15} />{t("accounts.add")}</Button> : null}
      </Inline>
      {accounts.length ? (
        <Page className="grid min-h-0 flex-1 grid-cols-[210px_minmax(0,1fr)] gap-6 overflow-hidden">
          <Stack className="self-start rounded-lg bg-muted/50 p-2" gap="xs">
            {accounts.map((account) => (
              <Button
                key={account.id}
                variant="ghost"
                className={account.id === activeAccountId ? "h-auto justify-start bg-card px-3 py-2.5 shadow-sm hover:bg-card" : "h-auto justify-start px-3 py-2.5"}
                onClick={() => onSelectedAccountChange(account.id)}
              >
                <Stack className="min-w-0 items-start" gap="xs">
                  <Text className="max-w-full truncate text-sm font-semibold text-foreground">{account.displayName || account.email}</Text>
                  <Text className="max-w-full truncate text-xs">{account.email}</Text>
                </Stack>
              </Button>
            ))}
          </Stack>
          {activeAccountId ? (
            <Stack className="min-h-0 overflow-y-scroll pr-2">
              <AccountManagementPanel
                accountId={activeAccountId}
                enabled={enabled}
                onRemoved={selectAfterRemoval}
              />
            </Stack>
          ) : null}
        </Page>
      ) : (
        <EmptyState
          icon={<CircleUserRound size={24} />}
          title={t("accounts.noAccount")}
          description={t("accounts.noAccountDescription")}
          action={<Button onClick={() => setAddOpen(true)}><Plus size={15} />{t("accounts.add")}</Button>}
        />
      )}
      <Modal open={addOpen} onOpenChange={setAddOpen} title={t("accounts.addTitle")} closeLabel={t("common.close")}>
        <Stack className="mt-5 max-h-[72vh] overflow-auto pr-1">
          <PasswordAccountForm submitLabel={t("accounts.add")} onSubmit={addAccount} />
        </Stack>
      </Modal>
    </Stack>
  );
}

export function nextAccountIdAfterRemoval(accounts: AccountSummary[], removedAccountId: string) {
  return accounts.find((account) => account.id !== removedAccountId)?.id ?? "";
}

export function AccountManagementPanel({
  accountId,
  enabled = true,
  className,
  onRemoved,
}: {
  accountId: string;
  enabled?: boolean;
  className?: string;
  onRemoved?: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [editOpen, setEditOpen] = useState(false);
  const [reauthOpen, setReauthOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [password, setPassword] = useState("");
  const detailQuery = useQuery({ queryKey: ["account-management", accountId], queryFn: () => api.getAccountManagementDetail(accountId), enabled: enabled && Boolean(accountId) });
  const connectionQuery = useQuery({ queryKey: ["account-connection", accountId], queryFn: () => api.getAccountConnectionDraft(accountId), enabled: enabled && editOpen && Boolean(accountId) });
  const progressQuery = useQuery({ queryKey: ["sync-progress", accountId], queryFn: () => api.getSyncProgress(accountId), enabled: enabled && Boolean(accountId) });
  const runtimeQuery = useQuery({ queryKey: ["account-runtimes"], queryFn: api.listAccountRuntimeSummaries, enabled });
  const mailboxesQuery = useQuery({ queryKey: ["mailboxes", accountId], queryFn: () => api.listMailboxes(accountId), enabled: enabled && Boolean(accountId) });
  const impactQuery = useQuery({ queryKey: ["account-removal-impact", accountId], queryFn: () => api.getAccountRemovalImpact(accountId), enabled: enabled && removeOpen && Boolean(accountId) });
  const policyMutation = useMutation({
    mutationFn: (syncPolicy: SyncPolicy) => api.setAccountSyncPolicy(accountId, syncPolicy),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["account-management", accountId] }),
  });
  const nonInboxBodyMutation = useMutation({
    mutationFn: (enabled: boolean) => api.setDownloadNonInboxBodies(accountId, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["account-management", accountId] }),
  });
  const roleMutation = useMutation({
    mutationFn: ({ role, mailboxId }: { role: MailboxRole; mailboxId: string | null }) => api.setMailboxRoleMapping(accountId, role, mailboxId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["mailboxes", accountId] }),
  });
  const reauthMutation = useMutation({
    mutationFn: () => api.reauthenticatePasswordAccount(accountId, password),
    onSuccess: async () => {
      setPassword("");
      setReauthOpen(false);
      await queryClient.invalidateQueries({ queryKey: ["account-runtimes"] });
    },
  });
  const removeMutation = useMutation({
    mutationFn: () => api.removeAccount(accountId),
    onSuccess: async () => {
      setRemoveOpen(false);
      await queryClient.invalidateQueries({ queryKey: ["accounts"] });
      await queryClient.invalidateQueries({ queryKey: ["bootstrap"] });
      onRemoved?.();
    },
  });
  const account = detailQuery.data;
  const runtime = runtimeQuery.data?.find((item) => item.accountId === accountId);
  const operationError = detailQuery.error ?? mailboxesQuery.error ?? policyMutation.error ?? nonInboxBodyMutation.error ?? roleMutation.error ?? reauthMutation.error ?? removeMutation.error;
  const normalizedError = operationError ? normalizeCommandError(operationError) : null;

  async function updateAccount(draft: AccountDraft) {
    await api.updatePasswordAccount(
      accountId,
      {
        email: draft.email,
        displayName: draft.displayName,
        incoming: draft.incoming,
        outgoing: draft.outgoing,
        insecureAcknowledged: draft.insecureAcknowledged,
      },
      draft.password || null,
    );
    setEditOpen(false);
    await queryClient.invalidateQueries({ queryKey: ["accounts"] });
    await queryClient.invalidateQueries({ queryKey: ["account-management", accountId] });
    await queryClient.invalidateQueries({ queryKey: ["account-runtimes"] });
  }

  return (
    <Stack className={className} gap="lg">
      {normalizedError ? <Alert tone="danger" title={t("errors.title")}>{t(`errors.${normalizedError.code}`, { defaultValue: t("common.unexpectedError") })}</Alert> : null}
      {detailQuery.isPending ? (
        <Stack className="items-center py-8"><Spinner size={22} /></Stack>
      ) : account ? (
        <>
          <Surface className="rounded-lg bg-muted/60 p-4 shadow-none">
            <Stack gap="sm">
              <Inline className="text-primary"><ShieldCheck size={17} /><LabelText>{account.displayName || account.email}</LabelText></Inline>
              {account.displayName ? <Text>{account.email}</Text> : null}
              <Inline className="mt-2 text-muted-foreground"><Server size={15} /><Text className="text-xs">{account.incomingHost}:{account.incomingPort} · {t(`accounts.security.${account.security}`)}</Text></Inline>
              <Text className="text-xs">{t("accounts.runtimeState", { state: t(`accounts.runtime.${runtime?.state ?? "stopped"}`) })}</Text>
              <Text className="text-xs">{t("accounts.syncState", { state: t(`sync.${progressQuery.data?.phase ?? "idle"}`) })}</Text>
            </Stack>
          </Surface>
          <Inline className="flex-wrap">
            <Button variant="secondary" onClick={() => setEditOpen(true)}><Pencil size={15} />{t("accounts.editConnection")}</Button>
            <Button variant="secondary" onClick={() => setReauthOpen(true)}><KeyRound size={15} />{t("accounts.reauthenticate")}</Button>
            <Button variant="ghost" className="text-destructive hover:text-destructive" onClick={() => setRemoveOpen(true)}><Trash2 size={15} />{t("accounts.remove")}</Button>
          </Inline>
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
          <Stack gap="xs">
            <Checkbox
              checked={account.downloadNonInboxBodies}
              onCheckedChange={(enabled) => nonInboxBodyMutation.mutate(enabled)}
              label={t("accounts.downloadNonInboxBodies")}
            />
            <Text className="pl-7 text-xs">{t("accounts.downloadNonInboxBodiesDescription")}</Text>
          </Stack>
          <Stack gap="sm">
            <LabelText>{t("accounts.folderMappings")}</LabelText>
            {(["sent", "drafts", "trash", "archive"] as MailboxRole[]).map((role) => (
              <SelectField
                key={role}
                label={t(`accounts.folderRole.${role}`)}
                value={mailboxesQuery.data?.find((mailbox) => mailbox.role === role)?.id ?? "__none__"}
                options={[
                  { value: "__none__", label: t("accounts.folderUnassigned") },
                  ...(mailboxesQuery.data ?? []).filter((mailbox) => mailbox.selectable).map((mailbox) => ({ value: mailbox.id, label: mailbox.name })),
                ]}
                onValueChange={(value) => roleMutation.mutate({ role, mailboxId: value === "__none__" ? null : value })}
                disabled={roleMutation.isPending}
              />
            ))}
            <Text className="text-xs">{t("accounts.folderMappingsDescription")}</Text>
          </Stack>
        </>
      ) : null}

      <Modal open={editOpen} onOpenChange={setEditOpen} title={t("accounts.editConnection")} closeLabel={t("common.close")}>
        <Stack className="mt-5 max-h-[70vh] overflow-auto pr-1">
          {connectionQuery.isPending ? <Spinner size={22} /> : connectionQuery.data ? (
            <PasswordAccountForm key={accountId} initial={connectionQuery.data} passwordRequired={false} submitLabel={t("common.save")} onSubmit={updateAccount} />
          ) : null}
        </Stack>
      </Modal>

      <Modal open={reauthOpen} onOpenChange={setReauthOpen} title={t("accounts.reauthenticate")} closeLabel={t("common.close")}>
        <Stack className="mt-5" gap="lg">
          <Text>{t("accounts.reauthenticateDescription")}</Text>
          <PasswordField required label={t("onboarding.password")} showPasswordLabel={t("onboarding.showPassword")} hidePasswordLabel={t("onboarding.hidePassword")} value={password} onChange={(event) => setPassword(event.currentTarget.value)} />
          <Inline className="justify-end"><Button loading={reauthMutation.isPending} disabled={!password} onClick={() => reauthMutation.mutate()}>{t("accounts.reauthenticate")}</Button></Inline>
        </Stack>
      </Modal>

      <Modal open={removeOpen} onOpenChange={setRemoveOpen} title={t("accounts.removeTitle")} closeLabel={t("common.close")}>
        <Stack className="mt-5" gap="lg">
          {impactQuery.isPending ? <Spinner size={22} /> : impactQuery.data ? (
            <>
              <Alert tone={impactQuery.data.canRemove ? "warning" : "danger"} title={impactQuery.data.canRemove ? t("accounts.removeWarning") : t("accounts.removeBlocked")}>
                {impactQuery.data.canRemove
                  ? t("accounts.removeDescription", { drafts: impactQuery.data.editingDrafts })
                  : t("accounts.removeBlockedDescription", { jobs: impactQuery.data.queuedSendJobs, operations: impactQuery.data.pendingOperations })}
              </Alert>
              <Inline className="justify-end">
                <Button variant="secondary" onClick={() => setRemoveOpen(false)}>{t("common.cancel")}</Button>
                <Button loading={removeMutation.isPending} disabled={!impactQuery.data.canRemove} className="bg-destructive text-destructive-foreground hover:bg-destructive/90" onClick={() => removeMutation.mutate()}>{t("accounts.confirmRemove")}</Button>
              </Inline>
            </>
          ) : null}
        </Stack>
      </Modal>
    </Stack>
  );
}

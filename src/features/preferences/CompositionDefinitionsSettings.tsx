import { listen } from "@tauri-apps/api/event";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Pencil, Plus, Signature, Star, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type {
  AccountSummary,
  CompositionScene,
  CompositionSceneRule,
  MailSignature,
  MailTemplate,
  SignaturePreferences,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Surface } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { Inline, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Heading, LabelText, Text } from "@/components/ui/typography";

type PendingDelete = { kind: "template" | "signature"; id: string } | null;

interface CompositionDefinitionsSettingsProps {
  accounts: AccountSummary[];
}

export function CompositionDefinitionsSettings({ accounts }: CompositionDefinitionsSettingsProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [scope, setScope] = useState("global");
  const [pendingDelete, setPendingDelete] = useState<PendingDelete>(null);
  const [openError, setOpenError] = useState<unknown>(null);
  const accountId = scope === "global" ? null : scope;
  const scopeKey = accountId ?? "global";
  const templatesKey = ["mail-templates", scopeKey] as const;
  const signaturesKey = ["mail-signatures", scopeKey] as const;
  const rulesKey = ["composition-scene-rules", scopeKey] as const;
  const signaturePreferencesKey = ["signature-preferences", scopeKey] as const;
  const templates = useQuery({
    queryKey: templatesKey,
    queryFn: () => api.listMailTemplates(accountId),
  });
  const signatures = useQuery({
    queryKey: signaturesKey,
    queryFn: () => api.listMailSignatures(accountId),
  });
  const globalTemplates = useQuery({
    queryKey: ["mail-templates", "global"],
    queryFn: () => api.listMailTemplates(null),
    enabled: accountId !== null,
  });
  const globalSignatures = useQuery({
    queryKey: ["mail-signatures", "global"],
    queryFn: () => api.listMailSignatures(null),
    enabled: accountId !== null,
  });
  const rules = useQuery({
    queryKey: rulesKey,
    queryFn: () => api.listCompositionSceneRules(accountId),
  });
  const signaturePreferences = useQuery({
    queryKey: signaturePreferencesKey,
    queryFn: () => api.getSignaturePreferences(accountId),
  });
  const ruleUpdate = useMutation({
    mutationFn: ({ rule, templateId, inherit }: {
      rule: CompositionSceneRule;
      templateId: string | null;
      inherit: boolean;
    }) => api.saveCompositionSceneRule(accountId, {
      scene: rule.scene,
      templateId,
      signatureId: null,
      inherit,
    }, rule.revision),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: rulesKey });
    },
  });
  const signaturePreferencesUpdate = useMutation({
    mutationFn: ({ preferences, defaultSignatureId, autoInsert }: {
      preferences: SignaturePreferences;
      defaultSignatureId: string | null;
      autoInsert: boolean;
    }) => api.saveSignaturePreferences(accountId, {
      defaultSignatureId,
      autoInsert,
      inherit: false,
    }, preferences.revision),
    onSuccess: (preferences) => {
      queryClient.setQueryData(signaturePreferencesKey, preferences);
    },
  });
  const deletion = useMutation({
    mutationFn: async (target: { kind: "template" | "signature"; id: string; revision: number }) => {
      if (target.kind === "template") {
        await api.deleteMailTemplate(accountId, target.id, target.revision);
      } else {
        await api.deleteMailSignature(accountId, target.id, target.revision);
      }
      return target.kind;
    },
    onSuccess: async (kind) => {
      setPendingDelete(null);
      await queryClient.invalidateQueries({ queryKey: kind === "template" ? templatesKey : signaturesKey });
      if (kind === "signature") {
        await queryClient.invalidateQueries({ queryKey: signaturePreferencesKey });
      }
    },
  });

  useEffect(() => {
    if (scope === "global" || accounts.some((account) => account.id === scope)) return;
    setScope("global");
  }, [accounts, scope]);

  useEffect(() => {
    setPendingDelete(null);
    setOpenError(null);
  }, [scope]);

  useEffect(() => {
    const unlisten = listen<{ accountId: string | null; kind: "template" | "signature" }>(
      "composition-definitions-changed",
      (event) => {
        const changedScope = event.payload.accountId ?? "global";
        void queryClient.invalidateQueries({
          queryKey: [event.payload.kind === "template" ? "mail-templates" : "mail-signatures", changedScope],
        });
        if (event.payload.kind === "signature") {
          void queryClient.invalidateQueries({ queryKey: ["signature-preferences", changedScope] });
        }
      },
    );
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [queryClient]);

  const error = templates.error ?? signatures.error ?? globalTemplates.error
    ?? globalSignatures.error ?? rules.error ?? signaturePreferences.error
    ?? ruleUpdate.error ?? signaturePreferencesUpdate.error ?? deletion.error ?? openError;
  const availableTemplates = accountId
    ? [...(globalTemplates.data ?? []), ...(templates.data ?? [])]
    : templates.data ?? [];
  const availableSignatures = accountId
    ? [...(globalSignatures.data ?? []), ...(signatures.data ?? [])]
    : signatures.data ?? [];

  async function openDefinitionEditor(
    kind: "template" | "signature",
    definitionId: string | null,
  ) {
    setOpenError(null);
    try {
      await api.openCompositionDefinitionEditor(accountId, kind, definitionId);
    } catch (reason) {
      setOpenError(reason);
    }
  }

  function requestDelete(kind: "template" | "signature", id: string, revision: number) {
    if (pendingDelete?.kind !== kind || pendingDelete.id !== id) {
      setPendingDelete({ kind, id });
      return;
    }
    deletion.mutate({ kind, id, revision });
  }

  return (
    <Stack gap="lg">
      <SelectField
        label={t("compositionLibrary.scope")}
        value={scope}
        options={[
          { value: "global", label: t("compositionLibrary.globalScope") },
          ...accounts.map((account) => ({
            value: account.id,
            label: t("compositionLibrary.accountScope", { account: account.displayName || account.email }),
          })),
        ]}
        onValueChange={setScope}
      />
      <Text>{accountId ? t("compositionLibrary.accountScopeDescription") : t("compositionLibrary.globalScopeDescription")}</Text>
      {error ? (
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${normalizeCommandError(error).code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      ) : null}
      <SceneRules
        accountScope={accountId !== null}
        rules={rules.data ?? []}
        templates={availableTemplates}
        loading={rules.isPending || ruleUpdate.isPending}
        onChange={(rule, templateId, inherit) => {
          ruleUpdate.mutate({ rule, templateId, inherit });
        }}
      />
      <DefinitionList
        kind="template"
        values={templates.data ?? []}
        loading={templates.isPending}
        pendingDelete={pendingDelete}
        deleting={deletion.isPending}
        onAdd={() => void openDefinitionEditor("template", null)}
        onEdit={(value) => void openDefinitionEditor("template", value.id)}
        onDelete={(value) => requestDelete("template", value.id, value.revision)}
      />
      <SignaturePreferencesPanel
        preferences={signaturePreferences.data}
        signatures={availableSignatures}
        accountScope={accountId !== null}
        loading={signaturePreferences.isPending || signaturePreferencesUpdate.isPending}
        onAutoInsertChange={(autoInsert) => {
          if (!signaturePreferences.data) return;
          signaturePreferencesUpdate.mutate({
            preferences: signaturePreferences.data,
            defaultSignatureId: signaturePreferences.data.defaultSignatureId,
            autoInsert,
          });
        }}
      />
      <DefinitionList
        kind="signature"
        values={availableSignatures}
        loading={signatures.isPending || globalSignatures.isPending}
        pendingDelete={pendingDelete}
        deleting={deletion.isPending}
        onAdd={() => void openDefinitionEditor("signature", null)}
        onEdit={(value) => void openDefinitionEditor("signature", value.id)}
        onDelete={(value) => requestDelete("signature", value.id, value.revision)}
        canManage={(value) => accountId === null || value.scope === "account"}
        showScope={accountId !== null}
        defaultValueId={signaturePreferences.data?.defaultSignatureId ?? null}
        settingDefault={signaturePreferencesUpdate.isPending}
        onSetDefault={(value) => {
          if (!signaturePreferences.data) return;
          signaturePreferencesUpdate.mutate({
            preferences: signaturePreferences.data,
            defaultSignatureId: value.id,
            autoInsert: signaturePreferences.data.autoInsert,
          });
        }}
      />
    </Stack>
  );
}

const SCENES: CompositionScene[] = ["new", "reply", "reply_all", "forward"];

function SceneRules({
  accountScope,
  rules,
  templates,
  loading,
  onChange,
}: {
  accountScope: boolean;
  rules: CompositionSceneRule[];
  templates: MailTemplate[];
  loading: boolean;
  onChange: (
    rule: CompositionSceneRule,
    templateId: string | null,
    inherit: boolean,
  ) => void;
}) {
  const { t } = useTranslation();
  return (
    <Stack gap="sm">
      <Stack gap="xs">
        <Heading level={2}>{t("compositionLibrary.defaultRules")}</Heading>
        <Text className="text-xs">
          {accountScope
            ? t("compositionLibrary.accountRulesDescription")
            : t("compositionLibrary.globalRulesDescription")}
        </Text>
      </Stack>
      {SCENES.map((scene) => {
        const rule = rules.find((value) => value.scene === scene) ?? {
          scene,
          templateId: null,
          signatureId: null,
          inherited: accountScope,
          revision: 0,
        };
        return (
          <Surface key={scene} className="p-4 shadow-none ring-1 ring-border/70">
            <Stack gap="sm">
              <LabelText>{t(`compositionLibrary.scene.${scene}`)}</LabelText>
              <Inline className="flex-wrap items-end">
                {accountScope ? (
                  <SelectField
                    label={t("compositionLibrary.ruleMode")}
                    value={rule.inherited ? "inherit" : "custom"}
                    options={[
                      { value: "inherit", label: t("compositionLibrary.inheritGlobal") },
                      { value: "custom", label: t("compositionLibrary.customRule") },
                    ]}
                    disabled={loading}
                    onValueChange={(value) => onChange(
                      rule,
                      rule.templateId,
                      value === "inherit",
                    )}
                  />
                ) : null}
                <SelectField
                  label={t("composer.template")}
                  value={rule.templateId ?? "none"}
                  options={[
                    { value: "none", label: t("composer.noTemplate") },
                    ...templates.map((value) => ({
                      value: value.id,
                      label: scopedDefinitionLabel(value.name, value.scope, t),
                    })),
                  ]}
                  disabled={loading || rule.inherited}
                  onValueChange={(value) => onChange(
                    rule,
                    value === "none" ? null : value,
                    false,
                  )}
                />
              </Inline>
            </Stack>
          </Surface>
        );
      })}
    </Stack>
  );
}

function SignaturePreferencesPanel({
  preferences,
  signatures,
  accountScope,
  loading,
  onAutoInsertChange,
}: {
  preferences?: SignaturePreferences;
  signatures: MailSignature[];
  accountScope: boolean;
  loading: boolean;
  onAutoInsertChange: (enabled: boolean) => void;
}) {
  const { t } = useTranslation();
  const defaultSignature = signatures.find((value) => value.id === preferences?.defaultSignatureId);
  return (
    <Surface className="p-4 shadow-none ring-1 ring-border/70">
      <Stack gap="sm">
        <Checkbox
          checked={preferences?.autoInsert ?? true}
          disabled={!preferences || loading}
          label={t("compositionLibrary.autoSelectDefaultSignature")}
          onCheckedChange={onAutoInsertChange}
        />
        <Text className="pl-7 text-xs">
          {defaultSignature
            ? t("compositionLibrary.currentDefaultSignature", { name: defaultSignature.name })
            : t("compositionLibrary.noDefaultSignature")}
          {accountScope && preferences?.inherited
            ? ` ${t("compositionLibrary.inheritedDefaultSignature")}`
            : ""}
        </Text>
      </Stack>
    </Surface>
  );
}

function scopedDefinitionLabel(
  name: string,
  scope: "global" | "account",
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  return t("composer.definitionOption", {
    name,
    scope: t(`compositionLibrary.${scope}Badge`),
  });
}

function DefinitionList<T extends MailTemplate | MailSignature>({
  kind,
  values,
  loading,
  pendingDelete,
  deleting,
  onAdd,
  onEdit,
  onDelete,
  defaultValueId,
  settingDefault = false,
  onSetDefault,
  canManage,
  showScope = false,
}: {
  kind: "template" | "signature";
  values: T[];
  loading: boolean;
  pendingDelete: PendingDelete;
  deleting: boolean;
  onAdd: () => void;
  onEdit: (value: T) => void;
  onDelete: (value: T) => void;
  defaultValueId?: string | null;
  settingDefault?: boolean;
  onSetDefault?: (value: T) => void;
  canManage?: (value: T) => boolean;
  showScope?: boolean;
}) {
  const { t } = useTranslation();
  const Icon = kind === "template" ? FileText : Signature;
  return (
    <Stack gap="sm">
      <Inline className="justify-between">
        <Stack gap="xs">
          <Heading level={2}>{t(`compositionLibrary.${kind}s`)}</Heading>
          <Text className="text-xs">{t(`compositionLibrary.${kind}sDescription`)}</Text>
        </Stack>
        <Button size="sm" onClick={onAdd}>
          <Plus size={15} />
          {t(`compositionLibrary.add${kind === "template" ? "Template" : "Signature"}`)}
        </Button>
      </Inline>
      {loading ? <Text>{t("common.loading")}</Text> : null}
      {!loading && !values.length ? (
        <EmptyState
          className="items-center rounded-lg bg-muted/45 p-6 text-center"
          icon={<Icon size={23} />}
          title={t(`compositionLibrary.no${kind === "template" ? "Templates" : "Signatures"}`)}
          description={t(`compositionLibrary.no${kind === "template" ? "Templates" : "Signatures"}Description`)}
        />
      ) : null}
      <Stack
        className={kind === "signature" && values.length
          ? "overflow-hidden rounded-lg bg-card ring-1 ring-border/70"
          : undefined}
        gap={kind === "signature" ? "none" : "sm"}
      >
        {values.map((value) => {
          const confirming = pendingDelete?.kind === kind && pendingDelete.id === value.id;
          const isDefault = kind === "signature" && value.id === defaultValueId;
          const manageable = canManage?.(value) ?? true;
          const subject = "subject" in value ? value.subject : "";
          return (
            <Surface
              key={value.id}
              className={kind === "signature"
                ? "rounded-none border-b border-border/70 p-3 shadow-none last:border-b-0"
                : "p-4 shadow-none ring-1 ring-border/70"}
            >
            <Inline className="items-start justify-between">
              <Stack className="min-w-0 flex-1" gap="xs">
                <Inline className="gap-2">
                  <LabelText className="truncate">{value.name}</LabelText>
                  {isDefault ? (
                    <Text className="shrink-0 rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-semibold text-primary">
                      {t("compositionLibrary.defaultSignature")}
                    </Text>
                  ) : null}
                  {showScope ? (
                    <Text className="shrink-0 text-[11px]">
                      {t(`compositionLibrary.${value.scope}Badge`)}
                    </Text>
                  ) : null}
                </Inline>
                {subject ? <Text className="truncate text-xs text-foreground">{subject}</Text> : null}
                <Text className="line-clamp-2 whitespace-pre-line text-xs">
                  {value.content.plainText || t("compositionLibrary.emptyContent")}
                </Text>
              </Stack>
              <Inline className="shrink-0 gap-1">
                {kind === "signature" && !isDefault && onSetDefault ? (
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={settingDefault}
                    onClick={() => onSetDefault(value)}
                  >
                    <Star size={14} />
                    {t("compositionLibrary.setAsDefault")}
                  </Button>
                ) : null}
                {manageable ? (
                  <>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label={t("common.edit")}
                      title={t("common.edit")}
                      onClick={() => onEdit(value)}
                    >
                      <Pencil size={15} />
                    </Button>
                    <Button
                      type="button"
                      variant={confirming ? "danger" : "ghost"}
                      size="icon"
                      loading={deleting && confirming}
                      aria-label={confirming ? t("compositionLibrary.confirmDelete") : t("common.delete")}
                      title={confirming ? t("compositionLibrary.confirmDelete") : t("common.delete")}
                      onClick={() => onDelete(value)}
                    >
                      <Trash2 size={15} />
                    </Button>
                  </>
                ) : null}
              </Inline>
            </Inline>
            </Surface>
          );
        })}
      </Stack>
    </Stack>
  );
}

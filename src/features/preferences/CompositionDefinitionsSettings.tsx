import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Pencil, Plus, Signature, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type {
  AccountSummary,
  DraftContent,
  MailSignature,
  MailSignatureDraft,
  MailTemplate,
  MailTemplateDraft,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { Modal } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { Form, Inline, Page, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { TextField } from "@/components/ui/input";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { RichTextEditor } from "@/features/composer/RichTextEditor";

const EMPTY_CONTENT: DraftContent = {
  editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
  html: "<p></p>",
  plainText: "",
};

type DefinitionEditorState =
  | { kind: "template"; value: MailTemplate | null }
  | { kind: "signature"; value: MailSignature | null }
  | null;

type PendingDelete = { kind: "template" | "signature"; id: string } | null;

interface CompositionDefinitionsSettingsProps {
  accounts: AccountSummary[];
}

export function CompositionDefinitionsSettings({ accounts }: CompositionDefinitionsSettingsProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [scope, setScope] = useState("global");
  const [editor, setEditor] = useState<DefinitionEditorState>(null);
  const [pendingDelete, setPendingDelete] = useState<PendingDelete>(null);
  const accountId = scope === "global" ? null : scope;
  const scopeKey = accountId ?? "global";
  const templatesKey = ["mail-templates", scopeKey] as const;
  const signaturesKey = ["mail-signatures", scopeKey] as const;
  const templates = useQuery({
    queryKey: templatesKey,
    queryFn: () => api.listMailTemplates(accountId),
  });
  const signatures = useQuery({
    queryKey: signaturesKey,
    queryFn: () => api.listMailSignatures(accountId),
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
    },
  });

  useEffect(() => {
    if (scope === "global" || accounts.some((account) => account.id === scope)) return;
    setScope("global");
  }, [accounts, scope]);

  useEffect(() => {
    setPendingDelete(null);
    setEditor(null);
  }, [scope]);

  const error = templates.error ?? signatures.error ?? deletion.error;

  async function saveDefinition(
    state: Exclude<DefinitionEditorState, null>,
    name: string,
    subject: string,
    content: DraftContent,
  ) {
    if (state.kind === "template") {
      const draft: MailTemplateDraft = { name, subject, content };
      if (state.value) {
        await api.updateMailTemplate(accountId, state.value.id, draft, state.value.revision);
      } else {
        await api.createMailTemplate(accountId, draft);
      }
      await queryClient.invalidateQueries({ queryKey: templatesKey });
    } else {
      const draft: MailSignatureDraft = { name, content };
      if (state.value) {
        await api.updateMailSignature(accountId, state.value.id, draft, state.value.revision);
      } else {
        await api.createMailSignature(accountId, draft);
      }
      await queryClient.invalidateQueries({ queryKey: signaturesKey });
    }
    setEditor(null);
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
      <DefinitionList
        kind="template"
        values={templates.data ?? []}
        loading={templates.isPending}
        pendingDelete={pendingDelete}
        deleting={deletion.isPending}
        onAdd={() => setEditor({ kind: "template", value: null })}
        onEdit={(value) => setEditor({ kind: "template", value })}
        onDelete={(value) => requestDelete("template", value.id, value.revision)}
      />
      <DefinitionList
        kind="signature"
        values={signatures.data ?? []}
        loading={signatures.isPending}
        pendingDelete={pendingDelete}
        deleting={deletion.isPending}
        onAdd={() => setEditor({ kind: "signature", value: null })}
        onEdit={(value) => setEditor({ kind: "signature", value })}
        onDelete={(value) => requestDelete("signature", value.id, value.revision)}
      />
      {editor ? (
        <DefinitionEditor
          key={`${editor.kind}-${editor.value?.id ?? "new"}`}
          state={editor}
          onClose={() => setEditor(null)}
          onSave={saveDefinition}
        />
      ) : null}
    </Stack>
  );
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
}: {
  kind: "template" | "signature";
  values: T[];
  loading: boolean;
  pendingDelete: PendingDelete;
  deleting: boolean;
  onAdd: () => void;
  onEdit: (value: T) => void;
  onDelete: (value: T) => void;
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
      {values.map((value) => {
        const confirming = pendingDelete?.kind === kind && pendingDelete.id === value.id;
        const subject = "subject" in value ? value.subject : "";
        return (
          <Surface key={value.id} className="p-4 shadow-none ring-1 ring-border/70">
            <Inline className="items-start justify-between">
              <Stack className="min-w-0 flex-1" gap="xs">
                <LabelText className="truncate">{value.name}</LabelText>
                {subject ? <Text className="truncate text-xs text-foreground">{subject}</Text> : null}
                <Text className="line-clamp-2 whitespace-pre-line text-xs">
                  {value.content.plainText || t("compositionLibrary.emptyContent")}
                </Text>
              </Stack>
              <Inline className="shrink-0 gap-1">
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
              </Inline>
            </Inline>
          </Surface>
        );
      })}
    </Stack>
  );
}

function DefinitionEditor({
  state,
  onClose,
  onSave,
}: {
  state: Exclude<DefinitionEditorState, null>;
  onClose: () => void;
  onSave: (
    state: Exclude<DefinitionEditorState, null>,
    name: string,
    subject: string,
    content: DraftContent,
  ) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(state.value?.name ?? "");
  const [subject, setSubject] = useState(state.kind === "template" ? state.value?.subject ?? "" : "");
  const [content, setContent] = useState(state.value?.content ?? EMPTY_CONTENT);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const label = state.kind === "template" ? t("compositionLibrary.templateContent") : t("compositionLibrary.signatureContent");

  async function submit() {
    setSaving(true);
    setError(null);
    try {
      await onSave(state, name, subject, content);
    } catch (reason) {
      setError(reason);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open
      onOpenChange={(open) => { if (!open && !saving) onClose(); }}
      title={t(`compositionLibrary.${state.value ? "edit" : "new"}${state.kind === "template" ? "Template" : "Signature"}`)}
      closeLabel={t("common.close")}
      contentClassName="w-[min(780px,calc(100vw-40px))]"
    >
      <Form className="mt-5" onSubmit={(event) => { event.preventDefault(); void submit(); }}>
        <Stack gap="md">
          <TextField
            label={t("compositionLibrary.name")}
            value={name}
            maxLength={80}
            autoFocus
            onChange={(event) => setName(event.target.value)}
          />
          {state.kind === "template" ? (
            <TextField
              label={t("composer.subject")}
              value={subject}
              onChange={(event) => setSubject(event.target.value)}
            />
          ) : null}
          <Stack gap="xs">
            <LabelText>{label}</LabelText>
            <Page className="h-[330px] overflow-hidden rounded-lg ring-1 ring-border">
              <RichTextEditor
                initialJson={content.editorJson}
                ariaLabel={label}
                disabled={saving}
                onChange={setContent}
              />
            </Page>
          </Stack>
          {error ? (
            <Alert tone="danger" title={t("errors.title")}>
              {t(`errors.${normalizeCommandError(error).code}`, { defaultValue: t("common.unexpectedError") })}
            </Alert>
          ) : null}
          <Inline className="justify-end">
            <Button type="button" variant="ghost" disabled={saving} onClick={onClose}>{t("common.cancel")}</Button>
            <Button type="submit" loading={saving} disabled={!name.trim()}>{t("common.save")}</Button>
          </Inline>
        </Stack>
      </Form>
    </Modal>
  );
}

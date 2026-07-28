import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type {
  DraftContent,
  MailSignature,
  MailTemplate,
} from "@/app/types";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TextField } from "@/components/ui/input";
import { AppShell, Form, Inline, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { fileToBase64 } from "@/features/composer/fileToBase64";
import { RichTextEditor } from "@/features/composer/RichTextEditor";

const EMPTY_CONTENT: DraftContent = {
  editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
  html: "<p></p>",
  plainText: "",
};

export type CompositionDefinitionKind = "template" | "signature";

interface CompositionDefinitionEditorAppProps {
  accountId: string | null;
  kind: CompositionDefinitionKind;
  definitionId: string | null;
}

export function CompositionDefinitionEditorApp({
  accountId,
  kind,
  definitionId,
}: CompositionDefinitionEditorAppProps) {
  const { t } = useTranslation();
  const definitionQuery = useQuery({
    queryKey: ["composition-definition-editor", kind, accountId ?? "global", definitionId],
    queryFn: async () => {
      const values = kind === "template"
        ? await api.listMailTemplates(accountId)
        : await api.listMailSignatures(accountId);
      return values.find((value) => value.id === definitionId) ?? null;
    },
    enabled: Boolean(definitionId),
  });
  const loading = Boolean(definitionId) && definitionQuery.isPending;
  useRevealWindowWhenReady(!loading);

  if (loading) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (definitionQuery.isError || (definitionId && !definitionQuery.data)) {
    const error = normalizeCommandError(definitionQuery.error);
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <Alert tone="danger" title={t("errors.title")}>
          {definitionId && !definitionQuery.isError
            ? t("compositionLibrary.definitionNotFound")
            : t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      </AppShell>
    );
  }

  return (
    <DefinitionEditorForm
      key={`${kind}-${definitionId ?? "new"}`}
      accountId={accountId}
      kind={kind}
      value={definitionQuery.data ?? null}
    />
  );
}

function DefinitionEditorForm({
  accountId,
  kind,
  value,
}: {
  accountId: string | null;
  kind: CompositionDefinitionKind;
  value: MailTemplate | MailSignature | null;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(value?.name ?? "");
  const [subject, setSubject] = useState(
    kind === "template" && value && "subject" in value ? value.subject : "",
  );
  const [content, setContent] = useState(value?.content ?? EMPTY_CONTENT);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const title = t(`compositionLibrary.${value ? "edit" : "new"}${kind === "template" ? "Template" : "Signature"}`);
  const label = kind === "template"
    ? t("compositionLibrary.templateContent")
    : t("compositionLibrary.signatureContent");

  async function close() {
    if ("__TAURI_INTERNALS__" in globalThis) await getCurrentWindow().destroy();
  }

  async function submit() {
    setSaving(true);
    setError(null);
    try {
      if (kind === "template") {
        const draft = { name, subject, content };
        if (value) {
          await api.updateMailTemplate(accountId, value.id, draft, value.revision);
        } else {
          await api.createMailTemplate(accountId, draft);
        }
      } else {
        const draft = { name, content };
        if (value) {
          await api.updateMailSignature(accountId, value.id, draft, value.revision);
        } else {
          await api.createMailSignature(accountId, draft);
        }
      }
      await close();
    } catch (reason) {
      setError(reason);
    } finally {
      setSaving(false);
    }
  }

  async function addInlineImage(file: File) {
    try {
      if (file.size > 3 * 1024 * 1024) {
        throw {
          code: "definition.image_too_large",
          params: {},
          retryable: false,
        };
      }
      const prepared = await api.prepareCompositionDefinitionImage(
        file.name || "inline-image",
        file.type,
        await fileToBase64(file),
      );
      setError(null);
      return {
        fileName: prepared.fileName,
        contentType: prepared.contentType,
        size: prepared.size,
        contentId: null,
        previewDataUrl: prepared.dataUrl,
      };
    } catch (reason) {
      setError(reason);
      throw reason;
    }
  }

  return (
    <AppShell className="overflow-hidden bg-card">
      <Form
        className="flex size-full min-h-0 flex-col"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Stack className="shrink-0 border-b border-border px-6 pt-5 pb-4" gap="sm">
          <Stack gap="xs">
            <Heading level={1} className="text-xl">{title}</Heading>
            <Text className="text-xs">{t("compositionLibrary.editorWindowDescription")}</Text>
          </Stack>
          <Inline className="items-start">
            <TextField
              label={t("compositionLibrary.name")}
              value={name}
              maxLength={80}
              autoFocus
              disabled={saving}
              onChange={(event) => setName(event.target.value)}
            />
            {kind === "template" ? (
              <TextField
                label={t("composer.subject")}
                value={subject}
                disabled={saving}
                onChange={(event) => setSubject(event.target.value)}
              />
            ) : null}
          </Inline>
          <Text className="text-xs">{t("compositionLibrary.variablesHint")}</Text>
        </Stack>
        <Page className="flex min-h-0 flex-1 flex-col px-6 pt-4">
          <LabelText className="mb-2 shrink-0">{label}</LabelText>
          <Page className="flex min-h-0 flex-1 overflow-hidden rounded-lg ring-1 ring-border">
            <RichTextEditor
              initialJson={content.editorJson}
              initialHtml={content.html}
              ariaLabel={label}
              disabled={saving}
              onChange={setContent}
              onAddInlineImage={addInlineImage}
              onSanitizeHtml={api.sanitizeRichTextPaste}
            />
          </Page>
        </Page>
        <Stack className="shrink-0 px-6 pt-3 pb-5" gap="sm">
          {error ? (
            <Alert tone="danger" title={t("errors.title")}>
              {t(`errors.${normalizeCommandError(error).code}`, { defaultValue: t("common.unexpectedError") })}
            </Alert>
          ) : null}
          <Inline className="justify-end">
            <Button type="button" variant="ghost" disabled={saving} onClick={() => void close()}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" loading={saving} disabled={!name.trim()}>
              {t("common.save")}
            </Button>
          </Inline>
        </Stack>
      </Form>
    </AppShell>
  );
}

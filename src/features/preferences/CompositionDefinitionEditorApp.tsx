import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type {
  DraftContent,
  DraftRecipientFields,
  MailSignature,
  MailTemplate,
  MessageAddress,
} from "@/app/types";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { CompactField } from "@/components/ui/compact-field";
import { TextField } from "@/components/ui/input";
import { AppShell, Form, Inline, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { fileToBase64 } from "@/features/composer/fileToBase64";
import { RecipientField } from "@/features/composer/RecipientField";
import { addRecipientInput, formatAddress } from "@/features/composer/recipient-utils";
import { RichTextEditor } from "@/features/composer/RichTextEditor";
import { mailQueryKeys } from "@/features/mail/mail-query-keys";

const EMPTY_CONTENT: DraftContent = {
  editorJson: '{"type":"doc","content":[{"type":"paragraph"}]}',
  html: "<p></p>",
  plainText: "",
};

export type CompositionDefinitionKind = "template" | "signature";
type RecipientKind = keyof DraftRecipientFields;

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
  const templateRecipients = kind === "template" && value && "subject" in value
    ? value.recipients
    : null;
  const [to, setTo] = useState(templateRecipients?.to ?? []);
  const [cc, setCc] = useState(templateRecipients?.cc ?? []);
  const [bcc, setBcc] = useState(templateRecipients?.bcc ?? []);
  const [toInput, setToInput] = useState("");
  const [ccInput, setCcInput] = useState("");
  const [bccInput, setBccInput] = useState("");
  const [recipientErrors, setRecipientErrors] = useState<Record<RecipientKind, string | null>>({
    to: null,
    cc: null,
    bcc: null,
  });
  const lastSelectedAccount = useQuery({
    queryKey: ["last-selected-account"],
    queryFn: api.getLastSelectedAccount,
    enabled: kind === "template" && accountId === null,
  });
  const contactAccountId = accountId ?? lastSelectedAccount.data ?? null;
  const recipientAddresses = useMemo(() => [...to, ...cc, ...bcc], [to, cc, bcc]);
  const recipientEmails = useMemo(
    () => [...new Set(recipientAddresses.map((address) => address.email.trim().toLocaleLowerCase()))].sort(),
    [recipientAddresses],
  );
  const resolvedRecipients = useQuery({
    queryKey: mailQueryKeys.contactAddresses(contactAccountId ?? "", recipientEmails),
    queryFn: () => api.resolveContactAddresses(contactAccountId!, recipientAddresses),
    enabled: Boolean(contactAccountId && recipientAddresses.length),
  });
  const resolvedRecipientsByEmail = useMemo(
    () => new Map((resolvedRecipients.data ?? []).map((address) => [address.email.trim().toLocaleLowerCase(), address])),
    [resolvedRecipients.data],
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
    const recipients = kind === "template" ? resolveRecipients() : null;
    if (kind === "template" && !recipients) return;
    setSaving(true);
    setError(null);
    try {
      if (kind === "template") {
        const draft = { name, subject, recipients: recipients!, content };
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

  function recipientValue(recipientKind: RecipientKind) {
    if (recipientKind === "to") return { addresses: to, input: toInput };
    if (recipientKind === "cc") return { addresses: cc, input: ccInput };
    return { addresses: bcc, input: bccInput };
  }

  function setRecipientAddresses(recipientKind: RecipientKind, addresses: MessageAddress[]) {
    if (recipientKind === "to") setTo(addresses);
    else if (recipientKind === "cc") setCc(addresses);
    else setBcc(addresses);
  }

  function setRecipientInput(recipientKind: RecipientKind, input: string) {
    if (recipientKind === "to") setToInput(input);
    else if (recipientKind === "cc") setCcInput(input);
    else setBccInput(input);
    setRecipientErrors((current) => ({ ...current, [recipientKind]: null }));
  }

  function commitRecipient(recipientKind: RecipientKind) {
    const current = recipientValue(recipientKind);
    const result = addRecipientInput(current.addresses, current.input);
    if (result.invalid) {
      setRecipientErrors((errors) => ({ ...errors, [recipientKind]: result.invalid }));
      return;
    }
    setRecipientAddresses(recipientKind, result.addresses);
    setRecipientInput(recipientKind, "");
  }

  function removeRecipient(recipientKind: RecipientKind, index: number) {
    setRecipientAddresses(
      recipientKind,
      recipientValue(recipientKind).addresses.filter((_, itemIndex) => itemIndex !== index),
    );
  }

  function editLastRecipient(recipientKind: RecipientKind, address: MessageAddress, index: number) {
    removeRecipient(recipientKind, index);
    setRecipientInput(recipientKind, formatAddress(address));
  }

  function selectContactRecipient(recipientKind: RecipientKind, contact: { name: string; email: string }) {
    const current = recipientValue(recipientKind);
    const normalizedEmail = contact.email.trim().toLocaleLowerCase();
    if (!current.addresses.some((address) => address.email.trim().toLocaleLowerCase() === normalizedEmail)) {
      setRecipientAddresses(recipientKind, [
        ...current.addresses,
        { name: contact.name, email: contact.email },
      ]);
    }
    setRecipientInput(recipientKind, "");
  }

  function resolveRecipients(): DraftRecipientFields | null {
    const resolved = {
      to: addRecipientInput(to, toInput),
      cc: addRecipientInput(cc, ccInput),
      bcc: addRecipientInput(bcc, bccInput),
    };
    const errors = {
      to: resolved.to.invalid,
      cc: resolved.cc.invalid,
      bcc: resolved.bcc.invalid,
    };
    setRecipientErrors(errors);
    if (errors.to || errors.cc || errors.bcc) return null;
    return {
      to: resolved.to.addresses,
      cc: resolved.cc.addresses,
      bcc: resolved.bcc.addresses,
    };
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
        <Stack className="shrink-0 border-b border-border/70 bg-card px-6 pt-4 pb-4" gap="sm">
          <Stack gap="xs">
            <Heading level={1} className="text-lg lg:text-lg">{title}</Heading>
            <Text className="text-xs">{t("compositionLibrary.editorWindowDescription")}</Text>
          </Stack>
          <TextField
            label={t("compositionLibrary.name")}
            value={name}
            maxLength={80}
            autoFocus
            disabled={saving}
            onChange={(event) => setName(event.target.value)}
          />
          {kind === "template" ? (
            <Stack className="overflow-visible border-t border-border/70 bg-card" gap="none">
              {(["to", "cc", "bcc"] as const).map((recipientKind) => {
                const current = recipientValue(recipientKind);
                return (
                  <RecipientField
                    key={recipientKind}
                    accountId={contactAccountId ?? undefined}
                    label={t(`composer.${recipientKind}`)}
                    addresses={current.addresses}
                    resolvedAddresses={resolvedRecipientsByEmail}
                    input={current.input}
                    error={recipientErrors[recipientKind]
                      ? t("composer.invalidRecipient", { value: recipientErrors[recipientKind] })
                      : null}
                    disabled={saving}
                    structured
                    onInputChange={(input) => setRecipientInput(recipientKind, input)}
                    onCommit={() => commitRecipient(recipientKind)}
                    onRemove={(index) => removeRecipient(recipientKind, index)}
                    onEditLast={(address, index) => editLastRecipient(recipientKind, address, index)}
                    onSelectContact={(contact) => selectContactRecipient(recipientKind, contact)}
                  />
                );
              })}
              <CompactField
                structured
                label={t("compositionLibrary.mailSubject")}
                value={subject}
                disabled={saving}
                onChange={(event) => setSubject(event.currentTarget.value)}
              />
            </Stack>
          ) : null}
          <Text className="text-xs">{t("compositionLibrary.variablesHint")}</Text>
        </Stack>
        <Page className="flex min-h-0 flex-1 flex-col px-6 pt-4">
          <LabelText className="mb-2 shrink-0">{label}</LabelText>
          <Page className="flex min-h-0 flex-1 overflow-hidden rounded-lg border border-border/80 bg-card shadow-[var(--shadow-raised)]">
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
        <Stack className="shrink-0 border-t border-border/70 bg-muted/20 px-6 py-3" gap="sm">
          {error ? (
            <Alert tone="danger" title={t("errors.title")}>
              {t(`errors.${normalizeCommandError(error).code}`, { defaultValue: t("common.unexpectedError") })}
            </Alert>
          ) : null}
          <Inline className="flex-wrap justify-end">
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

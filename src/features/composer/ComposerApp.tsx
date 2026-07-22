import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Paperclip, Send, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useAppearancePreferences } from "@/app/appearance";
import type {
  ComposerBootstrap,
  DraftAttachmentSummary,
  DraftContent,
  DraftRecipientFields,
  MessageAddress,
  SendJobSummary,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { CompactField } from "@/components/ui/compact-field";
import { Modal } from "@/components/ui/dialog";
import { AppShell, Inline, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { SelectField } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Text } from "@/components/ui/typography";
import {
  RichTextEditor,
  type CompositionNodeSelection,
  type RichTextEditorHandle,
} from "./RichTextEditor";
import { AddressTag, RecipientField } from "./RecipientField";
import { addRecipientInput, formatAddress } from "./recipient-utils";

interface ComposerAppProps {
  accountId: string;
  draftId: string;
}

export function ComposerApp({ accountId, draftId }: ComposerAppProps) {
  const { t } = useTranslation();
  const preferences = useAppearancePreferences();
  const bootstrap = useQuery({
    queryKey: ["composer", accountId, draftId],
    queryFn: () => api.getComposerBootstrap(accountId, draftId),
  });

  if (preferences.isPending || bootstrap.isPending) {
    return <AppShell className="grid place-items-center"><Spinner size={24} /></AppShell>;
  }
  if (preferences.isError || bootstrap.isError || !bootstrap.data) {
    const error = normalizeCommandError(preferences.error ?? bootstrap.error);
    return (
      <AppShell className="grid place-items-center p-8">
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      </AppShell>
    );
  }
  return <ComposerWorkspace key={draftId} bootstrap={bootstrap.data} />;
}

function ComposerWorkspace({ bootstrap }: { bootstrap: ComposerBootstrap }) {
  const { t } = useTranslation();
  const { draft, sender } = bootstrap;
  const [to, setTo] = useState(draft.recipients.to);
  const [cc, setCc] = useState(draft.recipients.cc);
  const [bcc, setBcc] = useState(draft.recipients.bcc);
  const [toInput, setToInput] = useState("");
  const [ccInput, setCcInput] = useState("");
  const [bccInput, setBccInput] = useState("");
  const [recipientErrors, setRecipientErrors] = useState<Record<RecipientKind, string | null>>({ to: null, cc: null, bcc: null });
  const [showCopies, setShowCopies] = useState(Boolean(cc.length || bcc.length));
  const [subject, setSubject] = useState(draft.subject);
  const [content, setContent] = useState<DraftContent>(draft.content);
  const [attachments, setAttachments] = useState(draft.attachments);
  const [revision, setRevision] = useState(draft.revision);
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<"saved" | "saving" | "failed">("saved");
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [sendJob, setSendJob] = useState<SendJobSummary | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [confirmEmptySubject, setConfirmEmptySubject] = useState(false);
  const [saveRetry, setSaveRetry] = useState(0);
  const initialComposition = compositionSelection(draft.content.editorJson);
  const [templateId, setTemplateId] = useState(initialComposition.templateId ?? "none");
  const [signatureId, setSignatureId] = useState(initialComposition.signatureId ?? "none");
  const [switchingDefinition, setSwitchingDefinition] = useState(false);
  const editorRef = useRef<RichTextEditorHandle>(null);
  const savingRef = useRef(false);
  const revisionRef = useRef(revision);
  const changeVersionRef = useRef(0);
  const editable = draft.status === "editing" && !sendJob && !submitting;
  const sending = submitting || sendJob?.status === "queued" || sendJob?.status === "sending";

  useEffect(() => { revisionRef.current = revision; }, [revision]);

  const resolveAllRecipients = useCallback((): DraftRecipientFields | null => {
    const resolved = {
      to: addRecipientInput(to, toInput),
      cc: addRecipientInput(cc, ccInput),
      bcc: addRecipientInput(bcc, bccInput),
    };
    const nextErrors = {
      to: resolved.to.invalid,
      cc: resolved.cc.invalid,
      bcc: resolved.bcc.invalid,
    };
    setRecipientErrors(nextErrors);
    if (nextErrors.to || nextErrors.cc || nextErrors.bcc) {
      setErrorCode("draft.recipient_invalid");
      return null;
    }
    if (toInput.trim()) { setTo(resolved.to.addresses); setToInput(""); }
    if (ccInput.trim()) { setCc(resolved.cc.addresses); setCcInput(""); }
    if (bccInput.trim()) { setBcc(resolved.bcc.addresses); setBccInput(""); }
    return {
      to: resolved.to.addresses,
      cc: resolved.cc.addresses,
      bcc: resolved.bcc.addresses,
    };
  }, [bcc, bccInput, cc, ccInput, to, toInput]);

  const saveNow = useCallback(async (commitPendingRecipients = false) => {
    if (!dirty || savingRef.current || !editable) return null;
    const resolvedRecipients = commitPendingRecipients
      ? resolveAllRecipients()
      : { to, cc, bcc };
    if (!resolvedRecipients) {
      setSaveState("failed");
      return null;
    }
    savingRef.current = true;
    const savingVersion = changeVersionRef.current;
    setSaveState("saving");
    try {
      const saved = await api.saveDraft(
        sender.id,
        draft.id,
        resolvedRecipients,
        subject,
        content,
        revisionRef.current,
      );
      revisionRef.current = saved.revision;
      setRevision(saved.revision);
      if (savingVersion === changeVersionRef.current) {
        setDirty(false);
      }
      setSaveState("saved");
      setErrorCode(null);
      return saved;
    } catch (error) {
      const normalized = normalizeCommandError(error);
      setSaveState("failed");
      setErrorCode(normalized.code);
      return null;
    } finally {
      savingRef.current = false;
      if (savingVersion !== changeVersionRef.current) {
        setSaveRetry((value) => value + 1);
      }
    }
  }, [bcc, cc, content, dirty, draft.id, editable, resolveAllRecipients, sender.id, subject, to]);
  const saveNowRef = useRef(saveNow);
  saveNowRef.current = saveNow;
  const closeStateRef = useRef({ dirty, editable, sendJob, submitting });
  closeStateRef.current = { dirty, editable, sendJob, submitting };

  useEffect(() => {
    if (!dirty || !editable || toInput.trim() || ccInput.trim() || bccInput.trim()) return;
    const timeout = window.setTimeout(() => void saveNow(false), 800);
    return () => window.clearTimeout(timeout);
  }, [bccInput, ccInput, dirty, editable, saveNow, saveRetry, toInput]);

  useEffect(() => {
    if (!editable || dirty || revision === draft.revision) return;
    const timeout = window.setTimeout(
      () => void api.queueRemoteDraft(sender.id, draft.id).catch(() => undefined),
      10_000,
    );
    return () => window.clearTimeout(timeout);
  }, [dirty, draft.id, draft.revision, editable, revision, sender.id]);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    const unlisten = currentWindow.onCloseRequested(async (event) => {
      event.preventDefault();
      const closeState = closeStateRef.current;
      if (closeState.sendJob?.status === "sent") {
        try {
          await currentWindow.destroy();
        } catch (error) {
          setErrorCode(normalizeCommandError(error).code);
        }
        return;
      }
      if (closeState.sendJob || closeState.submitting) return;
      if (closeState.dirty && closeState.editable) {
        const saved = await saveNowRef.current(true);
        if (!saved) return;
      }
      try {
        const discarded = await api.discardEmptyDraft(sender.id, draft.id);
        if (!discarded) await api.queueRemoteDraft(sender.id, draft.id);
        await currentWindow.destroy();
      } catch (error) {
        setErrorCode(normalizeCommandError(error).code);
      }
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [draft.id, sender.id]);

  useEffect(() => {
    if (!sendJob) return;
    const unlisten = listen<{ jobId: string }>("send-job-changed", (event) => {
      if (event.payload.jobId !== sendJob.id) return;
      void api.getSendJob(sender.id, sendJob.id).then(setSendJob);
    });
    const interval = window.setInterval(() => {
      void api.getSendJob(sender.id, sendJob.id).then(setSendJob);
    }, 1_500);
    return () => {
      window.clearInterval(interval);
      void unlisten.then((dispose) => dispose());
    };
  }, [sendJob?.id, sender.id]);

  useEffect(() => {
    if (sendJob?.status !== "sent") return;
    const timeout = window.setTimeout(() => {
      void getCurrentWindow()
        .destroy()
        .catch((error) => setErrorCode(normalizeCommandError(error).code));
    }, 80);
    return () => window.clearTimeout(timeout);
  }, [sendJob?.status]);

  const markDirty = () => {
    changeVersionRef.current += 1;
    setDirty(true);
  };

  const recipients = (): DraftRecipientFields => ({
    to: addRecipientInput(to, toInput).addresses,
    cc: addRecipientInput(cc, ccInput).addresses,
    bcc: addRecipientInput(bcc, bccInput).addresses,
  });

  function recipientValue(kind: RecipientKind) {
    if (kind === "to") return { addresses: to, input: toInput };
    if (kind === "cc") return { addresses: cc, input: ccInput };
    return { addresses: bcc, input: bccInput };
  }

  function setRecipientAddresses(kind: RecipientKind, value: MessageAddress[]) {
    if (kind === "to") setTo(value);
    else if (kind === "cc") setCc(value);
    else setBcc(value);
  }

  function setRecipientInput(kind: RecipientKind, value: string) {
    if (kind === "to") setToInput(value);
    else if (kind === "cc") setCcInput(value);
    else setBccInput(value);
    setRecipientErrors((current) => ({ ...current, [kind]: null }));
    markDirty();
  }

  function commitRecipient(kind: RecipientKind) {
    const current = recipientValue(kind);
    const result = addRecipientInput(current.addresses, current.input);
    if (result.invalid) {
      setRecipientErrors((errors) => ({ ...errors, [kind]: result.invalid }));
      return;
    }
    setRecipientAddresses(kind, result.addresses);
    if (kind === "to") setToInput("");
    else if (kind === "cc") setCcInput("");
    else setBccInput("");
    setRecipientErrors((errors) => ({ ...errors, [kind]: null }));
  }

  function removeRecipient(kind: RecipientKind, index: number) {
    setRecipientAddresses(kind, recipientValue(kind).addresses.filter((_, itemIndex) => itemIndex !== index));
    markDirty();
  }

  function editLastRecipient(kind: RecipientKind, address: MessageAddress, index: number) {
    setRecipientAddresses(kind, recipientValue(kind).addresses.filter((_, itemIndex) => itemIndex !== index));
    if (kind === "to") setToInput(formatAddress(address));
    else if (kind === "cc") setCcInput(formatAddress(address));
    else setBccInput(formatAddress(address));
    setRecipientErrors((errors) => ({ ...errors, [kind]: null }));
    markDirty();
  }

  async function selectTemplate(value: string) {
    setSwitchingDefinition(true);
    try {
      if (value === "none") {
        editorRef.current?.replaceTemplate(null);
        setTemplateId("none");
      } else {
        const rendered = await api.renderMailTemplate(sender.id, value, recipients());
        if (rendered.subject.trim()) setSubject(rendered.subject);
        editorRef.current?.replaceTemplate(rendered.id, rendered.content);
        setTemplateId(rendered.id);
      }
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    } finally {
      setSwitchingDefinition(false);
    }
  }

  async function selectSignature(value: string) {
    setSwitchingDefinition(true);
    try {
      if (value === "none") {
        editorRef.current?.replaceSignature(null);
        setSignatureId("none");
      } else {
        const rendered = await api.renderMailSignature(sender.id, value, recipients());
        editorRef.current?.replaceSignature(rendered.id, rendered.content);
        setSignatureId(rendered.id);
      }
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    } finally {
      setSwitchingDefinition(false);
    }
  }

  async function addAttachments() {
    const selected = await open({ multiple: true, directory: false });
    const paths = typeof selected === "string" ? [selected] : selected ?? [];
    if (!paths.length) return;
    try {
      const added = await api.addDraftAttachments(sender.id, draft.id, paths);
      setAttachments((current) => [...current, ...added]);
      markDirty();
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    }
  }

  async function removeAttachment(attachment: DraftAttachmentSummary) {
    try {
      await api.removeDraftAttachment(sender.id, draft.id, attachment.id);
      setAttachments((current) => current.filter((item) => item.id !== attachment.id));
      markDirty();
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    }
  }

  async function addInlineImage(file: File) {
    try {
      const added = await api.addDraftInlineImage(
        sender.id,
        draft.id,
        file.name || "pasted-image",
        file.type,
        await fileToBase64(file),
      );
      setAttachments((current) => [...current, added]);
      setErrorCode(null);
      return added;
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
      throw error;
    }
  }

  async function sanitizeRichTextPaste(html: string) {
    try {
      const sanitized = await api.sanitizeRichTextPaste(html);
      setErrorCode(null);
      return sanitized;
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
      throw error;
    }
  }

  async function sendMessage() {
    if (!subject.trim() && !confirmEmptySubject) {
      setConfirmEmptySubject(true);
      return;
    }
    setConfirmEmptySubject(false);
    setSubmitting(true);
    const saved = await saveNow(true);
    if (dirty && !saved) {
      setSubmitting(false);
      return;
    }
    try {
      const job = await api.queueDraftSend(sender.id, draft.id);
      setSendJob(job);
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    } finally {
      setSubmitting(false);
    }
  }

  async function retrySend() {
    if (!sendJob) return;
    try { setSendJob(await api.retrySendJob(sender.id, sendJob.id)); }
    catch (error) { setErrorCode(normalizeCommandError(error).code); }
  }

  return (
    <AppShell className="flex min-h-0 flex-col overflow-hidden">
      <Inline className="h-14 shrink-0 bg-card px-3">
        <Button className="shadow-none" loading={sending} onClick={() => void sendMessage()} disabled={!editable || saveState === "saving"}>
          <Send size={16} />{t("composer.send")}
        </Button>
        <Button variant="ghost" onClick={() => void addAttachments()} disabled={!editable}>
          <Paperclip size={16} />{t("composer.attach")}
        </Button>
        <Page className="flex-1" />
        <Text className="text-xs" aria-live="polite">
          {saveState === "saving" ? t("composer.saving") : saveState === "failed" ? t("composer.saveFailed") : t("composer.saved")}
        </Text>
      </Inline>

      <Page className="flex min-h-0 flex-1 flex-col">
        <Inline className="min-h-11 gap-0 bg-card">
          <Text className="w-20 shrink-0 px-4 text-xs font-semibold">{t("composer.from")}</Text>
          <AddressTag address={{ name: sender.displayName || null, email: sender.email }} />
        </Inline>
        <Separator className="ml-20" />
        <RecipientField
          label={t("composer.to")}
          addresses={to}
          input={toInput}
          error={recipientErrors.to ? t("composer.invalidRecipient", { value: recipientErrors.to }) : null}
          disabled={!editable}
          placeholder={t("composer.recipientPlaceholder")}
          onInputChange={(value) => setRecipientInput("to", value)}
          onCommit={() => commitRecipient("to")}
          onRemove={(index) => removeRecipient("to", index)}
          onEditLast={(address, index) => editLastRecipient("to", address, index)}
          trailing={
            <Button type="button" variant="ghost" size="sm" className="mr-2" onClick={() => setShowCopies((value) => !value)}>
              {t("composer.ccBcc")}<ChevronDown size={14} />
            </Button>
          }
        />
        <Separator className="ml-20" />
        {showCopies ? (
          <>
            <RecipientField
              label={t("composer.cc")}
              addresses={cc}
              input={ccInput}
              error={recipientErrors.cc ? t("composer.invalidRecipient", { value: recipientErrors.cc }) : null}
              disabled={!editable}
              onInputChange={(value) => setRecipientInput("cc", value)}
              onCommit={() => commitRecipient("cc")}
              onRemove={(index) => removeRecipient("cc", index)}
              onEditLast={(address, index) => editLastRecipient("cc", address, index)}
            />
            <Separator className="ml-20" />
            <RecipientField
              label={t("composer.bcc")}
              addresses={bcc}
              input={bccInput}
              error={recipientErrors.bcc ? t("composer.invalidRecipient", { value: recipientErrors.bcc }) : null}
              disabled={!editable}
              onInputChange={(value) => setRecipientInput("bcc", value)}
              onCommit={() => commitRecipient("bcc")}
              onRemove={(index) => removeRecipient("bcc", index)}
              onEditLast={(address, index) => editLastRecipient("bcc", address, index)}
            />
            <Separator className="ml-20" />
          </>
        ) : null}
        <CompactField label={t("composer.subject")} value={subject} disabled={!editable} onChange={(event) => { setSubject(event.currentTarget.value); markDirty(); }} />
        <Separator />
        <Inline className="min-h-12 shrink-0 flex-wrap bg-card px-4 py-2">
          <SelectField
            compact
            label={t("composer.template")}
            value={templateId}
            options={[
              { value: "none", label: t("composer.noTemplate") },
              ...bootstrap.templates.map((value) => ({
                value: value.id,
                label: definitionLabel(value.name, value.scope, t),
              })),
            ]}
            disabled={!editable || switchingDefinition}
            onValueChange={(value) => void selectTemplate(value)}
          />
          <SelectField
            compact
            label={t("composer.signature")}
            value={signatureId}
            options={[
              { value: "none", label: t("composer.noSignature") },
              ...bootstrap.signatures.map((value) => ({
                value: value.id,
                label: definitionLabel(value.name, value.scope, t),
              })),
            ]}
            disabled={!editable || switchingDefinition}
            onValueChange={(value) => void selectSignature(value)}
          />
        </Inline>
        <Separator />

        {errorCode ? (
          <Alert className="m-3 mb-0" tone="danger">{t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}</Alert>
        ) : null}
        {sendJob?.status === "failed" ? <SendFailure job={sendJob} onRetry={retrySend} /> : null}
        {attachments.some((attachment) => !attachment.isInline) ? (
          <Inline className="flex-wrap bg-muted/50 px-4 py-2">
            {attachments.filter((attachment) => !attachment.isInline).map((attachment) => (
              <Inline key={attachment.id} className="rounded-md border-0 bg-card px-2.5 py-1.5 shadow-xs">
                <Paperclip size={14} /><Text className="max-w-56 truncate text-xs text-foreground">{attachment.fileName}</Text>
                <Text className="text-[length:var(--ui-font-caption)]">{formatBytes(attachment.size)}</Text>
                {editable ? (
                  <Button variant="ghost" size="icon" className="size-6" aria-label={t("composer.removeAttachment")} onClick={() => void removeAttachment(attachment)}>
                    <Trash2 size={13} />
                  </Button>
                ) : null}
              </Inline>
            ))}
          </Inline>
        ) : null}
        <RichTextEditor
          ref={editorRef}
          initialJson={draft.content.editorJson}
          disabled={!editable}
          inlineImages={attachments}
          onAddInlineImage={addInlineImage}
          onSanitizeHtml={sanitizeRichTextPaste}
          onChange={(value) => { setContent(value); markDirty(); }}
          onCompositionChange={(value) => {
            setTemplateId(value.templateId ?? "none");
            setSignatureId(value.signatureId ?? "none");
          }}
        />
      </Page>

      <Modal open={confirmEmptySubject} onOpenChange={setConfirmEmptySubject} title={t("composer.emptySubjectTitle")} closeLabel={t("common.close")}>
        <Stack className="mt-4">
          <Text>{t("composer.emptySubjectDescription")}</Text>
          <Inline className="justify-end">
            <Button variant="secondary" onClick={() => setConfirmEmptySubject(false)}>{t("common.cancel")}</Button>
            <Button onClick={() => void sendMessage()}>{t("composer.sendAnyway")}</Button>
          </Inline>
        </Stack>
      </Modal>
      {sending ? (
        <SendProgressOverlay
          status={submitting ? "preparing" : sendJob?.status === "sending" ? "sending" : "queued"}
        />
      ) : null}
    </AppShell>
  );
}

function SendFailure({ job, onRetry }: { job: SendJobSummary; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <Alert className="m-3 mb-0" tone="danger" title={t("composer.sendFailed")}>
      <Inline><Text>{t(`errors.${job.errorCode}`, { defaultValue: t("composer.sendFailedDescription") })}</Text><Button size="sm" variant="secondary" onClick={onRetry}>{t("common.retry")}</Button></Inline>
    </Alert>
  );
}

function SendProgressOverlay({ status }: { status: "preparing" | "queued" | "sending" }) {
  const { t } = useTranslation();
  return (
    <Page
      className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6 backdrop-blur-[2px]"
      role="dialog"
      aria-modal="true"
      aria-label={t("composer.sendProgressTitle")}
    >
      <Stack className="w-[min(22rem,calc(100vw-3rem))] items-center rounded-lg border-0 bg-popover px-8 py-7 text-center text-popover-foreground shadow-2xl" gap="sm">
        <Spinner size={30} />
        <Text className="font-semibold text-foreground">{t("composer.sendProgressTitle")}</Text>
        <Text className="text-xs">{t(`composer.${status}`)}</Text>
      </Stack>
    </Page>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

type RecipientKind = "to" | "cc" | "bcc";

function fileToBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("image read failed"));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const separator = result.indexOf(",");
      if (separator < 0) reject(new Error("image encoding failed"));
      else resolve(result.slice(separator + 1));
    };
    reader.readAsDataURL(file);
  });
}

function compositionSelection(editorJson: string): CompositionNodeSelection {
  try {
    type CompositionJsonNode = {
      type?: string;
      attrs?: { definitionId?: unknown };
      content?: CompositionJsonNode[];
    };
    const document = JSON.parse(editorJson) as CompositionJsonNode;
    const selection: CompositionNodeSelection = { templateId: null, signatureId: null };
    const visit = (node: CompositionJsonNode) => {
      const id = typeof node.attrs?.definitionId === "string" ? node.attrs.definitionId : null;
      if (node.type === "nextmailTemplate") selection.templateId = id;
      if (node.type === "nextmailSignature") selection.signatureId = id;
      node.content?.forEach(visit);
    };
    visit(document);
    return selection;
  } catch {
    return { templateId: null, signatureId: null };
  }
}

function definitionLabel(
  name: string,
  scope: "global" | "account",
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  return t("composer.definitionOption", {
    name,
    scope: t(`compositionLibrary.${scope}Badge`),
  });
}

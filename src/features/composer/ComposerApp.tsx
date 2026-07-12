import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useQuery } from "@tanstack/react-query";
import { Check, ChevronDown, Paperclip, Send, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { applyAppearance } from "@/app/appearance";
import type {
  ComposerBootstrap,
  DraftAttachmentSummary,
  DraftContent,
  SendJobSummary,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { CompactField } from "@/components/ui/compact-field";
import { Modal } from "@/components/ui/dialog";
import { AppShell, Inline, Page, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Text } from "@/components/ui/typography";
import { RichTextEditor } from "./RichTextEditor";
import { formatAddresses, parseAddresses } from "./recipient-utils";

interface ComposerAppProps {
  accountId: string;
  draftId: string;
}

export function ComposerApp({ accountId, draftId }: ComposerAppProps) {
  const { t, i18n } = useTranslation();
  const preferences = useQuery({ queryKey: ["preferences"], queryFn: api.getPreferences });
  const bootstrap = useQuery({
    queryKey: ["composer", accountId, draftId],
    queryFn: () => api.getComposerBootstrap(accountId, draftId),
  });

  useEffect(() => {
    if (!preferences.data) return;
    applyAppearance(preferences.data);
    void i18n.changeLanguage(preferences.data.language);
  }, [i18n, preferences.data]);

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
  const [to, setTo] = useState(formatAddresses(draft.recipients.to));
  const [cc, setCc] = useState(formatAddresses(draft.recipients.cc));
  const [bcc, setBcc] = useState(formatAddresses(draft.recipients.bcc));
  const [showCopies, setShowCopies] = useState(Boolean(cc || bcc));
  const [subject, setSubject] = useState(draft.subject);
  const [content, setContent] = useState<DraftContent>(draft.content);
  const [attachments, setAttachments] = useState(draft.attachments);
  const [revision, setRevision] = useState(draft.revision);
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<"saved" | "saving" | "failed">("saved");
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [sendJob, setSendJob] = useState<SendJobSummary | null>(null);
  const [confirmEmptySubject, setConfirmEmptySubject] = useState(false);
  const [saveRetry, setSaveRetry] = useState(0);
  const savingRef = useRef(false);
  const revisionRef = useRef(revision);
  const changeVersionRef = useRef(0);
  const editable = draft.status === "editing" && !sendJob;

  useEffect(() => { revisionRef.current = revision; }, [revision]);

  const saveNow = useCallback(async () => {
    if (!dirty || savingRef.current || !editable) return null;
    savingRef.current = true;
    const savingVersion = changeVersionRef.current;
    setSaveState("saving");
    try {
      const saved = await api.saveDraft(
        sender.id,
        draft.id,
        { to: parseAddresses(to), cc: parseAddresses(cc), bcc: parseAddresses(bcc) },
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
  }, [bcc, cc, content, dirty, draft.id, editable, sender.id, subject, to]);

  useEffect(() => {
    if (!dirty || !editable) return;
    const timeout = window.setTimeout(() => void saveNow(), 800);
    return () => window.clearTimeout(timeout);
  }, [dirty, editable, saveNow, saveRetry]);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    const unlisten = currentWindow.onCloseRequested(async (event) => {
      if (!dirty || !editable) return;
      event.preventDefault();
      const saved = await saveNow();
      if (saved) await currentWindow.destroy();
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [dirty, editable, saveNow]);

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
    const timeout = window.setTimeout(() => void getCurrentWindow().close(), 900);
    return () => window.clearTimeout(timeout);
  }, [sendJob?.status]);

  const markDirty = () => {
    changeVersionRef.current += 1;
    setDirty(true);
  };

  async function addAttachments() {
    const selected = await open({ multiple: true, directory: false });
    const paths = typeof selected === "string" ? [selected] : selected ?? [];
    if (!paths.length) return;
    try {
      const added = await api.addDraftAttachments(sender.id, draft.id, paths);
      setAttachments((current) => [...current, ...added]);
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    }
  }

  async function removeAttachment(attachment: DraftAttachmentSummary) {
    try {
      await api.removeDraftAttachment(sender.id, draft.id, attachment.id);
      setAttachments((current) => current.filter((item) => item.id !== attachment.id));
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    }
  }

  async function sendMessage() {
    if (!subject.trim() && !confirmEmptySubject) {
      setConfirmEmptySubject(true);
      return;
    }
    setConfirmEmptySubject(false);
    const saved = await saveNow();
    if (dirty && !saved) return;
    try {
      const job = await api.queueDraftSend(sender.id, draft.id);
      setSendJob(job);
      setErrorCode(null);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    }
  }

  async function retrySend() {
    if (!sendJob) return;
    try { setSendJob(await api.retrySendJob(sender.id, sendJob.id)); }
    catch (error) { setErrorCode(normalizeCommandError(error).code); }
  }

  return (
    <AppShell className="flex min-h-0 flex-col overflow-hidden">
      <Inline className="h-14 shrink-0 border-b border-border bg-card px-3 shadow-xs">
        <Button loading={sendJob?.status === "queued" || sendJob?.status === "sending"} onClick={() => void sendMessage()} disabled={!editable || saveState === "saving"}>
          <Send size={16} />{t("composer.send")}
        </Button>
        <Button variant="ghost" onClick={() => void addAttachments()} disabled={!editable}>
          <Paperclip size={16} />{t("composer.attach")}
        </Button>
        <Page className="flex-1" />
        <Text className="text-xs" aria-live="polite">
          {saveState === "saving" ? t("composer.saving") : saveState === "failed" ? t("composer.saveFailed") : t("composer.saved")}
        </Text>
        <Button variant="ghost" size="icon" aria-label={t("common.close")} onClick={() => void getCurrentWindow().close()}>
          <X size={17} />
        </Button>
      </Inline>

      <Page className="flex min-h-0 flex-1 flex-col">
        <Inline className="min-h-11 border-b border-border bg-card px-4">
          <Text className="w-16 shrink-0 text-xs font-semibold">{t("composer.from")}</Text>
          <Text className="text-sm text-foreground">{sender.displayName || sender.email} &lt;{sender.email}&gt;</Text>
        </Inline>
        <CompactField
          label={t("composer.to")}
          value={to}
          disabled={!editable}
          placeholder={t("composer.recipientPlaceholder")}
          onChange={(event) => { setTo(event.currentTarget.value); markDirty(); }}
          trailing={
            <Button type="button" variant="ghost" size="sm" className="mr-2" onClick={() => setShowCopies((value) => !value)}>
              {t("composer.ccBcc")}<ChevronDown size={14} />
            </Button>
          }
        />
        {showCopies ? (
          <>
            <CompactField label={t("composer.cc")} value={cc} disabled={!editable} onChange={(event) => { setCc(event.currentTarget.value); markDirty(); }} />
            <CompactField label={t("composer.bcc")} value={bcc} disabled={!editable} onChange={(event) => { setBcc(event.currentTarget.value); markDirty(); }} />
          </>
        ) : null}
        <CompactField label={t("composer.subject")} value={subject} disabled={!editable} onChange={(event) => { setSubject(event.currentTarget.value); markDirty(); }} />

        {errorCode ? (
          <Alert className="m-3 mb-0" tone="danger">{t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}</Alert>
        ) : null}
        {sendJob ? <SendStatus job={sendJob} onRetry={retrySend} /> : null}
        {attachments.length ? (
          <Inline className="flex-wrap border-b border-border bg-muted/40 px-4 py-2">
            {attachments.map((attachment) => (
              <Inline key={attachment.id} className="rounded-sm border border-border bg-card px-2.5 py-1.5">
                <Paperclip size={14} /><Text className="max-w-56 truncate text-xs text-foreground">{attachment.fileName}</Text>
                <Text className="text-[11px]">{formatBytes(attachment.size)}</Text>
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
          initialJson={draft.content.editorJson}
          disabled={!editable}
          signature={{ name: sender.displayName, email: sender.email }}
          onChange={(value) => { setContent(value); markDirty(); }}
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
    </AppShell>
  );
}

function SendStatus({ job, onRetry }: { job: SendJobSummary; onRetry: () => void }) {
  const { t } = useTranslation();
  if (job.status === "sent") return <Alert className="m-3 mb-0" tone="success"><Check size={16} />{t("composer.sent")}</Alert>;
  if (job.status === "failed") return (
    <Alert className="m-3 mb-0" tone="danger" title={t("composer.sendFailed")}>
      <Inline><Text>{t(`errors.${job.errorCode}`, { defaultValue: t("composer.sendFailedDescription") })}</Text><Button size="sm" variant="secondary" onClick={onRetry}>{t("common.retry")}</Button></Inline>
    </Alert>
  );
  return <Alert className="m-3 mb-0" tone="info">{t(job.status === "sending" ? "composer.sending" : "composer.queued")}</Alert>;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

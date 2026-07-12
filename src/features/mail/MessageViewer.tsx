import { Download, FileText, MailOpen, Paperclip } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { AttachmentSummary, MessageAddress } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Modal } from "@/components/ui/dialog";
import { Inline, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { SafeMailFrame } from "./SafeMailFrame";

export function MessageViewer({ accountId, messageId }: { accountId: string; messageId: string }) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [rawSource, setRawSource] = useState<string | null>(null);
  const [remoteImagesAllowed, setRemoteImagesAllowed] = useState(false);

  useEffect(() => {
    setRemoteImagesAllowed(false);
  }, [messageId]);
  const query = useQuery({
    queryKey: ["message", accountId, messageId],
    queryFn: () => api.getMessageDetail(accountId, messageId),
    enabled: Boolean(accountId && messageId),
  });
  const attachmentMutation = useMutation({
    mutationFn: (attachmentId: string) => api.requestAttachment(accountId, attachmentId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["message", accountId, messageId] }),
  });
  const bodyMutation = useMutation({
    mutationFn: () => api.requestMessageBody(accountId, messageId),
    onSuccess: (detail) => queryClient.setQueryData(["message", accountId, messageId], detail),
  });
  const rawMutation = useMutation({
    mutationFn: () => api.requestRawMessage(accountId, messageId),
    onSuccess: setRawSource,
  });

  if (!messageId) {
    return <EmptyState icon={<MailOpen size={28} />} title={t("mail.selectMessage")} />;
  }
  if (query.isPending) {
    return <Stack className="m-auto items-center"><Spinner size={24} /></Stack>;
  }
  if (query.isError || !query.data) {
    const error = normalizeCommandError(query.error);
    return (
      <Alert className="m-5" tone="danger" title={t("errors.title")}>
        {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
      </Alert>
    );
  }
  const message = query.data;
  const operationError = bodyMutation.error ?? rawMutation.error ?? attachmentMutation.error;
  const normalizedOperationError = operationError ? normalizeCommandError(operationError) : null;
  const date = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(message.receivedAt * 1000));

  async function showRemoteImages() {
    if (message.safeHtml && /<img[^>]+src=["']https?:\/\//i.test(message.safeHtml)) {
      setRemoteImagesAllowed(true);
      return;
    }
    try {
      await bodyMutation.mutateAsync();
      setRemoteImagesAllowed(true);
    } catch {
      // The mutation error is rendered by the shared operation alert.
    }
  }

  return (
    <Stack className="min-h-0 flex-1" gap="xs">
      <Stack className="border-b border-border px-6 py-5" gap="sm">
        <Heading level={2}>{message.subject || t("mail.noSubject")}</Heading>
        <Inline className="flex-wrap">
          <LabelText>{formatAddresses(message.from)}</LabelText>
          <Text className="text-xs">{date}</Text>
          <Button
            className="ml-auto"
            variant="ghost"
            size="sm"
            loading={rawMutation.isPending}
            onClick={() => rawMutation.mutate()}
          >
            <FileText size={14} />
            {t("mail.viewSource")}
          </Button>
        </Inline>
        <Text className="text-xs">
          {t("mail.toRecipients", { recipients: formatAddresses(message.to) })}
        </Text>
        {message.remoteImagesBlocked && !remoteImagesAllowed ? (
          <Alert tone="warning" title={t("mail.remoteImagesBlocked")}>
            <Inline className="flex-wrap justify-between">
              <Text className="text-xs text-current">{t("mail.remoteImagesBlockedDescription")}</Text>
              <Button variant="secondary" size="sm" loading={bodyMutation.isPending} onClick={() => void showRemoteImages()}>
                {t("mail.showRemoteImages")}
              </Button>
            </Inline>
          </Alert>
        ) : null}
        {normalizedOperationError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${normalizedOperationError.code}`, {
              defaultValue: t("common.unexpectedError"),
            })}
          </Alert>
        ) : null}
      </Stack>

      <Stack className="min-h-0 flex-1" gap="xs">
        {message.safeHtml ? (
          <SafeMailFrame
            document={message.safeHtml}
            title={message.subject || t("mail.messageBody")}
            allowRemoteImages={remoteImagesAllowed}
          />
        ) : message.plainText ? (
          <Text className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap p-6 text-sm leading-relaxed text-foreground">
            {message.plainText}
          </Text>
        ) : (
          <EmptyState
            icon={<MailOpen size={24} />}
            title={t("mail.bodyUnavailable")}
            description={bodyMutation.isPending
              ? t("mail.downloadingBody")
              : t("mail.bodyUnavailableDescription")}
            action={(
              <Button loading={bodyMutation.isPending} onClick={() => bodyMutation.mutate()}>
                <Download size={14} />
                {t("mail.downloadBody")}
              </Button>
            )}
          />
        )}
      </Stack>

      {message.attachments.length ? (
        <Stack className="border-t border-border p-4" gap="sm">
          <Inline><Paperclip size={15} /><LabelText>{t("mail.attachments")}</LabelText></Inline>
          <Inline className="flex-wrap">
            {message.attachments.map((attachment) => (
              <AttachmentButton
                key={attachment.id}
                attachment={attachment}
                loading={attachmentMutation.isPending && attachmentMutation.variables === attachment.id}
                onClick={() => attachmentMutation.mutate(attachment.id)}
              />
            ))}
          </Inline>
        </Stack>
      ) : null}
      <Modal
        open={rawSource !== null}
        onOpenChange={(open) => { if (!open) setRawSource(null); }}
        title={t("mail.sourceTitle")}
        closeLabel={t("common.close")}
      >
        <Text className="mt-4 max-h-[65vh] overflow-auto whitespace-pre-wrap break-all rounded-sm border border-border bg-muted p-3 font-mono text-xs text-foreground">
          {rawSource ?? ""}
        </Text>
      </Modal>
    </Stack>
  );
}

function AttachmentButton({
  attachment,
  loading,
  onClick,
}: {
  attachment: AttachmentSummary;
  loading: boolean;
  onClick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Button
      variant="secondary"
      loading={loading}
      disabled={attachment.availability === "available"}
      title={attachment.availability === "available" ? t("mail.attachmentReady") : undefined}
      onClick={onClick}
    >
      <Download size={14} />
      {attachment.fileName}
      <Text className="text-[11px]">{formatBytes(attachment.size)}</Text>
      {attachment.availability === "available" ? t("mail.downloaded") : null}
    </Button>
  );
}

function formatAddresses(addresses: MessageAddress[]) {
  return addresses.map((address) => address.name || address.email).join(", ") || "—";
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

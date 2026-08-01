import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { AppShell, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { mailQueryKeys, messageQueryKeys } from "./mail-query-keys";
import { MessageViewer } from "./MessageViewer";

interface MessagePreviewLocation {
  accountId: string;
  mailboxId: string;
  messageId: string;
}

export function MessagePreviewApp(initialLocation: MessagePreviewLocation) {
  const { t } = useTranslation();
  const [location, setLocation] = useState(initialLocation);

  useEffect(() => {
    const unlisten = listen<MessagePreviewLocation>("message-preview-location-changed", (event) => {
      setLocation(event.payload);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);

  const mailboxes = useQuery({
    queryKey: mailQueryKeys.mailboxes(location.accountId),
    queryFn: () => api.listMailboxes(location.accountId),
  });
  const detail = useQuery({
    queryKey: messageQueryKeys.detail(location.accountId, location.mailboxId, location.messageId),
    queryFn: () => api.getMessageDetail(location.accountId, location.messageId, location.mailboxId),
  });
  useRevealWindowWhenReady(!mailboxes.isPending && !detail.isPending);

  if (mailboxes.isPending || detail.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (mailboxes.isError || detail.isError) {
    const error = normalizeCommandError(mailboxes.error ?? detail.error);
    return (
      <AppShell className="grid place-items-center bg-card p-8">
        <Alert tone="danger" title={t("errors.title")}>
          {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      </AppShell>
    );
  }

  return (
    <AppShell className="overflow-hidden bg-card">
      <Stack className="size-full min-h-0" gap="none">
        <MessageViewer
          accountId={location.accountId}
          mailboxId={location.mailboxId}
          messageId={location.messageId}
          mailboxes={mailboxes.data ?? []}
          allowOpenInNewWindow={false}
          onMessageRemoved={() => void getCurrentWindow().destroy()}
        />
      </Stack>
    </AppShell>
  );
}

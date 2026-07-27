import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";

interface RawMessageLocation {
  accountId: string;
  messageId: string;
}

export function RawMessageApp({ accountId, messageId }: RawMessageLocation) {
  const { t } = useTranslation();
  const [location, setLocation] = useState<RawMessageLocation>({ accountId, messageId });

  useEffect(() => {
    const unlisten = listen<RawMessageLocation>("raw-message-location-changed", (event) => {
      setLocation(event.payload);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, []);

  const rawQuery = useQuery({
    queryKey: ["raw-message", location.accountId, location.messageId],
    queryFn: () => api.requestRawMessage(location.accountId, location.messageId),
  });
  useRevealWindowWhenReady(!rawQuery.isPending);

  if (rawQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  if (rawQuery.isError) {
    const error = normalizeCommandError(rawQuery.error);
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
      <Page className="grid size-full min-h-0 grid-rows-[auto_minmax(0,1fr)] px-6 pt-5 pb-6">
        <Stack className="pb-4" gap="xs">
          <Heading level={1} className="text-xl">{t("mail.sourceTitle")}</Heading>
          <Text className="text-xs">{t("mail.sourceDescription")}</Text>
        </Stack>
        <OverlayScrollArea className="min-h-0 rounded-md bg-muted/70" viewportClassName="p-4 pr-6">
          <Text className="select-text whitespace-pre-wrap break-all font-mono text-xs leading-relaxed text-foreground">
            {rawQuery.data}
          </Text>
        </OverlayScrollArea>
      </Page>
    </AppShell>
  );
}

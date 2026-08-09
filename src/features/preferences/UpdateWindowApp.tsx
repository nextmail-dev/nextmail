import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useRevealWindowWhenReady } from "@/app/windowReady";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { IconTile } from "@/components/ui/icon-tile";
import { AppShell, Inline, Page, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Heading, LabelText, Text } from "@/components/ui/typography";
import { ReleaseNotesMarkdown } from "./ReleaseNotesMarkdown";

export function UpdateWindowApp() {
  const { t } = useTranslation();
  const updateQuery = useQuery({
    queryKey: ["available-update"],
    queryFn: api.getAvailableUpdate,
    retry: false,
  });
  const installMutation = useMutation({ mutationFn: api.installUpdate });
  useRevealWindowWhenReady(!updateQuery.isPending);

  function closeWindow() {
    if ("__TAURI_INTERNALS__" in globalThis) void getCurrentWindow().destroy();
  }

  if (updateQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={24} /></AppShell>;
  }
  const update = updateQuery.data;
  if (!update?.available || !update.version) {
    const error = normalizeCommandError(updateQuery.error);
    return (
      <AppShell className="grid place-items-center bg-card p-7">
        <EmptyState
          icon={<Download size={24} />}
          title={t("errors.title")}
          description={t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          action={<Button onClick={closeWindow}>{t("common.close")}</Button>}
        />
      </AppShell>
    );
  }
  const installError = installMutation.error ? normalizeCommandError(installMutation.error) : null;

  return (
    <AppShell className="bg-card">
      <Page className="flex h-full min-h-0 flex-col gap-5 p-7">
        <Inline className="items-start gap-4">
          <IconTile large><Download size={25} /></IconTile>
          <Stack gap="xs">
            <Heading level={1} className="text-2xl lg:text-3xl">
              {t("updates.availableTitle", { version: update.version })}
            </Heading>
            <Text>{t("updates.availableDescription")}</Text>
          </Stack>
        </Inline>
        <Stack className="min-h-0 flex-1" gap="sm">
          <LabelText>{t("updates.releaseNotes")}</LabelText>
          <OverlayScrollArea
            className="min-h-0 flex-1 rounded-lg bg-muted/60 ring-1 ring-border/60"
            viewportClassName="p-4 pr-6"
          >
            {update.notes ? (
              <ReleaseNotesMarkdown>{update.notes}</ReleaseNotesMarkdown>
            ) : (
              <Text className="text-xs">{t("updates.noReleaseNotes")}</Text>
            )}
          </OverlayScrollArea>
        </Stack>
        {installError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${installError.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
        <Inline className="justify-end">
          <Button variant="ghost" disabled={installMutation.isPending} onClick={closeWindow}>
            {t("updates.later")}
          </Button>
          <Button loading={installMutation.isPending} onClick={() => installMutation.mutate()}>
            {installMutation.isPending ? t("updates.installing") : t("updates.install")}
          </Button>
        </Inline>
      </Page>
    </AppShell>
  );
}

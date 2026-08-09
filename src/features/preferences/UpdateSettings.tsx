import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { DesktopPreferences, UpdateCheckResult } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Modal } from "@/components/ui/dialog";
import { Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { LabelText, Text } from "@/components/ui/typography";

export function UpdateSettings({
  preferences,
  onChange,
  saveError,
}: {
  preferences: DesktopPreferences;
  onChange: (preferences: DesktopPreferences) => void;
  saveError?: unknown;
}) {
  const { t } = useTranslation();
  const [availableUpdate, setAvailableUpdate] = useState<UpdateCheckResult | null>(null);
  const checkMutation = useMutation({
    mutationFn: api.checkForUpdate,
    onSuccess: (result) => {
      if (result.available) setAvailableUpdate(result);
    },
  });
  const error = checkMutation.error ? normalizeCommandError(checkMutation.error) : null;
  const preferenceError = saveError ? normalizeCommandError(saveError) : null;

  return (
    <>
      <Stack gap="md">
        <Stack gap="sm">
          <Checkbox
            checked={preferences.autoCheckUpdates}
            label={t("settings.autoCheckUpdates")}
            onCheckedChange={(autoCheckUpdates) => onChange({ ...preferences, autoCheckUpdates })}
          />
          <Text className="pl-[28px] text-xs">{t("settings.autoCheckUpdatesDescription")}</Text>
        </Stack>
        <div className="flex items-center gap-3 pl-[28px]">
          <Button
            variant="secondary"
            loading={checkMutation.isPending}
            onClick={() => checkMutation.mutate()}
          >
            {checkMutation.isPending ? t("updates.checking") : t("updates.check")}
          </Button>
          {checkMutation.data && !checkMutation.data.available ? (
            <Text className="text-xs">
              {t("updates.upToDate", { version: checkMutation.data.currentVersion })}
            </Text>
          ) : null}
          {checkMutation.data?.available ? (
            <Text className="text-xs text-primary">
              {t("updates.available", { version: checkMutation.data.version })}
            </Text>
          ) : null}
        </div>
        {error ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
        {preferenceError ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${preferenceError.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
      </Stack>
      <UpdateAvailableModal
        update={availableUpdate}
        onClose={() => setAvailableUpdate(null)}
      />
    </>
  );
}

export function StartupUpdateChecker() {
  const attempted = useRef(false);
  const [availableUpdate, setAvailableUpdate] = useState<UpdateCheckResult | null>(null);
  const desktopPreferences = useQuery({
    queryKey: ["desktop-preferences"],
    queryFn: api.getDesktopPreferences,
  });
  const bootstrap = useQuery({ queryKey: ["bootstrap"], queryFn: api.getBootstrapStatus });
  const checkMutation = useMutation({
    mutationFn: api.checkForUpdate,
    onSuccess: (result) => {
      if (result.available) setAvailableUpdate(result);
    },
  });

  useEffect(() => {
    if (
      attempted.current
      || desktopPreferences.data?.autoCheckUpdates !== true
      || bootstrap.data?.stage !== "ready"
    ) return;
    attempted.current = true;
    checkMutation.mutate();
  }, [bootstrap.data?.stage, checkMutation, desktopPreferences.data?.autoCheckUpdates]);

  return (
    <UpdateAvailableModal
      update={availableUpdate}
      onClose={() => setAvailableUpdate(null)}
    />
  );
}

function UpdateAvailableModal({
  update,
  onClose,
}: {
  update: UpdateCheckResult | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const installMutation = useMutation({ mutationFn: api.installUpdate });
  const error = installMutation.error ? normalizeCommandError(installMutation.error) : null;
  const version = update?.version ?? "";

  return (
    <Modal
      open={Boolean(update)}
      onOpenChange={(open) => { if (!open && !installMutation.isPending) onClose(); }}
      title={t("updates.availableTitle", { version })}
      closeLabel={t("common.close")}
    >
      <Stack className="pt-4" gap="lg">
        <Text>{t("updates.availableDescription")}</Text>
        <Stack gap="sm">
          <LabelText>{t("updates.releaseNotes")}</LabelText>
          {update?.notes ? (
            <OverlayScrollArea className="h-36 rounded-md bg-muted/60" viewportClassName="p-3 pr-5">
              <Text className="whitespace-pre-wrap text-xs">{update.notes}</Text>
            </OverlayScrollArea>
          ) : (
            <Text className="text-xs">{t("updates.noReleaseNotes")}</Text>
          )}
        </Stack>
        {error ? (
          <Alert tone="danger" title={t("errors.title")}>
            {t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}
          </Alert>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={installMutation.isPending} onClick={onClose}>
            {t("updates.later")}
          </Button>
          <Button loading={installMutation.isPending} onClick={() => installMutation.mutate()}>
            {installMutation.isPending ? t("updates.installing") : t("updates.install")}
          </Button>
        </div>
      </Stack>
    </Modal>
  );
}

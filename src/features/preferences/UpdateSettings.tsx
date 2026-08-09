import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { DesktopPreferences } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

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
  const checkMutation = useMutation({
    mutationFn: api.checkForUpdate,
  });
  const error = checkMutation.error ? normalizeCommandError(checkMutation.error) : null;
  const preferenceError = saveError ? normalizeCommandError(saveError) : null;

  return (
    <Stack gap="md">
      <Checkbox
        checked={preferences.autoCheckUpdates}
        label={t("settings.autoCheckUpdates")}
        description={t("settings.autoCheckUpdatesDescription")}
        onCheckedChange={(autoCheckUpdates) => onChange({ ...preferences, autoCheckUpdates })}
      />
      <div className="flex items-center gap-3 px-3">
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
  );
}

export function StartupUpdateChecker() {
  const attempted = useRef(false);
  const desktopPreferences = useQuery({
    queryKey: ["desktop-preferences"],
    queryFn: api.getDesktopPreferences,
  });
  const bootstrap = useQuery({ queryKey: ["bootstrap"], queryFn: api.getBootstrapStatus });
  const checkMutation = useMutation({
    mutationFn: api.checkForUpdate,
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

  return null;
}

import { useEffect, useState } from "react";
import { FolderCheck, FolderOpen, HardDrive } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { api, normalizeCommandError } from "@/app/api";
import type {
  AppearancePreferences,
  BootstrapStatus,
  DataDirectoryValidation,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { TextField } from "@/components/ui/input";
import { Form, Inline, Stack } from "@/components/ui/layout";
import { Eyebrow, Heading, Text } from "@/components/ui/typography";
import { OnboardingLayout } from "./OnboardingLayout";

interface DataDirectoryStepProps {
  status: BootstrapStatus;
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
  onCompleted: () => void;
}

export function DataDirectoryStep({
  status,
  preferences,
  onPreferencesChange,
  onCompleted,
}: DataDirectoryStepProps) {
  const { t } = useTranslation();
  const [path, setPath] = useState(status.configuredDataDir ?? status.defaultDataDir);
  const [validation, setValidation] = useState<DataDirectoryValidation | null>(null);
  const [checking, setChecking] = useState(false);
  const [initializing, setInitializing] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const recovering = status.stage === "data_directory_missing";

  useEffect(() => {
    setValidation(null);
    setErrorCode(null);
  }, [path]);

  async function chooseDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: path || status.defaultDataDir,
      title: t("onboarding.dataTitle"),
    });
    if (typeof selected === "string") setPath(selected);
  }

  async function validate() {
    setChecking(true);
    setErrorCode(null);
    try {
      const result = await api.validateDataDirectory(path);
      setValidation(result);
      if (!result.valid) setErrorCode(result.messageCode);
      return result;
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
      return null;
    } finally {
      setChecking(false);
    }
  }

  async function submit() {
    const result = validation?.valid ? validation : await validate();
    if (!result?.valid) return;
    setInitializing(true);
    setErrorCode(null);
    try {
      await api.initializeDataDirectory(path);
      onCompleted();
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    } finally {
      setInitializing(false);
    }
  }

  return (
    <OnboardingLayout
      activeStep={1}
      preferences={preferences}
      onPreferencesChange={onPreferencesChange}
      aside={
        <Stack className="mt-2 rounded-sm bg-primary/10 p-4 text-primary" gap="sm">
          <HardDrive size={28} aria-hidden="true" />
          <Text className="text-xs">{t("onboarding.dataHint")}</Text>
        </Stack>
      }
    >
      <Form
        className="mx-auto max-w-3xl"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Stack gap="xl">
          <Stack gap="sm">
            <Eyebrow>{t("onboarding.dataEyebrow")}</Eyebrow>
            <Heading>{recovering ? t("onboarding.missingTitle") : t("onboarding.dataTitle")}</Heading>
            <Text className="max-w-2xl text-base leading-relaxed">
              {recovering ? t("onboarding.missingDescription") : t("onboarding.dataDescription")}
            </Text>
          </Stack>

          <Surface className="rounded-md p-5">
            <Stack gap="lg">
              <TextField
                label={t("onboarding.customPath")}
                value={path}
                onChange={(event) => setPath(event.currentTarget.value)}
                spellCheck={false}
                autoComplete="off"
                trailing={
                  <Button type="button" variant="ghost" size="sm" onClick={() => void chooseDirectory()}>
                    <FolderOpen size={16} />
                    {t("common.browse")}
                  </Button>
                }
              />
              <Inline className="flex-wrap justify-between">
                <Button
                  type="button"
                  variant="secondary"
                  loading={checking}
                  onClick={() => void validate()}
                >
                  {t("onboarding.validate")}
                </Button>
                <Button type="submit" loading={initializing} disabled={!path.trim()}>
                  <FolderCheck size={17} />
                  {t("onboarding.initialize")}
                </Button>
              </Inline>
            </Stack>
          </Surface>

          {validation?.valid ? (
            <Alert tone="success">
              {validation.isExistingDataset
                ? t("onboarding.existingDataset")
                : t("onboarding.newDataset")}
            </Alert>
          ) : null}
          {errorCode ? (
            <Alert title={t("errors.title")} tone="danger">
              {t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}
            </Alert>
          ) : null}
        </Stack>
      </Form>
    </OnboardingLayout>
  );
}

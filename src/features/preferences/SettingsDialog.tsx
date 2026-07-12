import { useTranslation } from "react-i18next";

import type {
  AppearancePreferences,
  LanguagePreference,
  ThemePreference,
} from "@/app/types";
import { Modal } from "@/components/ui/dialog";
import { SelectField } from "@/components/ui/select";
import { Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  preferences: AppearancePreferences;
  onChange: (preferences: AppearancePreferences) => void;
}

export function SettingsDialog({
  open,
  onOpenChange,
  preferences,
  onChange,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={t("settings.title")}
      closeLabel={t("common.close")}
    >
      <Stack className="mt-5" gap="lg">
        <Text>{t("settings.description")}</Text>
        <SelectField
          label={t("preferences.language")}
          value={preferences.language}
          options={[
            { value: "zh-CN", label: t("preferences.chinese") },
            { value: "en-US", label: t("preferences.english") },
          ]}
          onValueChange={(language) =>
            onChange({ ...preferences, language: language as LanguagePreference })
          }
        />
        <SelectField
          label={t("preferences.theme")}
          value={preferences.theme}
          options={[
            { value: "system", label: t("preferences.system") },
            { value: "light", label: t("preferences.light") },
            { value: "dark", label: t("preferences.dark") },
          ]}
          onValueChange={(theme) =>
            onChange({ ...preferences, theme: theme as ThemePreference })
          }
        />
        <SelectField
          label={t("preferences.accent")}
          value={preferences.accentColor}
          options={[
            { value: "#2563eb", label: t("preferences.blue") },
            { value: "#7c3aed", label: t("preferences.violet") },
            { value: "#0f8a7b", label: t("preferences.teal") },
            { value: "#d13c68", label: t("preferences.rose") },
          ]}
          onValueChange={(accentColor) => onChange({ ...preferences, accentColor })}
        />
      </Stack>
    </Modal>
  );
}

import { useTranslation } from "react-i18next";

import { SelectField } from "@/components/ui/select";
import type { AppearancePreferences, LanguagePreference } from "@/app/types";

interface LanguageSwitcherProps {
  preferences: AppearancePreferences;
  onChange: (preferences: AppearancePreferences) => void;
  compact?: boolean;
}

export function LanguageSwitcher({ preferences, onChange, compact }: LanguageSwitcherProps) {
  const { t } = useTranslation();
  return (
    <SelectField
      compact={compact}
      className={compact ? "w-36" : "w-full"}
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
  );
}


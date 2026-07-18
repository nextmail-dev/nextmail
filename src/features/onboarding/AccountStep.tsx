import { ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api } from "@/app/api";
import type { AccountDraft, AppearancePreferences } from "@/app/types";
import { Stack } from "@/components/ui/layout";
import { Eyebrow, Heading, Text } from "@/components/ui/typography";
import { PasswordAccountForm } from "@/features/accounts/PasswordAccountForm";
import { OnboardingLayout } from "./OnboardingLayout";

interface AccountStepProps {
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
  onCompleted: () => void;
}

export function AccountStep({ preferences, onPreferencesChange, onCompleted }: AccountStepProps) {
  const { t } = useTranslation();

  async function submit(draft: AccountDraft) {
    await api.savePasswordAccount(draft);
    await api.completeOnboarding();
    onCompleted();
  }

  return (
    <OnboardingLayout
      activeStep={2}
      preferences={preferences}
      onPreferencesChange={onPreferencesChange}
      aside={
        <Stack className="mt-2 rounded-lg bg-primary/10 p-4 text-primary" gap="sm">
          <ShieldCheck size={28} aria-hidden="true" />
          <Text className="text-xs">{t("onboarding.privacyNote")}</Text>
        </Stack>
      }
    >
      <Stack className="mx-auto max-w-3xl pb-12" gap="xl">
        <Stack gap="sm">
          <Eyebrow>{t("onboarding.accountEyebrow")}</Eyebrow>
          <Heading>{t("onboarding.accountTitle")}</Heading>
          <Text className="max-w-2xl text-base leading-relaxed">{t("onboarding.accountDescription")}</Text>
        </Stack>
        <PasswordAccountForm submitLabel={t("onboarding.verifyAndFinish")} onSubmit={submit} />
      </Stack>
    </OnboardingLayout>
  );
}

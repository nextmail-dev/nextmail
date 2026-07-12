import { ArrowRight, Database, Mail, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { AppearancePreferences } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { IconTile } from "@/components/ui/icon-tile";
import { Inline, Stack } from "@/components/ui/layout";
import { Eyebrow, Heading, LabelText, Text } from "@/components/ui/typography";
import { OnboardingLayout } from "./OnboardingLayout";

interface WelcomeStepProps {
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
  onContinue: () => void;
}

export function WelcomeStep({
  preferences,
  onPreferencesChange,
  onContinue,
}: WelcomeStepProps) {
  const { t } = useTranslation();

  return (
    <OnboardingLayout
      activeStep={0}
      preferences={preferences}
      onPreferencesChange={onPreferencesChange}
    >
      <Stack className="mx-auto max-w-3xl py-[clamp(16px,5vh,64px)]" gap="xl">
        <Stack gap="md">
          <IconTile large>
            <Mail size={26} />
          </IconTile>
          <Eyebrow>{t("onboarding.welcomeEyebrow")}</Eyebrow>
          <Heading className="max-w-2xl text-[clamp(2rem,4vw,3.25rem)] leading-[1.06]">
            {t("onboarding.welcomeTitle")}
          </Heading>
          <Text className="max-w-2xl text-base leading-relaxed">
            {t("onboarding.welcomeDescription")}
          </Text>
        </Stack>

        <Stack className="grid grid-cols-2 gap-3.5 max-sm:grid-cols-1">
          <WelcomeFeature
            icon={<Database size={19} />}
            title={t("onboarding.welcomeLocalTitle")}
            description={t("onboarding.welcomeLocalDescription")}
          />
          <WelcomeFeature
            icon={<ShieldCheck size={19} />}
            title={t("onboarding.welcomePrivacyTitle")}
            description={t("onboarding.welcomePrivacyDescription")}
          />
        </Stack>

        <Inline className="justify-end">
          <Button size="lg" onClick={onContinue}>
            {t("onboarding.welcomeStart")}
            <ArrowRight size={18} />
          </Button>
        </Inline>
      </Stack>
    </OnboardingLayout>
  );
}

function WelcomeFeature({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <Surface className="rounded-sm p-4">
      <Stack gap="sm">
        <Inline className="text-primary">
          {icon}
          <LabelText>{title}</LabelText>
        </Inline>
        <Text className="text-xs leading-relaxed">{description}</Text>
      </Stack>
    </Surface>
  );
}

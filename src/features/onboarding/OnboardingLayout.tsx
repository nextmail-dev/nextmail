import type { PropsWithChildren, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { AppearancePreferences } from "@/app/types";
import { Surface } from "@/components/ui/card";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { ProgressSteps } from "@/components/ui/progress-steps";
import { Brand } from "@/features/preferences/Brand";
import { LanguageSwitcher } from "@/features/preferences/LanguageSwitcher";

interface OnboardingLayoutProps extends PropsWithChildren {
  activeStep: 0 | 1 | 2;
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
  aside?: ReactNode;
}

export function OnboardingLayout({
  activeStep,
  preferences,
  onPreferencesChange,
  children,
  aside,
}: OnboardingLayoutProps) {
  const { t } = useTranslation();
  return (
    <AppShell className="grid grid-cols-[minmax(260px,300px)_minmax(0,1fr)]">
      <Page className="flex min-h-0 flex-col justify-between border-r border-border bg-card/80 px-7 py-8">
        <Stack gap="xl">
          <Brand />
          <ProgressSteps
            label={t("onboarding.progressLabel")}
            items={[
              t("onboarding.stepWelcome"),
              t("onboarding.stepData"),
              t("onboarding.stepAccount"),
            ]}
            activeIndex={activeStep}
          />
          {aside}
        </Stack>
        <Surface className="mt-6 rounded-sm bg-muted p-3.5">
          <LanguageSwitcher preferences={preferences} onChange={onPreferencesChange} />
        </Surface>
      </Page>
      <Page className="min-h-0 overflow-auto px-[clamp(44px,7vw,104px)] py-[clamp(42px,7vh,76px)]">
        {children}
      </Page>
    </AppShell>
  );
}

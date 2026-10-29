import type { PropsWithChildren, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { AppearancePreferences } from "@/app/types";
import { Surface } from "@/components/ui/card";
import { AppShell, Page, Stack } from "@/components/ui/layout";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
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
    <AppShell className="-mt-[var(--titlebar-height)] grid h-[calc(100%+var(--titlebar-height))] grid-cols-[minmax(220px,240px)_minmax(0,1fr)]">
      <Page className="flex min-h-0 flex-col justify-between border-r border-border bg-sidebar px-5 pt-[calc(var(--titlebar-height)+1.5rem)] pb-6">
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
        <Surface className="mt-6 rounded-lg bg-card/75 p-3.5 shadow-none">
          <LanguageSwitcher preferences={preferences} onChange={onPreferencesChange} />
        </Surface>
      </Page>
      <div className="min-h-0 pt-[var(--titlebar-height)]">
        <OverlayScrollArea className="h-full min-h-0 bg-background">
          <Page className="mx-auto w-full max-w-5xl min-h-full px-[clamp(24px,4vw,56px)] py-8">
            {children}
          </Page>
        </OverlayScrollArea>
      </div>
    </AppShell>
  );
}

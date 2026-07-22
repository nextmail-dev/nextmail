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
    <AppShell className="-mt-[var(--titlebar-height)] grid h-[calc(100%+var(--titlebar-height))] grid-cols-[minmax(260px,300px)_minmax(0,1fr)]">
      <Page className="flex min-h-0 flex-col justify-between bg-sidebar px-7 pt-[calc(var(--titlebar-height)+2rem)] pb-8">
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
      <OverlayScrollArea alwaysVisible className="min-h-0 bg-background" viewportClassName="pr-3">
        <Page className="min-h-full px-[clamp(44px,7vw,104px)] pt-[calc(var(--titlebar-height)+clamp(42px,7vh,76px))] pb-[clamp(42px,7vh,76px)]">
          {children}
        </Page>
      </OverlayScrollArea>
    </AppShell>
  );
}

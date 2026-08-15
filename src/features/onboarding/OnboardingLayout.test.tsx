import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { OnboardingLayout } from "./OnboardingLayout";

afterEach(cleanup);

const preferences = {
  theme: "system" as const,
  accentColor: "#2563eb",
  language: "en-US" as const,
};

describe("OnboardingLayout", () => {
  it("keeps the wizard column below the fixed title bar", () => {
    const { container } = render(
      <OnboardingLayout
        activeStep={0}
        preferences={preferences}
        onPreferencesChange={() => {}}
      >
        <p>wizard content</p>
      </OnboardingLayout>,
    );

    const scrollArea = container.querySelector("[data-scrollbar-auto-hide]");
    expect(scrollArea).not.toBeNull();
    // The scroll area lives inside a padded wrapper: both content and the
    // absolute scrollbar track start below the fixed title bar instead of
    // sliding underneath it.
    expect(scrollArea!.parentElement).toHaveClass("pt-[var(--titlebar-height)]");
    expect(scrollArea!.parentElement).toHaveClass("min-h-0");
  });
});

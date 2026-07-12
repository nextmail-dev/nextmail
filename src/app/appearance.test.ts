import { afterEach, describe, expect, it } from "vitest";
import { applyAppearance, defaultPreferences } from "./appearance";

describe("appearance preferences", () => {
  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("style");
  });

  it("applies the selected theme and accent as document tokens", () => {
    applyAppearance({
      ...defaultPreferences,
      theme: "dark",
      accentColor: "#7c3aed",
    });

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#7c3aed");
    expect(document.documentElement.style.getPropertyValue("--ring")).toBe("#7c3aed");
  });
});

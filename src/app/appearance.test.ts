import { afterEach, describe, expect, it } from "vitest";
import { accessiblePrimary, applyAppearance, defaultPreferences } from "./appearance";

function relativeLuminance(hex: string) {
  return [1, 3, 5]
    .map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16) / 255)
    .map((channel) => channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4)
    .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
}

function contrast(first: string, second: string) {
  const values = [relativeLuminance(first), relativeLuminance(second)].sort((a, b) => b - a);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

describe("appearance preferences", () => {
  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("style");
  });

  it("uses the light theme before persisted preferences are available", () => {
    expect(defaultPreferences.theme).toBe("light");
  });

  it("adapts a dark-theme accent for readable surfaces and controls", () => {
    applyAppearance({
      ...defaultPreferences,
      theme: "dark",
      accentColor: "#2563eb",
    });

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement.style.getPropertyValue("--accent-color")).toBe("#2563eb");
    expect(document.documentElement.style.getPropertyValue("--primary")).toBe(accessiblePrimary("#2563eb", true));
    expect(document.documentElement.style.getPropertyValue("--primary-foreground")).toBe("#000000");
    expect(contrast(document.documentElement.style.getPropertyValue("--primary"), "#171717")).toBeGreaterThanOrEqual(4.5);
  });

  it("darkens a bright light-theme accent without changing the saved source color", () => {
    applyAppearance({
      ...defaultPreferences,
      accentColor: "#d97706",
    });

    const primary = document.documentElement.style.getPropertyValue("--primary");
    expect(document.documentElement.style.getPropertyValue("--accent-color")).toBe("#d97706");
    expect(document.documentElement.style.getPropertyValue("--primary-foreground")).toBe("#ffffff");
    expect(contrast(primary, "#fbfcfe")).toBeGreaterThanOrEqual(4.5);
    expect(contrast(primary, "#ffffff")).toBeGreaterThanOrEqual(4.5);
  });
});

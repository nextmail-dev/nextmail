import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseStyles = readFileSync(
  resolve(process.cwd(), "src/styles/base.css"),
  "utf8",
);
const themeStyles = readFileSync(
  resolve(process.cwd(), "src/styles/theme.css"),
  "utf8",
);
const composerSource = readFileSync(
  resolve(process.cwd(), "src/features/composer/ComposerApp.tsx"),
  "utf8",
);

describe("global scrollbar styles", () => {
  it("never reserves layout space for a scrollbar gutter", () => {
    expect(baseStyles).not.toContain("scrollbar-gutter");
    expect(baseStyles).toMatch(/\.native-scrollbar-hidden\s*\{[^}]*scrollbar-width:\s*none/m);
  });
});

describe("global pointer styles", () => {
  it("keeps clickable controls on the desktop default cursor", () => {
    expect(baseStyles).toMatch(/:where\([\s\S]*\)\s*\{\s*cursor:\s*default;/m);
  });
});

describe("global dialog layer", () => {
  it("stays above the draggable titlebar and takes over native drag hit testing", () => {
    expect(baseStyles).toMatch(/--layer-window-titlebar:\s*100/);
    expect(baseStyles).toMatch(/--layer-dialog-overlay:\s*200/);
    expect(baseStyles).toMatch(/--layer-dialog-content:\s*201/);
    expect(baseStyles).toMatch(/--layer-floating-content:\s*210/);
    expect(baseStyles).toMatch(/\.app-floating-content\s*\{[^}]*z-index:\s*var\(--layer-floating-content\)/m);
    expect(baseStyles).toMatch(/\.app-dialog-overlay,\s*\.app-dialog-content\s*\{[^}]*-webkit-app-region:\s*no-drag/m);
  });

  it("also covers the custom composer progress dialog", () => {
    expect(composerSource).toMatch(/className="app-dialog-overlay[^"]*"\s+role="dialog"/m);
  });
});

describe("desktop window chrome", () => {
  it("defines shared depth tokens for light, dark, and system themes", () => {
    expect(themeStyles).toMatch(/--border-strong:/);
    expect(themeStyles).toMatch(/--titlebar:/);
    expect(themeStyles).toMatch(/--window-background:\s*linear-gradient\(180deg/);
    expect(themeStyles).toMatch(/--titlebar-background:\s*linear-gradient\(180deg/);
    expect(themeStyles).toMatch(/--surface-highlight:/);
    expect(themeStyles).toMatch(/--shadow-control:/);
    expect(themeStyles).toMatch(/--shadow-overlay:/);
    expect(themeStyles).toMatch(/:root\[data-theme="dark"\]/);
    expect(themeStyles).toMatch(/:root\[data-theme="system"\]/);
  });

  it("keeps a visible, layered titlebar above the application surface", () => {
    expect(baseStyles).toMatch(/--titlebar-height:\s*42px/);
    expect(baseStyles).toMatch(/\.window-titlebar\s*\{[^}]*border-bottom:\s*1px solid var\(--titlebar-border\)/m);
    expect(baseStyles).toMatch(/\.window-titlebar\s*\{[^}]*background:\s*var\(--titlebar-background\)/m);
    expect(baseStyles).toMatch(/\.window-titlebar-title\s*\{[^}]*left:\s*50%/m);
  });
});

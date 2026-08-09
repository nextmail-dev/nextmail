import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseStyles = readFileSync(
  resolve(process.cwd(), "src/styles/base.css"),
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

describe("global dialog layer", () => {
  it("stays above the draggable titlebar and takes over native drag hit testing", () => {
    expect(baseStyles).toMatch(/--layer-window-titlebar:\s*100/);
    expect(baseStyles).toMatch(/--layer-dialog-overlay:\s*200/);
    expect(baseStyles).toMatch(/--layer-dialog-content:\s*201/);
    expect(baseStyles).toMatch(/\.app-dialog-overlay,\s*\.app-dialog-content\s*\{[^}]*-webkit-app-region:\s*no-drag/m);
  });

  it("also covers the custom composer progress dialog", () => {
    expect(composerSource).toMatch(/className="app-dialog-overlay[^"]*"\s+role="dialog"/m);
  });
});

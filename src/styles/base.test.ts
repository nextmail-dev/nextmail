import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseStyles = readFileSync(
  resolve(process.cwd(), "src/styles/base.css"),
  "utf8",
);

describe("global scrollbar styles", () => {
  it("never reserves layout space for a scrollbar gutter", () => {
    expect(baseStyles).not.toContain("scrollbar-gutter");
    expect(baseStyles).toMatch(/\.native-scrollbar-hidden\s*\{[^}]*scrollbar-width:\s*none/m);
  });
});

import { describe, expect, it } from "vitest";

import enUS from "./en-US";
import zhCN from "./zh-CN";

function translationPaths(value: unknown, prefix = ""): string[] {
  if (typeof value === "string") {
    return [prefix.replace(/_(zero|one|two|few|many|other)$/, "")];
  }
  if (!value || typeof value !== "object") return [];
  return Object.entries(value).flatMap(([key, child]) =>
    translationPaths(child, prefix ? `${prefix}.${key}` : key));
}

describe("locale resources", () => {
  it("keeps English and Chinese translation keys aligned after domain splitting", () => {
    expect([...new Set(translationPaths(zhCN))].sort()).toEqual(
      [...new Set(translationPaths(enUS))].sort(),
    );
  });
});

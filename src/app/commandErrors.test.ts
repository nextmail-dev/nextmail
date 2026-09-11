import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import i18n from "./i18n";
import { formatCommandError, normalizeCommandError } from "./commandErrors";
import enUS from "@/locales/en-US/common.json";
import zhCN from "@/locales/zh-CN/common.json";

describe("public errors", () => {
  it("preserves safe reasons in object and serialized IPC errors", () => {
    const error = { code: "storage.accounts_write_failed", retryable: false, params: { reason: "permission_denied", password: "secret", path: "C:/private" } };
    for (const input of [error, JSON.stringify(error)]) {
      const normalized = normalizeCommandError(input);
      expect(normalized.params).toEqual({ reason: "permission_denied" });
      const message = formatCommandError(i18n.getFixedT("en-US"), normalized);
      expect(message).toContain("Access was denied");
      expect(message).toContain("storage.accounts_write_failed");
      expect(message).not.toContain("secret");
      expect(message).not.toContain("C:/private");
    }
  });
  it("never renders unknown backend strings, raw errors or untrusted parameters", () => {
    for (const input of ["secret server response", new Error("secret"), { code: "secret", params: { reason: "secret" } }, null, "null"]) {
      expect(formatCommandError(i18n.getFixedT("zh-CN"), input)).not.toContain("secret");
    }
  });
  it("has bilingual text for every literal production backend error conversion", () => {
    const walk = (dir: string): string[] => fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? walk(path.join(dir, entry.name)) : [path.join(dir, entry.name)]);
    const missing = new Set<string>();
    for (const file of walk("src-tauri/src").filter((file) => file.endsWith(".rs") && !file.endsWith("tests.rs"))) {
      const source = fs.readFileSync(file, "utf8").split("#[cfg(test)]")[0];
      for (const match of source.matchAll(/(?:CommandError::(?:new|retryable|diagnosed)|command_error|lock_error|map_storage_err|map_imap_err|map_operation_err)\s*\(\s*"([a-zA-Z_]+\.[a-zA-Z_]+)"/g)) {
        for (const catalog of [enUS.errors, zhCN.errors]) {
          if (!Object.prototype.hasOwnProperty.call(catalog, match[1])) missing.add(match[1]);
        }
      }
    }
    expect([...missing]).toEqual([]);
  });
});

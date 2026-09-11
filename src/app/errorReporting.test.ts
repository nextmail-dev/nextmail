import { afterEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("frontend diagnostics", () => {
  it("deduplicates 10,000 failures and sends no payload, stack, params or console objects", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { reportCaughtError } = await import("./errorReporting");
    const error = { code: "storage.accounts_write_failed", retryable: false, params: { password: "secret", reason: "permission_denied" }, stack: "secret" };
    for (let index = 0; index < 10_000; index++) reportCaughtError("ipc", error);
    expect(invoke).toHaveBeenCalledTimes(1);
    const args = invoke.mock.calls[0][1];
    expect(JSON.parse(args.message)).toEqual({ context: "ipc", code: error.code, retryable: false });
    expect(args.location).toBeNull();
    expect(consoleError).not.toHaveBeenCalled();
    await Promise.resolve();
  });
});

import { invoke } from "@tauri-apps/api/core";
import { normalizeCommandError } from "./commandErrors";

// Never retain Error objects, stacks, arbitrary strings, command arguments or
// params: they may hold message bodies, passwords and native response payloads.
const recent = new Map<string, number>();
let installed = false;
let inFlight = 0;

export function reportCaughtError(context: string, value: unknown) {
  const error = normalizeCommandError(value);
  const category = context === "ipc" ? "ipc"
    : context === "window.error" ? "uncaught"
    : context === "window.unhandledrejection" ? "rejection" : "caught";
  const key = `${category}:${error.code}`;
  const now = Date.now();
  if (now - (recent.get(key) ?? -Infinity) < 60_000 || inFlight >= 4) return;
  if (recent.size >= 128) recent.delete(recent.keys().next().value!);
  recent.set(key, now);
  if (!("__TAURI_INTERNALS__" in globalThis)) return;
  inFlight += 1;
  void invoke("log_frontend_event", {
    level: "error",
    message: JSON.stringify({ context: category, code: error.code, retryable: error.retryable }),
    location: null,
  }).catch(() => undefined).finally(() => { inFlight -= 1; });
}

export function setupGlobalErrorReporting() {
  if (installed) return;
  installed = true;
  window.addEventListener("error", (event) => reportCaughtError("window.error", event.error));
  window.addEventListener("unhandledrejection", (event) => reportCaughtError("window.unhandledrejection", event.reason));
}

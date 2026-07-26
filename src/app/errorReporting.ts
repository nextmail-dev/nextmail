import { api } from "./api";

interface FrontendErrorDescriptor {
  message: string;
  location: string | null;
}

function describeError(value: unknown): FrontendErrorDescriptor {
  if (value instanceof Error) {
    return { message: value.message, location: value.stack ?? null };
  }
  if (typeof value === "object" && value !== null && "code" in value) {
    const commandError = value as { code?: unknown; retryable?: unknown };
    return {
      message: JSON.stringify({
        code: String(commandError.code ?? "common.unexpected_error"),
        retryable: commandError.retryable === true,
      }),
      location: null,
    };
  }
  if (typeof value === "string") {
    return { message: value, location: null };
  }
  try {
    return { message: JSON.stringify(value), location: null };
  } catch {
    return { message: String(value), location: null };
  }
}

function report(level: string, context: string, value: unknown) {
  const { message, location } = describeError(value);
  // eslint-disable-next-line no-console
  console.error("[nextmail]", level, context, message, location ?? "");
  void api.logFrontendEvent(level, `${context}: ${message}`, location).catch(() => undefined);
}

export function reportCaughtError(context: string, value: unknown) {
  report("error", context, value);
}

/**
 * Captures uncaught errors and unhandled promise rejections at the window level
 * and forwards them to the backend log file, so frontend crashes leave a
 * diagnostic trail alongside the Rust-side sync/IMAP logs.
 */
export function setupGlobalErrorReporting() {
  window.addEventListener("error", (event) => {
    report("error", "window.error", event.error ?? event.message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    report("error", "window.unhandledrejection", event.reason);
  });
}

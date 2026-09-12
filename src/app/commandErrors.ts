import type { TFunction } from "i18next";
import type { CommandError } from "./types";
import enUSErrors from "@/locales/en-US/errors";

const knownCodes = new Set(Object.keys(enUSErrors.errors));
export function normalizeCommandError(value: unknown): CommandError {
  let error = value;
  if (typeof error === "string") {
    if (knownCodes.has(error)) error = { code: error };
    else {
      try { error = JSON.parse(error); } catch { error = null; }
    }
  }
  if (typeof error !== "object" || error === null) {
    return { code: "common.unexpected_error", params: {}, retryable: false };
  }
  const candidate = error as Partial<CommandError>;
  const code = typeof candidate.code === "string" && knownCodes.has(candidate.code)
    ? candidate.code : "common.unexpected_error";
  const params: Record<string, string> = {};
  if (candidate.params && typeof candidate.params === "object") {
    for (const key of ["reason", "variable", "sendJobs", "operations"]) {
      const value = candidate.params[key];
      if (typeof value === "string" && value.length <= 80 && /^[a-zA-Z0-9_.-]+$/.test(value)) params[key] = value;
    }
  }
  return { code, params, retryable: candidate.retryable === true };
}

export function formatCommandError(t: TFunction, value: unknown): string {
  const error = normalizeCommandError(value);
  const message = t(`errors.${error.code}`, { variable: t("errorDetails.unknownVariable"), ...error.params, defaultValue: t("common.unexpectedError") });
  const reason = error.params.reason;
  const explanation = reason && Object.prototype.hasOwnProperty.call(enUSErrors.errorReasons, reason)
    ? t(`errorReasons.${reason}`) : "";
  return [message, explanation, t("errorDetails.code", { code: error.code })].filter(Boolean).join(" ");
}

import { normalizeCommandError } from "../commandErrors";
import { invoke as invokeCommand } from "../demo/session";
import { reportCaughtError } from "../errorReporting";

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invokeCommand<T>(command, args);
  } catch (error) {
    const normalized = normalizeCommandError(error);
    if (command !== "log_frontend_event") reportCaughtError("ipc", normalized);
    throw normalized;
  }
}

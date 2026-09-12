import type { AppAbout, BootstrapStatus, DataDirectoryValidation } from "../types";
import { invoke } from "./invoke";

export const lifecycleApi = {
  enterDemoMode: () => invoke<void>("enter_demo_mode"),
  getBootstrapStatus: () => invoke<BootstrapStatus>("get_bootstrap_status"),
  validateDataDirectory: (path: string) =>
    invoke<DataDirectoryValidation>("validate_data_directory", { path }),
  initializeDataDirectory: (path: string) =>
    invoke<BootstrapStatus>("initialize_data_directory", { path }),
  completeOnboarding: () => invoke<BootstrapStatus>("complete_onboarding"),
  startBackgroundServices: () => invoke<void>("start_background_services"),
  getAppAbout: () => invoke<AppAbout>("get_app_about"),
  quitApp: () => invoke<void>("quit_app"),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  openAccountManagementWindow: () => invoke<void>("open_account_management_window"),
  openRawMessageWindow: (accountId: string, messageId: string) =>
    invoke<void>("open_raw_message_window", { accountId, messageId }),
  openMessagePreviewWindow: (accountId: string, mailboxId: string, messageId: string) =>
    invoke<void>("open_message_preview_window", { accountId, mailboxId, messageId }),
  openCompositionDefinitionEditor: (
    accountId: string | null,
    kind: "template" | "signature",
    definitionId: string | null,
  ) => invoke<void>("open_composition_definition_editor_window", {
    accountId, kind, definitionId,
  }),
  logFrontendEvent: (level: string, message: string, location: string | null) =>
    invoke<void>("log_frontend_event", { level, message, location }),
};

import type {
  AppearancePreferences,
  DesktopPreferences,
  MainCloseAction,
  NewMailNotification,
  NotificationPreferences,
  ReadingPreferences,
  UpdateCheckResult,
} from "../types";
import { invoke } from "./invoke";

export const preferencesApi = {
  getPreferences: () => invoke<AppearancePreferences>("get_preferences"),
  setAppearancePreferences: (preferences: AppearancePreferences) =>
    invoke<AppearancePreferences>("set_appearance_preferences", { preferences }),
  getReadingPreferences: () => invoke<ReadingPreferences>("get_reading_preferences"),
  setReadingPreferences: (preferences: ReadingPreferences) =>
    invoke<ReadingPreferences>("set_reading_preferences", { preferences }),
  getDesktopPreferences: () => invoke<DesktopPreferences>("get_desktop_preferences"),
  setDesktopPreferences: (preferences: DesktopPreferences) =>
    invoke<DesktopPreferences>("set_desktop_preferences", { preferences }),
  getAutostartEnabled: () => invoke<boolean>("get_autostart_enabled"),
  setAutostartEnabled: (enabled: boolean) =>
    invoke<boolean>("set_autostart_enabled", { enabled }),
  resolveMainClose: (action: MainCloseAction, remember: boolean) =>
    invoke<void>("resolve_main_close", { action, remember }),
  checkForUpdate: () => invoke<UpdateCheckResult>("check_for_update"),
  getAvailableUpdate: () => invoke<UpdateCheckResult>("get_available_update"),
  installUpdate: () => invoke<void>("install_update"),
  getNotificationPreferences: () =>
    invoke<NotificationPreferences>("get_notification_preferences"),
  setNotificationPreferences: (preferences: NotificationPreferences) =>
    invoke<NotificationPreferences>("set_notification_preferences", { preferences }),
  getNewMailNotification: (notificationId: string) =>
    invoke<NewMailNotification>("get_new_mail_notification", { notificationId }),
  dismissNewMailNotification: (notificationId: string) =>
    invoke<void>("dismiss_new_mail_notification", { notificationId }),
  activateNewMailNotification: (notificationId: string) =>
    invoke<void>("activate_new_mail_notification", { notificationId }),
};

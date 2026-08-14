use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Window, WindowEvent, Wry,
};

use crate::{
    commands,
    core::{CommandError, CommandResult, LanguagePreference, MainCloseAction},
    state::AppState,
};

const TRAY_ID: &str = "nextmail-tray";
const SHOW_ID: &str = "tray-show-main";
const SETTINGS_ID: &str = "tray-settings";
const QUIT_ID: &str = "tray-quit";

struct TrayMenuItems {
    show: MenuItem<Wry>,
    settings: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let language = app.state::<AppState>().service.get_preferences()?.language;
    let labels = tray_labels(&language);
    let show = MenuItem::with_id(app, SHOW_ID, labels.show, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, labels.settings, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("NextMail")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "linux"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_main_window(app),
            SETTINGS_ID => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = commands::open_settings_window_from_tray(app).await {
                        tracing::warn!(
                            code = %error.code,
                            retryable = error.retryable,
                            "tray settings window opening failed"
                        );
                    }
                });
            }
            QUIT_ID => crate::exit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayMenuItems {
        show,
        settings,
        quit,
    });
    Ok(())
}

pub fn update_language(app: &AppHandle, language: &LanguagePreference) {
    let labels = tray_labels(language);
    let Some(items) = app.try_state::<TrayMenuItems>() else {
        return;
    };
    for (item, text) in [
        (&items.show, labels.show),
        (&items.settings, labels.settings),
        (&items.quit, labels.quit),
    ] {
        if let Err(error) = item.set_text(text) {
            tracing::warn!(?error, "tray menu localization failed");
        }
    }
}

pub fn handle_main_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    api.prevent_close();

    let app = window.app_handle();
    match app.state::<AppState>().service.get_desktop_preferences() {
        Ok(preferences) => {
            if let Some(action) = automatic_close_action(&preferences).filter(|action| {
                *action != MainCloseAction::MinimizeToTray || app.tray_by_id(TRAY_ID).is_some()
            }) {
                if let Err(error) = apply_main_close_action(app, action) {
                    tracing::warn!(
                        code = %error.code,
                        retryable = error.retryable,
                        "main close action failed"
                    );
                }
            } else if let Err(error) = app.emit_to("main", "main-close-confirmation-requested", ())
            {
                tracing::warn!(?error, "main close confirmation event failed");
            }
        }
        Err(error) => {
            tracing::warn!(
                code = %error.code,
                retryable = error.retryable,
                "desktop preferences loading failed during close"
            );
            if let Err(error) = app.emit_to("main", "main-close-confirmation-requested", ()) {
                tracing::warn!(?error, "fallback close confirmation event failed");
            }
        }
    }
}

pub fn handle_run_event(_app: &AppHandle, _event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Reopen {
        has_visible_windows: false,
        ..
    } = _event
    {
        show_main_window(_app);
    }
}

pub fn apply_main_close_action(app: &AppHandle, action: MainCloseAction) -> CommandResult<()> {
    match action {
        MainCloseAction::MinimizeToTray => {
            if app.tray_by_id(TRAY_ID).is_none() {
                return Err(CommandError::new("tray.unavailable"));
            }
            app.get_webview_window("main")
                .ok_or_else(|| CommandError::new("window.main_unavailable"))?
                .hide()
                .map_err(|_| CommandError::new("window.main_hide_failed"))
        }
        MainCloseAction::Quit => {
            crate::exit_app(app);
            Ok(())
        }
    }
}

fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
    {
        tracing::warn!(?error, "tray main window activation failed");
    }
}

fn automatic_close_action(
    preferences: &crate::core::DesktopPreferences,
) -> Option<MainCloseAction> {
    if preferences.ask_before_exit {
        None
    } else if preferences.minimize_to_tray {
        Some(MainCloseAction::MinimizeToTray)
    } else {
        Some(MainCloseAction::Quit)
    }
}

struct TrayLabels {
    show: &'static str,
    settings: &'static str,
    quit: &'static str,
}

fn tray_labels(language: &LanguagePreference) -> TrayLabels {
    match language {
        LanguagePreference::ZhCn => TrayLabels {
            show: "显示主界面",
            settings: "设置",
            quit: "退出",
        },
        LanguagePreference::EnUs => TrayLabels {
            show: "Show NextMail",
            settings: "Settings",
            quit: "Quit",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_labels_follow_interface_language() {
        assert_eq!(tray_labels(&LanguagePreference::ZhCn).show, "显示主界面");
        assert_eq!(tray_labels(&LanguagePreference::EnUs).quit, "Quit");
    }

    #[test]
    fn close_preferences_choose_prompt_hide_or_quit() {
        let mut preferences = crate::core::DesktopPreferences::default();
        assert_eq!(automatic_close_action(&preferences), None);

        preferences.ask_before_exit = false;
        assert_eq!(
            automatic_close_action(&preferences),
            Some(MainCloseAction::Quit)
        );

        preferences.minimize_to_tray = true;
        assert_eq!(
            automatic_close_action(&preferences),
            Some(MainCloseAction::MinimizeToTray)
        );
    }
}

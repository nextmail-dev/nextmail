//! Process-only demonstration session. No demo state is written to user storage.
use std::sync::Mutex;

use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem, Submenu},
    AppHandle, Emitter, Manager, State,
};

use crate::{
    core::{AppearancePreferences, LanguagePreference},
    error::{CommandError, CommandResult},
    state::AppState,
};

#[derive(Default)]
pub struct DemoSession(pub Mutex<Option<AppearancePreferences>>);

struct LanguageMenuItem(MenuItem<tauri::Wry>);

fn language_label(language: &LanguagePreference) -> &'static str {
    match language {
        LanguagePreference::ZhCn => "切换语言",
        LanguagePreference::EnUs => "Switch language",
    }
}

pub fn update_language(app: &AppHandle, language: &LanguagePreference) {
    if let Some(item) = app.try_state::<LanguageMenuItem>() {
        let _ = item.0.set_text(language_label(language));
    }
}

fn install_shortcut(app: &AppHandle, language: &LanguagePreference) -> tauri::Result<()> {
    // A native accelerator also works while focus is inside the opaque mail
    // iframe; no scripts, same-origin or keyboard bridge is added to mail HTML.
    let item = MenuItem::with_id(
        app,
        "demo-language",
        language_label(language),
        true,
        Some("CmdOrCtrl+L"),
    )?;
    let submenu = Submenu::with_items(app, "NextMail", true, &[&item])?;
    #[cfg(target_os = "macos")]
    {
        let menu = match app.menu() {
            Some(menu) => menu,
            None => Menu::new(app)?,
        };
        menu.append(&submenu)?;
        app.set_menu(menu)?;
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(main) = app.get_webview_window("main") {
        main.set_menu(Menu::with_items(app, &[&submenu])?)?;
        main.hide_menu()?;
    }
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "demo-language" && app.state::<AppState>().demo.active() {
            let _ = app.emit_to("main", "demo-language-requested", ());
        }
    });
    app.manage(LanguageMenuItem(item));
    Ok(())
}

impl DemoSession {
    pub fn active(&self) -> bool {
        self.0.lock().map(|state| state.is_some()).unwrap_or(true)
    }
}

// Deny by default: adding an ordinary application command cannot accidentally
// expose real accounts, files, credentials or send operations in demo mode.
pub fn allowed_command(command: &str) -> bool {
    matches!(
        command,
        "get_demo_status"
            | "enter_demo_mode"
            | "get_demo_content"
            | "get_preferences"
            | "set_appearance_preferences"
            | "get_app_about"
            | "quit_app"
            | "resolve_main_close"
            | "log_frontend_event"
    )
}

pub fn guard(
    handler: impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    move |invoke| {
        if invoke.message.webview().state::<AppState>().demo.active()
            && !allowed_command(invoke.message.command())
        {
            invoke
                .resolver
                .reject(CommandError::new("demo.unavailable"));
            true
        } else {
            handler(invoke)
        }
    }
}

#[tauri::command]
pub fn get_demo_status(state: State<'_, AppState>) -> CommandResult<Option<AppearancePreferences>> {
    Ok(state
        .demo
        .0
        .lock()
        .map_err(|_| CommandError::new("demo.unavailable"))?
        .clone())
}

#[tauri::command]
pub async fn enter_demo_mode(state: State<'_, AppState>, app: AppHandle) -> CommandResult<()> {
    tokio::task::yield_now().await;
    if state.demo.active() {
        return Ok(());
    }
    // Never destroy an editor containing unsaved user input.
    if app
        .webview_windows()
        .keys()
        .any(|label| label.starts_with("composer-") || label.starts_with("definition-"))
    {
        return Err(CommandError::new("demo.closeEditors"));
    }
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| CommandError::new("demo.unavailable"))?;
    let preferences = state.service.get_preferences()?;
    install_shortcut(&app, &preferences.language)
        .map_err(|_| CommandError::new("demo.unavailable"))?;
    main.hide()
        .map_err(|_| CommandError::new("demo.unavailable"))?;
    {
        let mut session = state
            .demo
            .0
            .lock()
            .map_err(|_| CommandError::new("demo.unavailable"))?;
        if session.is_none() {
            *session = Some(preferences);
        }
    }
    state.mail.stop_for_demo();
    state.composer.stop_for_demo();
    crate::tray_runtime::disable_settings_for_demo(&app);
    // Reload disposes every old query, in-flight frontend callback and selection.
    // The new WebView boot reads process state before mounting any business UI.
    main.eval("window.location.reload()")
        .map_err(|_| CommandError::new("demo.unavailable"))?;
    for (label, window) in app.webview_windows() {
        if label != "main" {
            window
                .destroy()
                .map_err(|_| CommandError::new("demo.unavailable"))?;
        }
    }
    Ok(())
}

fn content() -> CommandResult<Value> {
    let mut data: Value = serde_json::from_str(include_str!("../../src/app/demo/messages.json"))
        .map_err(|_| CommandError::new("demo.unavailable"))?;
    for letters in data
        .as_object_mut()
        .into_iter()
        .flat_map(|map| map.values_mut())
    {
        for letter in letters.as_array_mut().into_iter().flatten() {
            if let Some(html) = letter["html"].as_str() {
                let sanitized = crate::protocols::sanitize_mail_html(html);
                letter["html"] = Value::String(sanitized.document);
                letter["remoteImagesBlocked"] = Value::Bool(sanitized.remote_images_blocked);
            }
        }
    }
    Ok(data)
}

#[tauri::command]
pub fn get_demo_content(state: State<'_, AppState>) -> CommandResult<Value> {
    if !state.demo.active() {
        return Err(CommandError::new("demo.unavailable"));
    }
    content()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_process_local_and_real_commands_are_denied() {
        assert!(!DemoSession::default().active());
        for command in [
            "save_password_account",
            "list_account_summaries",
            "queue_draft_send",
            "request_attachment",
            "open_account_management_window",
            "initialize_data_directory",
            "future_command",
        ] {
            assert!(!allowed_command(command), "{command}");
        }
        assert!(allowed_command("get_demo_content"));
    }

    #[test]
    fn every_demo_html_uses_the_authoritative_sanitizer() {
        let data = content().unwrap();
        for language in ["zh-CN", "en-US"] {
            let letters = data[language].as_array().unwrap();
            assert!(letters.len() >= 10);
            assert!(letters.iter().any(|letter| letter["html"].is_null()));
            for letter in letters {
                if let Some(html) = letter["html"].as_str() {
                    assert!(!html.contains("<script"));
                    assert!(html.contains("Content-Security-Policy"));
                }
            }
        }
    }
}

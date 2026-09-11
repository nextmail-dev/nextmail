use super::*;

#[tauri::command]
pub fn get_bootstrap_status(state: State<'_, AppState>) -> CommandResult<BootstrapStatus> {
    state.service.get_bootstrap_status()
}

#[tauri::command]
pub fn validate_data_directory(
    state: State<'_, AppState>,
    path: String,
) -> DataDirectoryValidation {
    state.service.validate_data_directory(&path)
}

#[tauri::command]
pub async fn initialize_data_directory(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<BootstrapStatus> {
    state.service.initialize_data_directory(&path).await
}

#[tauri::command]
pub fn get_preferences(state: State<'_, AppState>) -> CommandResult<AppearancePreferences> {
    if let Some(preferences) = state
        .demo
        .0
        .lock()
        .map_err(|error| crate::diagnostics::lock_error("demo.unavailable", &error))?
        .clone()
    {
        return Ok(preferences);
    }
    state.service.get_preferences()
}

#[tauri::command]
pub fn set_appearance_preferences(
    state: State<'_, AppState>,
    app: AppHandle,
    preferences: AppearancePreferences,
) -> CommandResult<AppearancePreferences> {
    let preferences = {
        let mut demo = state
            .demo
            .0
            .lock()
            .map_err(|error| crate::diagnostics::lock_error("demo.unavailable", &error))?;
        if demo.is_some() {
            *demo = Some(preferences.clone());
            preferences
        } else {
            state.service.set_preferences(preferences)?
        }
    };
    update_open_window_titles(&app, &preferences.language);
    tray_runtime::update_language(&app, &preferences.language);
    crate::demo::update_language(&app, &preferences.language);
    emit_or_log(&app, "appearance-preferences-changed", &preferences);
    Ok(preferences)
}

#[tauri::command]
pub fn get_reading_preferences(state: State<'_, AppState>) -> CommandResult<ReadingPreferences> {
    state.service.get_reading_preferences()
}

#[tauri::command]
pub fn set_reading_preferences(
    state: State<'_, AppState>,
    app: AppHandle,
    preferences: ReadingPreferences,
) -> CommandResult<ReadingPreferences> {
    let preferences = state.service.set_reading_preferences(preferences)?;
    emit_or_log(&app, "reading-preferences-changed", &preferences);
    Ok(preferences)
}

#[tauri::command]
pub fn get_desktop_preferences(state: State<'_, AppState>) -> CommandResult<DesktopPreferences> {
    state.service.get_desktop_preferences()
}

#[tauri::command]
pub fn set_desktop_preferences(
    state: State<'_, AppState>,
    app: AppHandle,
    preferences: DesktopPreferences,
) -> CommandResult<DesktopPreferences> {
    let preferences = state.service.set_desktop_preferences(preferences)?;
    emit_or_log(&app, "desktop-preferences-changed", &preferences);
    Ok(preferences)
}

#[tauri::command]
pub fn resolve_main_close(
    state: State<'_, AppState>,
    app: AppHandle,
    action: MainCloseAction,
    remember: bool,
) -> CommandResult<()> {
    if remember && !state.demo.active() {
        let mut preferences = state.service.get_desktop_preferences()?;
        preferences.ask_before_exit = false;
        preferences.minimize_to_tray = action == MainCloseAction::MinimizeToTray;
        let preferences = state.service.set_desktop_preferences(preferences)?;
        emit_or_log(&app, "desktop-preferences-changed", &preferences);
    }
    tray_runtime::apply_main_close_action(&app, action)
}

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> CommandResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|error| {
        crate::diagnostics::command_error("autostart.state_read_failed", false, &error)
    })
}

#[tauri::command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> CommandResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|error| {
        crate::diagnostics::command_error("autostart.update_failed", false, &error)
    })?;
    manager.is_enabled().map_err(|error| {
        crate::diagnostics::command_error("autostart.state_read_failed", false, &error)
    })
}

#[tauri::command]
pub async fn check_for_update(
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<UpdateCheckResult> {
    let result = updater_runtime::check(&app).await?;
    if result.available {
        {
            let mut available = state.available_update.lock().map_err(|error| {
                crate::diagnostics::lock_error("update.window_create_failed", &error)
            })?;
            *available = Some(result.clone());
        }
        open_update_window_inner(&state, &app)?;
    }
    Ok(result)
}

#[tauri::command]
pub fn get_available_update(
    state: State<'_, AppState>,
    window: WebviewWindow,
) -> CommandResult<UpdateCheckResult> {
    if window.label() != "update" {
        return Err(crate::error::CommandError::new("update.not_available"));
    }
    state
        .available_update
        .lock()
        .map_err(|error| crate::diagnostics::lock_error("update.not_available", &error))?
        .clone()
        .ok_or_else(|| crate::error::CommandError::new("update.not_available"))
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> CommandResult<()> {
    updater_runtime::install(&app).await
}

fn open_update_window_inner(state: &AppState, app: &AppHandle) -> CommandResult<()> {
    if let Some(window) = app.get_webview_window("update") {
        if window.is_visible().unwrap_or(false) {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|error| {
                    crate::diagnostics::command_error("update.window_create_failed", false, &error)
                })?;
        }
        return Ok(());
    }

    let external_link_opener = std::sync::Arc::clone(&state.external_link_opener);
    let builder = WebviewWindowBuilder::new(
        app,
        "update",
        WebviewUrl::App("index.html?window=update".into()),
    )
    .title(window_title(
        &state.service.get_preferences()?.language,
        WindowTitleKind::Update,
    ))
    .inner_size(640.0, 560.0)
    .min_inner_size(520.0, 420.0)
    .center()
    .on_new_window(move |url, _features| {
        if let Err(error) =
            crate::open_external_mail_target(external_link_opener.as_ref(), url.as_str())
        {
            tracing::warn!(
                code = %error.code,
                retryable = error.retryable,
                "external update link opening failed"
            );
        }
        tauri::webview::NewWindowResponse::Deny
    })
    .visible(false);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder.build().map_err(|error| {
        crate::diagnostics::command_error("update.window_create_failed", false, &error)
    })?;
    Ok(())
}

#[tauri::command]
pub fn get_notification_preferences(
    state: State<'_, AppState>,
) -> CommandResult<NotificationPreferences> {
    state.service.get_notification_preferences()
}

#[tauri::command]
pub fn set_notification_preferences(
    state: State<'_, AppState>,
    app: AppHandle,
    preferences: NotificationPreferences,
) -> CommandResult<NotificationPreferences> {
    let preferences = state.service.set_notification_preferences(preferences)?;
    state.notifications.preferences_changed();
    emit_or_log(&app, "notification-preferences-changed", &preferences);
    Ok(preferences)
}

#[tauri::command]
pub fn get_new_mail_notification(
    state: State<'_, AppState>,
    window: WebviewWindow,
    notification_id: String,
) -> CommandResult<NewMailNotification> {
    state
        .notifications
        .bootstrap_for_window(&notification_id, window.label())
}

#[tauri::command]
pub fn dismiss_new_mail_notification(
    state: State<'_, AppState>,
    window: WebviewWindow,
    notification_id: String,
) -> CommandResult<()> {
    state
        .notifications
        .dismiss_for_window(&notification_id, window.label())
}

#[tauri::command]
pub async fn activate_new_mail_notification(
    state: State<'_, AppState>,
    app: AppHandle,
    window: WebviewWindow,
    notification_id: String,
) -> CommandResult<()> {
    let notification = state
        .notifications
        .take_for_window(&notification_id, window.label())?;
    if let Some(main) = app.get_webview_window("main") {
        if let Err(error) = main.show() {
            tracing::warn!(
                error_type = std::any::type_name_of_val(&error),
                "main window show failed"
            );
        }
        if let Err(error) = main.unminimize() {
            tracing::warn!(
                error_type = std::any::type_name_of_val(&error),
                "main window unminimize failed"
            );
        }
        if let Err(error) = main.set_focus() {
            tracing::warn!(
                error_type = std::any::type_name_of_val(&error),
                "main window focus failed"
            );
        }
        if let Some(target) = state
            .mail
            .resolve_notification_target(&notification.candidate())
            .await
        {
            if let Some(message_id) = target.message_id.as_ref() {
                if let Err(error) = state
                    .mail
                    .set_message_read(
                        &target.account_id,
                        &target.mailbox_id,
                        std::slice::from_ref(message_id),
                        true,
                    )
                    .await
                {
                    tracing::warn!(
                        code = %error.code,
                        retryable = error.retryable,
                        "notification target mark-read failed"
                    );
                }
            }
            if let Err(error) = app.emit_to("main", "open-mail-location", target) {
                tracing::warn!(
                    error_type = std::any::type_name_of_val(&error),
                    "notification navigation event failed"
                );
            }
        }
    }
    Ok(())
}

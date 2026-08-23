use super::*;

#[tauri::command]
pub fn get_app_about() -> AppAbout {
    AppAbout {
        name: "NextMail".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::exit_app(&app);
}

#[tauri::command]
pub fn log_frontend_event(level: String, message: String, location: Option<String>) {
    const MAX_FRONTEND_MESSAGE_BYTES: usize = 4_096;
    const MAX_FRONTEND_LOCATION_BYTES: usize = 16_384;
    let message = truncate_log_field(message, MAX_FRONTEND_MESSAGE_BYTES);
    let location = location.map(|value| truncate_log_field(value, MAX_FRONTEND_LOCATION_BYTES));
    match level.as_str() {
        "error" => tracing::error!(%message, location, "frontend error"),
        "warn" => tracing::warn!(%message, location, "frontend warning"),
        _ => tracing::info!(%message, location, "frontend event"),
    }
}

fn truncate_log_field(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value.push('…');
    value
}

#[tauri::command]
pub async fn open_settings_window(state: State<'_, AppState>, app: AppHandle) -> CommandResult<()> {
    open_settings_window_inner(&state, &app).await
}

pub(crate) async fn open_settings_window_from_tray(app: AppHandle) -> CommandResult<()> {
    let state = app.state::<AppState>();
    open_settings_window_inner(&state, &app).await
}

async fn open_settings_window_inner(state: &AppState, app: &AppHandle) -> CommandResult<()> {
    // Window creation must not run inside the synchronous WebView IPC callback on Windows.
    // Yielding here keeps this path aligned with the working composer-window lifecycle.
    tokio::task::yield_now().await;

    if let Some(window) = app.get_webview_window("settings") {
        if !window.is_visible().unwrap_or(false) {
            return Ok(());
        }
        window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|_| crate::error::CommandError::new("settings.window_create_failed"))?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title(window_title(
        &state.service.get_preferences()?.language,
        WindowTitleKind::Settings,
    ))
    .inner_size(900.0, 680.0)
    .min_inner_size(760.0, 560.0)
    .center()
    .visible(false);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("settings.window_create_failed"))?;
    Ok(())
}

#[tauri::command]
pub async fn open_account_management_window(
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    tokio::task::yield_now().await;

    if let Some(window) = app.get_webview_window("accounts") {
        if !window.is_visible().unwrap_or(false) {
            return Ok(());
        }
        window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|_| crate::error::CommandError::new("accounts.window_create_failed"))?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        &app,
        "accounts",
        WebviewUrl::App("index.html?window=accounts".into()),
    )
    .title(window_title(
        &state.service.get_preferences()?.language,
        WindowTitleKind::Accounts,
    ))
    .inner_size(980.0, 720.0)
    .min_inner_size(820.0, 600.0)
    .center()
    .visible(false);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("accounts.window_create_failed"))?;
    Ok(())
}

#[tauri::command]
pub async fn open_raw_message_window(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: String,
    message_id: String,
) -> CommandResult<()> {
    tokio::task::yield_now().await;
    uuid::Uuid::parse_str(&account_id)
        .map_err(|_| crate::error::CommandError::new("account.not_found"))?;
    uuid::Uuid::parse_str(&message_id)
        .map_err(|_| crate::error::CommandError::new("message.not_found"))?;
    state
        .mail
        .get_message_detail(&account_id, &message_id, None)
        .await?;
    let location = RawMessageWindowLocation {
        account_id,
        message_id,
    };

    if let Some(window) = app.get_webview_window("raw-message") {
        window
            .emit("raw-message-location-changed", &location)
            .map_err(|_| crate::error::CommandError::new("message.raw_window_create_failed"))?;
        if window.is_visible().unwrap_or(false) {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|_| crate::error::CommandError::new("message.raw_window_create_failed"))?;
        }
        return Ok(());
    }

    let url = format!(
        "index.html?window=raw-message&accountId={}&messageId={}",
        location.account_id, location.message_id
    );
    let builder = WebviewWindowBuilder::new(&app, "raw-message", WebviewUrl::App(url.into()))
        .title(window_title(
            &state.service.get_preferences()?.language,
            WindowTitleKind::RawMessage,
        ))
        .inner_size(900.0, 700.0)
        .min_inner_size(680.0, 500.0)
        .center()
        .visible(false);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("message.raw_window_create_failed"))?;
    Ok(())
}

#[tauri::command]
pub async fn open_message_preview_window(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: String,
    mailbox_id: String,
    message_id: String,
) -> CommandResult<()> {
    tokio::task::yield_now().await;
    uuid::Uuid::parse_str(&account_id)
        .map_err(|_| crate::error::CommandError::new("account.not_found"))?;
    uuid::Uuid::parse_str(&message_id)
        .map_err(|_| crate::error::CommandError::new("message.not_found"))?;
    uuid::Uuid::parse_str(&mailbox_id)
        .map_err(|_| crate::error::CommandError::new("mailbox.not_found"))?;
    let detail = state
        .mail
        .get_message_detail(&account_id, &message_id, Some(&mailbox_id))
        .await?;
    let location = MessagePreviewWindowLocation {
        account_id,
        mailbox_id,
        message_id,
    };
    let label = format!("message-preview-{}", location.message_id);

    if detail.unread {
        if let Err(error) = state
            .mail
            .set_message_read(
                &location.account_id,
                &location.mailbox_id,
                std::slice::from_ref(&location.message_id),
                true,
            )
            .await
        {
            tracing::warn!(
                account_id = %location.account_id,
                mailbox_id = %location.mailbox_id,
                message_id = %location.message_id,
                code = %error.code,
                "message preview could not mark message read"
            );
        }
    }

    if let Some(window) = app.get_webview_window(&label) {
        window
            .emit("message-preview-location-changed", &location)
            .map_err(|_| crate::error::CommandError::new("message.preview_window_create_failed"))?;
        if window.is_visible().unwrap_or(false) {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|_| {
                    crate::error::CommandError::new("message.preview_window_create_failed")
                })?;
        }
        return Ok(());
    }

    let url = format!(
        "index.html?window=message-preview&accountId={}&mailboxId={}&messageId={}",
        location.account_id, location.mailbox_id, location.message_id
    );
    let external_link_opener = std::sync::Arc::clone(&state.external_link_opener);
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(window_title(
            &state.service.get_preferences()?.language,
            WindowTitleKind::MessagePreview,
        ))
        .inner_size(980.0, 760.0)
        .min_inner_size(720.0, 520.0)
        .center()
        .on_new_window(move |url, _features| {
            if let Err(error) =
                crate::open_external_mail_target(external_link_opener.as_ref(), url.as_str())
            {
                tracing::warn!(
                    code = %error.code,
                    retryable = error.retryable,
                    "external mail link opening failed"
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
    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("message.preview_window_create_failed"))?;
    Ok(())
}

#[tauri::command]
pub async fn open_composition_definition_editor_window(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: Option<String>,
    kind: String,
    definition_id: Option<String>,
) -> CommandResult<()> {
    tokio::task::yield_now().await;
    if let Some(value) = account_id.as_deref() {
        uuid::Uuid::parse_str(value)
            .map_err(|_| crate::error::CommandError::new("account.not_found"))?;
    }
    if let Some(value) = definition_id.as_deref() {
        uuid::Uuid::parse_str(value)
            .map_err(|_| crate::error::CommandError::new("definition.not_found"))?;
    }
    let exists = match kind.as_str() {
        "template" => state
            .composer
            .list_mail_templates(account_id.as_deref())
            .await?
            .into_iter()
            .any(|value| definition_id.as_deref().is_some_and(|id| value.id == id)),
        "signature" => state
            .composer
            .list_mail_signatures(account_id.as_deref())
            .await?
            .into_iter()
            .any(|value| definition_id.as_deref().is_some_and(|id| value.id == id)),
        _ => return Err(crate::error::CommandError::new("definition.kind_invalid")),
    };
    if definition_id.is_some() && !exists {
        return Err(crate::error::CommandError::new("definition.not_found"));
    }
    let scope = account_id.as_deref().unwrap_or("global");
    let target = definition_id.as_deref().unwrap_or("new");
    let label = format!("definition-{kind}-{scope}-{target}");
    if let Some(window) = app.get_webview_window(&label) {
        if window.is_visible().unwrap_or(false) {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|_| crate::error::CommandError::new("definition.window_create_failed"))?;
        }
        return Ok(());
    }
    let mut url = format!("index.html?window=definition&kind={kind}");
    if let Some(value) = account_id.as_deref() {
        url.push_str("&accountId=");
        url.push_str(value);
    }
    if let Some(value) = definition_id.as_deref() {
        url.push_str("&definitionId=");
        url.push_str(value);
    }
    let title_kind = if kind == "template" {
        WindowTitleKind::TemplateEditor
    } else {
        WindowTitleKind::SignatureEditor
    };
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(window_title(
            &state.service.get_preferences()?.language,
            title_kind,
        ))
        .inner_size(1040.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .center()
        .visible(false);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("definition.window_create_failed"))?;
    Ok(())
}

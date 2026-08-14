mod adapters;
mod application;
mod commands;
mod composer_runtime;
pub mod core;
mod domain;
mod error;
mod logging;
mod mail_runtime;
mod notification_runtime;
pub mod protocols;
mod state;
pub mod storage;
mod tray_runtime;
mod updater_runtime;
mod window_titles;

use std::{io, sync::Arc};

use crate::core::ExternalLinkOpener;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crypto_provider();

    let app = tauri::Builder::default()
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("context-menu-policy")
                .js_init_script_on_all_frames(
                    "window.addEventListener('contextmenu', event => event.preventDefault());",
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .map_label(|label| {
                    if label.starts_with("composer-") {
                        "composer"
                    } else if label.starts_with("message-preview-") {
                        "message-preview"
                    } else if label.starts_with("definition-") {
                        "definition"
                    } else {
                        label
                    }
                })
                .with_filter(|label| !label.starts_with("notification-") && label != "update")
                .build(),
        )
        .setup(|app| {
            logging::init(app.handle());
            let state = state::AppState::from_handle(app.handle())?;
            app.manage(state);
            create_main_window(app)?;
            if let Err(error) = tray_runtime::setup(app.handle()) {
                tracing::warn!(%error, "system tray setup failed");
            }
            Ok(())
        })
        .on_window_event(tray_runtime::handle_main_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_status,
            commands::validate_data_directory,
            commands::initialize_data_directory,
            commands::get_preferences,
            commands::set_appearance_preferences,
            commands::get_reading_preferences,
            commands::set_reading_preferences,
            commands::get_desktop_preferences,
            commands::set_desktop_preferences,
            commands::resolve_main_close,
            commands::check_for_update,
            commands::get_available_update,
            commands::install_update,
            commands::get_notification_preferences,
            commands::set_notification_preferences,
            commands::get_new_mail_notification,
            commands::dismiss_new_mail_notification,
            commands::activate_new_mail_notification,
            commands::discover_account_config,
            commands::test_account_connections,
            commands::save_password_account,
            commands::add_password_account,
            commands::complete_onboarding,
            commands::start_background_services,
            commands::list_account_summaries,
            commands::get_account_connection_draft,
            commands::update_password_account,
            commands::reauthenticate_password_account,
            commands::get_account_removal_impact,
            commands::remove_account,
            commands::list_account_runtime_summaries,
            commands::get_last_selected_account,
            commands::set_last_selected_account,
            commands::get_app_about,
            commands::quit_app,
            commands::log_frontend_event,
            commands::open_settings_window,
            commands::open_account_management_window,
            commands::open_raw_message_window,
            commands::open_message_preview_window,
            commands::open_composition_definition_editor_window,
            commands::list_mailboxes,
            commands::create_mailbox,
            commands::rename_mailbox,
            commands::move_mailbox,
            commands::delete_mailbox,
            commands::mark_mailbox_all_read,
            commands::reorder_mailboxes,
            commands::list_messages,
            commands::search_messages,
            commands::get_message_detail,
            commands::list_contacts,
            commands::list_contact_suggestions,
            commands::resolve_contact_addresses,
            commands::get_contact_detail,
            commands::create_contact,
            commands::update_contact_name,
            commands::delete_contacts,
            commands::open_contact_composer,
            commands::get_sync_progress,
            commands::sync_now,
            commands::set_message_read,
            commands::set_message_flagged,
            commands::move_messages,
            commands::copy_messages,
            commands::delete_messages,
            commands::archive_messages,
            commands::set_mailbox_role_mapping,
            commands::list_pending_operation_status,
            commands::retry_pending_operation,
            commands::get_account_management_detail,
            commands::set_account_sync_interval,
            commands::set_account_download_full_messages,
            commands::request_raw_message,
            commands::request_message_body,
            commands::request_attachment,
            commands::open_message_attachment,
            commands::reveal_message_attachment,
            commands::save_message_attachment_as,
            commands::open_composer,
            commands::list_drafts,
            commands::open_existing_composer,
            commands::open_remote_draft,
            commands::open_message_action_composer,
            commands::get_composer_bootstrap,
            commands::list_mail_templates,
            commands::create_mail_template,
            commands::update_mail_template,
            commands::delete_mail_template,
            commands::list_mail_signatures,
            commands::create_mail_signature,
            commands::update_mail_signature,
            commands::delete_mail_signature,
            commands::get_signature_preferences,
            commands::save_signature_preferences,
            commands::list_composition_scene_rules,
            commands::save_composition_scene_rule,
            commands::render_mail_template,
            commands::render_mail_signature,
            commands::save_draft,
            commands::add_draft_attachments,
            commands::add_draft_inline_image,
            commands::prepare_composition_definition_image,
            commands::sanitize_rich_text_paste,
            commands::remove_draft_attachment,
            commands::discard_empty_draft,
            commands::discard_draft_session,
            commands::delete_draft,
            commands::queue_remote_draft,
            commands::queue_draft_send,
            commands::retry_send_job,
            commands::get_send_job,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application");
    app.run(tray_runtime::handle_run_event);
}

pub(crate) fn exit_app(app: &tauri::AppHandle) {
    for window in app.webview_windows().into_values() {
        if let Err(error) = window.destroy() {
            tracing::warn!(
                label = window.label(),
                ?error,
                "webview window destruction failed during exit"
            );
        }
    }
    app.exit(0);
}

fn create_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "main window config is missing"))?;
    let external_link_opener = Arc::clone(&app.state::<state::AppState>().external_link_opener);
    let window = tauri::WebviewWindowBuilder::from_config(app, config)?
        .center()
        .visible(false)
        .on_new_window(move |url, _features| {
            if let Err(error) =
                open_external_mail_target(external_link_opener.as_ref(), url.as_str())
            {
                tracing::warn!(
                    code = %error.code,
                    retryable = error.retryable,
                    "external mail link opening failed"
                );
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

pub(crate) fn open_external_mail_target(
    opener: &dyn ExternalLinkOpener,
    candidate: &str,
) -> core::CommandResult<()> {
    let validated = protocols::validate_mail_link_target(candidate)
        .ok_or_else(|| core::CommandError::new("message.link_invalid"))?;
    opener.open(&validated.target)
}

fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    assert!(
        rustls::crypto::CryptoProvider::get_default().is_some(),
        "failed to install the process-level rustls crypto provider"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::Value;

    use super::{install_crypto_provider, open_external_mail_target};
    use crate::core::{CommandResult, ExternalLinkOpener};

    #[derive(Default)]
    struct RecordingOpener {
        targets: Mutex<Vec<String>>,
    }

    impl ExternalLinkOpener for RecordingOpener {
        fn open(&self, target: &str) -> CommandResult<()> {
            self.targets.lock().unwrap().push(target.to_owned());
            Ok(())
        }
    }

    #[test]
    fn installs_process_level_rustls_crypto_provider() {
        install_crypto_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    #[test]
    fn release_csp_preserves_sanitized_mail_styles_without_weakening_scripts() {
        let config: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let security = &config["app"]["security"];
        let disabled_modifications = security["dangerousDisableAssetCspModification"]
            .as_array()
            .expect("directive-scoped CSP modification setting");
        assert_eq!(
            disabled_modifications,
            &[Value::String("style-src".to_owned())]
        );

        let csp = security["csp"].as_str().expect("configured CSP");
        let directives = csp.split(';').map(str::trim).collect::<Vec<_>>();
        assert!(directives.contains(&"style-src 'self' 'unsafe-inline'"));
        assert!(directives.contains(&"script-src 'self'"));
    }

    #[test]
    fn updater_plugin_config_deserializes_during_startup() {
        let config: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let updater_config = config["plugins"]["updater"].clone();
        let updater = serde_json::from_value::<tauri_plugin_updater::Config>(updater_config)
            .expect("updater plugin config must deserialize during app startup");

        assert_eq!(updater.endpoints.len(), 2);
    }

    #[test]
    fn external_mail_targets_are_revalidated_before_system_opening() {
        let opener = RecordingOpener::default();
        open_external_mail_target(&opener, "HTTPS://Example.COM:443/account").unwrap();
        assert_eq!(
            *opener.targets.lock().unwrap(),
            vec!["https://example.com/account"]
        );

        for unsafe_target in [
            "javascript:alert(1)",
            "file:///C:/secret.txt",
            "https://user:secret@example.com/",
        ] {
            assert!(open_external_mail_target(&opener, unsafe_target).is_err());
        }
        assert_eq!(opener.targets.lock().unwrap().len(), 1);
    }
}

mod adapters;
mod application;
mod commands;
mod composer_runtime;
pub mod core;
mod domain;
mod error;
mod mail_runtime;
pub mod protocols;
mod state;
pub mod storage;

use std::{io, sync::Arc};

use crate::core::ExternalLinkOpener;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crypto_provider();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::from_handle(app.handle())?;
            app.manage(state);
            create_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_status,
            commands::validate_data_directory,
            commands::initialize_data_directory,
            commands::get_preferences,
            commands::set_appearance_preferences,
            commands::get_reading_preferences,
            commands::set_reading_preferences,
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
            commands::open_settings_window,
            commands::list_mailboxes,
            commands::list_messages,
            commands::search_messages,
            commands::get_message_detail,
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
            commands::set_account_sync_policy,
            commands::set_download_non_inbox_bodies,
            commands::request_raw_message,
            commands::request_message_body,
            commands::request_attachment,
            commands::open_message_attachment,
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
            commands::list_composition_scene_rules,
            commands::save_composition_scene_rule,
            commands::render_mail_template,
            commands::render_mail_signature,
            commands::save_draft,
            commands::add_draft_attachments,
            commands::add_draft_inline_image,
            commands::remove_draft_attachment,
            commands::discard_empty_draft,
            commands::delete_draft,
            commands::queue_remote_draft,
            commands::queue_draft_send,
            commands::retry_send_job,
            commands::get_send_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
    tauri::WebviewWindowBuilder::from_config(app, config)?
        .on_new_window(move |url, _features| {
            let _ = open_external_mail_target(external_link_opener.as_ref(), url.as_str());
            tauri::webview::NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}

fn open_external_mail_target(
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

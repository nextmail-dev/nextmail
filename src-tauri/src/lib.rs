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

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crypto_provider();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::from_handle(app.handle())?;
            app.manage(state);
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
            commands::save_draft,
            commands::add_draft_attachments,
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
    use super::install_crypto_provider;

    #[test]
    fn installs_process_level_rustls_crypto_provider() {
        install_crypto_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}

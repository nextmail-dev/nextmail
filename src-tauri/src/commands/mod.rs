use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::{
    domain::{
        AccountConnectionDraft, AccountDraft, AccountManagementDetail, AccountRemovalImpact,
        AccountRuntimeSummary, AccountSummary, AppAbout, AppearancePreferences, AttachmentSummary,
        BootstrapStatus, ComposerBootstrap, ConnectionTestResult, DataDirectoryValidation,
        DiscoveredAccountConfig, DraftAttachmentSummary, DraftContent, DraftDetail, DraftListItem,
        DraftRecipientFields, MailSignature, MailSignatureDraft, MailTemplate, MailTemplateDraft,
        MailboxRole, MailboxSummary, MessageComposeAction, MessageDetail, MessageListPage,
        PendingOperationSummary, ReadingPreferences, SendJobSummary, SyncPolicy, SyncProgress,
    },
    error::CommandResult,
    state::AppState,
};

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
    state.service.get_preferences()
}

#[tauri::command]
pub fn set_appearance_preferences(
    state: State<'_, AppState>,
    app: AppHandle,
    preferences: AppearancePreferences,
) -> CommandResult<AppearancePreferences> {
    let preferences = state.service.set_preferences(preferences)?;
    let _ = app.emit("appearance-preferences-changed", &preferences);
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
    let _ = app.emit("reading-preferences-changed", &preferences);
    Ok(preferences)
}

#[tauri::command]
pub async fn discover_account_config(
    state: State<'_, AppState>,
    email: String,
) -> CommandResult<DiscoveredAccountConfig> {
    state.service.discover_account_config(&email).await
}

#[tauri::command]
pub async fn test_account_connections(
    state: State<'_, AppState>,
    draft: AccountDraft,
) -> CommandResult<ConnectionTestResult> {
    state.service.test_account_connections(&draft).await
}

#[tauri::command]
pub async fn save_password_account(
    state: State<'_, AppState>,
    app: AppHandle,
    draft: AccountDraft,
) -> CommandResult<AccountSummary> {
    add_account(&state, &app, draft).await
}

#[tauri::command]
pub async fn add_password_account(
    state: State<'_, AppState>,
    app: AppHandle,
    draft: AccountDraft,
) -> CommandResult<AccountSummary> {
    add_account(&state, &app, draft).await
}

async fn add_account(
    state: &State<'_, AppState>,
    app: &AppHandle,
    draft: AccountDraft,
) -> CommandResult<AccountSummary> {
    let account = state.service.add_password_account(draft).await?;
    state.mail.reconcile_accounts();
    emit_accounts_changed(app, state.service.accounts_revision()?);
    Ok(account)
}

#[tauri::command]
pub fn complete_onboarding(state: State<'_, AppState>) -> CommandResult<BootstrapStatus> {
    state.service.complete_onboarding()
}

#[tauri::command]
pub async fn start_background_services(state: State<'_, AppState>) -> CommandResult<()> {
    let _ = state.service.retry_pending_credential_cleanup().await;
    state.mail.start();
    state.composer.start();
    Ok(())
}

#[tauri::command]
pub fn list_account_summaries(state: State<'_, AppState>) -> CommandResult<Vec<AccountSummary>> {
    state.service.list_account_summaries()
}

#[tauri::command]
pub fn get_account_connection_draft(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<AccountConnectionDraft> {
    state.service.get_account_connection_draft(&account_id)
}

#[tauri::command]
pub async fn update_password_account(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: String,
    draft: AccountConnectionDraft,
    new_password: Option<String>,
) -> CommandResult<AccountSummary> {
    let summary = state
        .service
        .update_password_account(&account_id, draft, new_password)
        .await?;
    state.mail.restart_account(&account_id);
    emit_accounts_changed(&app, state.service.accounts_revision()?);
    Ok(summary)
}

#[tauri::command]
pub async fn reauthenticate_password_account(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: String,
    password: String,
) -> CommandResult<AccountSummary> {
    let summary = state
        .service
        .reauthenticate_password_account(&account_id, password)
        .await?;
    state.mail.restart_account(&account_id);
    emit_accounts_changed(&app, state.service.accounts_revision()?);
    Ok(summary)
}

#[tauri::command]
pub async fn get_account_removal_impact(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<AccountRemovalImpact> {
    state.mail.get_account_removal_impact(&account_id).await
}

#[tauri::command]
pub async fn remove_account(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: String,
) -> CommandResult<()> {
    state.service.account_record(&account_id)?;
    let _ = app.emit(
        "account-removing",
        AccountRemovingEvent {
            account_id: account_id.clone(),
        },
    );
    state.mail.begin_remove_account(&account_id);
    let impact = match state.mail.get_account_removal_impact(&account_id).await {
        Ok(impact) => impact,
        Err(error) => {
            state.mail.restart_account(&account_id);
            return Err(error);
        }
    };
    if !impact.can_remove {
        state.mail.restart_account(&account_id);
        return Err(crate::error::CommandError::new("account.remove_blocked")
            .with_param("sendJobs", impact.queued_send_jobs.to_string())
            .with_param("operations", impact.pending_operations.to_string()));
    }
    if let Err(error) = state.composer.close_account_windows(&account_id).await {
        state.mail.restart_account(&account_id);
        return Err(error);
    }
    let revision = match state.service.remove_account(&account_id).await {
        Ok(revision) => revision,
        Err(error) => {
            state.mail.restart_account(&account_id);
            return Err(error);
        }
    };
    state.mail.reconcile_accounts();
    emit_accounts_changed(&app, revision);
    Ok(())
}

#[tauri::command]
pub fn list_account_runtime_summaries(state: State<'_, AppState>) -> Vec<AccountRuntimeSummary> {
    state.mail.list_account_runtime_summaries()
}

#[tauri::command]
pub fn get_last_selected_account(state: State<'_, AppState>) -> CommandResult<Option<String>> {
    state.service.last_selected_account_id()
}

#[tauri::command]
pub async fn set_last_selected_account(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<String> {
    state.service.set_last_selected_account(&account_id).await
}

#[tauri::command]
pub fn get_app_about() -> AppAbout {
    AppAbout {
        name: "NextMail".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> CommandResult<()> {
    // Window creation must not run inside the synchronous WebView IPC callback on Windows.
    // Yielding here keeps this path aligned with the working composer-window lifecycle.
    tokio::task::yield_now().await;

    if let Some(window) = app.get_webview_window("settings") {
        window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|_| crate::error::CommandError::new("settings.window_create_failed"))?;
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("NextMail Settings")
    .inner_size(900.0, 680.0)
    .min_inner_size(760.0, 560.0);
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(12.0, 11.0));

    builder
        .build()
        .map_err(|_| crate::error::CommandError::new("settings.window_create_failed"))?;
    Ok(())
}

#[tauri::command]
pub async fn list_mailboxes(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<Vec<MailboxSummary>> {
    state.mail.list_mailboxes(&account_id).await
}

#[tauri::command]
pub async fn list_messages(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<MessageListPage> {
    state
        .mail
        .list_messages(&account_id, &mailbox_id, cursor.as_deref(), limit)
        .await
}

#[tauri::command]
pub async fn get_message_detail(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    mailbox_id: Option<String>,
) -> CommandResult<MessageDetail> {
    state
        .mail
        .get_message_detail(&account_id, &message_id, mailbox_id.as_deref())
        .await
}

#[tauri::command]
pub fn get_sync_progress(state: State<'_, AppState>, account_id: String) -> SyncProgress {
    state.mail.get_sync_progress(&account_id)
}

#[tauri::command]
pub fn sync_now(state: State<'_, AppState>, account_id: String) -> CommandResult<()> {
    state.mail.sync_now(&account_id)
}

#[tauri::command]
pub async fn set_message_read(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    message_ids: Vec<String>,
    read: bool,
) -> CommandResult<()> {
    state
        .mail
        .set_message_read(&account_id, &mailbox_id, &message_ids, read)
        .await
}

#[tauri::command]
pub async fn set_message_flagged(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    message_ids: Vec<String>,
    flagged: bool,
) -> CommandResult<()> {
    state
        .mail
        .set_message_flagged(&account_id, &mailbox_id, &message_ids, flagged)
        .await
}

#[tauri::command]
pub async fn move_messages(
    state: State<'_, AppState>,
    account_id: String,
    source_mailbox_id: String,
    destination_mailbox_id: String,
    message_ids: Vec<String>,
) -> CommandResult<()> {
    state
        .mail
        .transfer_messages(
            &account_id,
            &source_mailbox_id,
            &destination_mailbox_id,
            &message_ids,
            false,
        )
        .await
}

#[tauri::command]
pub async fn copy_messages(
    state: State<'_, AppState>,
    account_id: String,
    source_mailbox_id: String,
    destination_mailbox_id: String,
    message_ids: Vec<String>,
) -> CommandResult<()> {
    state
        .mail
        .transfer_messages(
            &account_id,
            &source_mailbox_id,
            &destination_mailbox_id,
            &message_ids,
            true,
        )
        .await
}

#[tauri::command]
pub async fn delete_messages(
    state: State<'_, AppState>,
    account_id: String,
    source_mailbox_id: String,
    message_ids: Vec<String>,
) -> CommandResult<()> {
    state
        .mail
        .delete_messages(&account_id, &source_mailbox_id, &message_ids)
        .await
}

#[tauri::command]
pub async fn archive_messages(
    state: State<'_, AppState>,
    account_id: String,
    source_mailbox_id: String,
    message_ids: Vec<String>,
) -> CommandResult<()> {
    state
        .mail
        .archive_messages(&account_id, &source_mailbox_id, &message_ids)
        .await
}

#[tauri::command]
pub async fn set_mailbox_role_mapping(
    state: State<'_, AppState>,
    account_id: String,
    role: MailboxRole,
    mailbox_id: Option<String>,
) -> CommandResult<()> {
    state
        .mail
        .set_mailbox_role_mapping(&account_id, role, mailbox_id.as_deref())
        .await
}

#[tauri::command]
pub async fn list_pending_operation_status(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<Vec<PendingOperationSummary>> {
    state.mail.list_pending_operation_status(&account_id).await
}

#[tauri::command]
pub async fn retry_pending_operation(
    state: State<'_, AppState>,
    account_id: String,
    operation_id: String,
) -> CommandResult<()> {
    state
        .mail
        .retry_pending_operation(&account_id, &operation_id)
        .await
}

#[tauri::command]
pub async fn get_account_management_detail(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<AccountManagementDetail> {
    state.mail.get_account_management_detail(&account_id).await
}

#[tauri::command]
pub async fn set_account_sync_policy(
    state: State<'_, AppState>,
    account_id: String,
    sync_policy: SyncPolicy,
) -> CommandResult<SyncPolicy> {
    state
        .mail
        .set_account_sync_policy(&account_id, sync_policy)
        .await
}

#[tauri::command]
pub async fn request_raw_message(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
) -> CommandResult<String> {
    state
        .mail
        .request_raw_message(&account_id, &message_id)
        .await
}

#[tauri::command]
pub async fn request_message_body(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    mailbox_id: Option<String>,
) -> CommandResult<MessageDetail> {
    state
        .mail
        .request_message_body(&account_id, &message_id, mailbox_id.as_deref())
        .await
}

#[tauri::command]
pub async fn request_attachment(
    state: State<'_, AppState>,
    account_id: String,
    attachment_id: String,
) -> CommandResult<AttachmentSummary> {
    state
        .mail
        .request_attachment(&account_id, &attachment_id)
        .await
}

#[tauri::command]
pub async fn open_message_attachment(
    state: State<'_, AppState>,
    account_id: String,
    attachment_id: String,
) -> CommandResult<()> {
    state
        .mail
        .open_message_attachment(&account_id, &attachment_id)
        .await
}

#[tauri::command]
pub async fn save_message_attachment_as(
    state: State<'_, AppState>,
    account_id: String,
    attachment_id: String,
) -> CommandResult<bool> {
    state
        .mail
        .save_message_attachment_as(&account_id, &attachment_id)
        .await
}

#[tauri::command]
pub async fn open_composer(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<String> {
    state.composer.open_composer(&account_id).await
}

#[tauri::command]
pub async fn list_drafts(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<Vec<DraftListItem>> {
    state.composer.list_drafts(&account_id).await
}

#[tauri::command]
pub async fn open_existing_composer(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<()> {
    state
        .composer
        .open_existing_composer(&account_id, &draft_id)
        .await
}

#[tauri::command]
pub async fn open_remote_draft(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
) -> CommandResult<()> {
    state
        .composer
        .open_remote_draft(&account_id, &message_id)
        .await
}

#[tauri::command]
pub async fn open_message_action_composer(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
    action: MessageComposeAction,
) -> CommandResult<()> {
    state
        .composer
        .open_message_action_composer(&account_id, &message_id, action)
        .await
}

#[tauri::command]
pub async fn get_composer_bootstrap(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<ComposerBootstrap> {
    state.composer.get_bootstrap(&account_id, &draft_id).await
}

#[tauri::command]
pub async fn list_mail_templates(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> CommandResult<Vec<MailTemplate>> {
    state
        .composer
        .list_mail_templates(account_id.as_deref())
        .await
}

#[tauri::command]
pub async fn create_mail_template(
    state: State<'_, AppState>,
    account_id: Option<String>,
    draft: MailTemplateDraft,
) -> CommandResult<MailTemplate> {
    state
        .composer
        .create_mail_template(account_id.as_deref(), draft)
        .await
}

#[tauri::command]
pub async fn update_mail_template(
    state: State<'_, AppState>,
    account_id: Option<String>,
    template_id: String,
    draft: MailTemplateDraft,
    expected_revision: u64,
) -> CommandResult<MailTemplate> {
    state
        .composer
        .update_mail_template(
            account_id.as_deref(),
            &template_id,
            draft,
            expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn delete_mail_template(
    state: State<'_, AppState>,
    account_id: Option<String>,
    template_id: String,
    expected_revision: u64,
) -> CommandResult<()> {
    state
        .composer
        .delete_mail_template(account_id.as_deref(), &template_id, expected_revision)
        .await
}

#[tauri::command]
pub async fn list_mail_signatures(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> CommandResult<Vec<MailSignature>> {
    state
        .composer
        .list_mail_signatures(account_id.as_deref())
        .await
}

#[tauri::command]
pub async fn create_mail_signature(
    state: State<'_, AppState>,
    account_id: Option<String>,
    draft: MailSignatureDraft,
) -> CommandResult<MailSignature> {
    state
        .composer
        .create_mail_signature(account_id.as_deref(), draft)
        .await
}

#[tauri::command]
pub async fn update_mail_signature(
    state: State<'_, AppState>,
    account_id: Option<String>,
    signature_id: String,
    draft: MailSignatureDraft,
    expected_revision: u64,
) -> CommandResult<MailSignature> {
    state
        .composer
        .update_mail_signature(
            account_id.as_deref(),
            &signature_id,
            draft,
            expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn delete_mail_signature(
    state: State<'_, AppState>,
    account_id: Option<String>,
    signature_id: String,
    expected_revision: u64,
) -> CommandResult<()> {
    state
        .composer
        .delete_mail_signature(account_id.as_deref(), &signature_id, expected_revision)
        .await
}

#[tauri::command]
pub async fn save_draft(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
    recipients: DraftRecipientFields,
    subject: String,
    content: DraftContent,
    expected_revision: u64,
) -> CommandResult<DraftDetail> {
    state
        .composer
        .save_draft(
            &account_id,
            &draft_id,
            recipients,
            subject,
            content,
            expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn add_draft_attachments(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
    selected_paths: Vec<String>,
) -> CommandResult<Vec<DraftAttachmentSummary>> {
    state
        .composer
        .add_attachments(&account_id, &draft_id, selected_paths)
        .await
}

#[tauri::command]
pub async fn remove_draft_attachment(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
    attachment_id: String,
) -> CommandResult<()> {
    state
        .composer
        .remove_attachment(&account_id, &draft_id, &attachment_id)
        .await
}

#[tauri::command]
pub async fn discard_empty_draft(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<()> {
    state
        .composer
        .discard_empty_draft(&account_id, &draft_id)
        .await
}

#[tauri::command]
pub async fn delete_draft(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<()> {
    state.composer.delete_draft(&account_id, &draft_id).await
}

#[tauri::command]
pub async fn queue_remote_draft(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<()> {
    state
        .composer
        .queue_remote_draft(&account_id, &draft_id)
        .await
}

#[tauri::command]
pub async fn queue_draft_send(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<SendJobSummary> {
    state.composer.queue_send(&account_id, &draft_id).await
}

#[tauri::command]
pub async fn retry_send_job(
    state: State<'_, AppState>,
    account_id: String,
    send_job_id: String,
) -> CommandResult<SendJobSummary> {
    state.composer.retry_send(&account_id, &send_job_id).await
}

#[tauri::command]
pub async fn get_send_job(
    state: State<'_, AppState>,
    account_id: String,
    send_job_id: String,
) -> CommandResult<SendJobSummary> {
    state.composer.get_send_job(&account_id, &send_job_id).await
}

fn emit_accounts_changed(app: &AppHandle, revision: u64) {
    let _ = app.emit("accounts-changed", AccountsChangedEvent { revision });
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountsChangedEvent {
    revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountRemovingEvent {
    account_id: String,
}

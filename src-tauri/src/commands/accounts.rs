use super::*;

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
    if let Err(error) = state.service.retry_pending_credential_cleanup().await {
        tracing::warn!(
            code = %error.code,
            retryable = error.retryable,
            "pending credential cleanup failed"
        );
    }
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
    emit_or_log(
        &app,
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
    state.notifications.dismiss_account(&account_id);
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
pub async fn set_last_selected_mailbox(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
) -> CommandResult<String> {
    state
        .service
        .set_last_selected_mailbox(&account_id, &mailbox_id)
        .await
}

use super::*;

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
    app: AppHandle,
    account_id: Option<String>,
    draft: MailTemplateDraft,
) -> CommandResult<MailTemplate> {
    let template = state
        .composer
        .create_mail_template(account_id.as_deref(), draft)
        .await?;
    emit_composition_definitions_changed(&app, account_id, "template");
    Ok(template)
}

#[tauri::command]
pub async fn update_mail_template(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: Option<String>,
    template_id: String,
    draft: MailTemplateDraft,
    expected_revision: u64,
) -> CommandResult<MailTemplate> {
    let template = state
        .composer
        .update_mail_template(
            account_id.as_deref(),
            &template_id,
            draft,
            expected_revision,
        )
        .await?;
    emit_composition_definitions_changed(&app, account_id, "template");
    Ok(template)
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
    app: AppHandle,
    account_id: Option<String>,
    draft: MailSignatureDraft,
) -> CommandResult<MailSignature> {
    let signature = state
        .composer
        .create_mail_signature(account_id.as_deref(), draft)
        .await?;
    emit_composition_definitions_changed(&app, account_id, "signature");
    Ok(signature)
}

#[tauri::command]
pub async fn update_mail_signature(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: Option<String>,
    signature_id: String,
    draft: MailSignatureDraft,
    expected_revision: u64,
) -> CommandResult<MailSignature> {
    let signature = state
        .composer
        .update_mail_signature(
            account_id.as_deref(),
            &signature_id,
            draft,
            expected_revision,
        )
        .await?;
    emit_composition_definitions_changed(&app, account_id, "signature");
    Ok(signature)
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
pub async fn get_signature_preferences(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> CommandResult<SignaturePreferences> {
    state
        .composer
        .get_signature_preferences(account_id.as_deref())
        .await
}

#[tauri::command]
pub async fn save_signature_preferences(
    state: State<'_, AppState>,
    account_id: Option<String>,
    draft: SignaturePreferencesDraft,
    expected_revision: u64,
) -> CommandResult<SignaturePreferences> {
    state
        .composer
        .save_signature_preferences(account_id.as_deref(), draft, expected_revision)
        .await
}

#[tauri::command]
pub async fn list_composition_scene_rules(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> CommandResult<Vec<CompositionSceneRule>> {
    state
        .composer
        .list_composition_scene_rules(account_id.as_deref())
        .await
}

#[tauri::command]
pub async fn save_composition_scene_rule(
    state: State<'_, AppState>,
    account_id: Option<String>,
    draft: CompositionSceneRuleDraft,
    expected_revision: u64,
) -> CommandResult<CompositionSceneRule> {
    state
        .composer
        .save_composition_scene_rule(account_id.as_deref(), draft, expected_revision)
        .await
}

#[tauri::command]
pub async fn render_mail_template(
    state: State<'_, AppState>,
    account_id: String,
    template_id: String,
    recipients: DraftRecipientFields,
) -> CommandResult<RenderedMailTemplate> {
    state
        .composer
        .render_mail_template(&account_id, &template_id, recipients)
        .await
}

#[tauri::command]
pub async fn render_mail_signature(
    state: State<'_, AppState>,
    account_id: String,
    signature_id: String,
    recipients: DraftRecipientFields,
) -> CommandResult<RenderedMailSignature> {
    state
        .composer
        .render_mail_signature(&account_id, &signature_id, recipients)
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
pub async fn add_draft_inline_image(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
    file_name: String,
    content_type: String,
    content_base64: String,
) -> CommandResult<DraftAttachmentSummary> {
    state
        .composer
        .add_inline_image(
            &account_id,
            &draft_id,
            file_name,
            content_type,
            content_base64,
        )
        .await
}

#[tauri::command]
pub fn sanitize_rich_text_paste(state: State<'_, AppState>, html: String) -> CommandResult<String> {
    state.composer.sanitize_rich_text_paste(&html)
}

#[tauri::command]
pub fn prepare_composition_definition_image(
    state: State<'_, AppState>,
    file_name: String,
    content_type: String,
    content_base64: String,
) -> CommandResult<PreparedInlineImage> {
    state
        .composer
        .prepare_definition_inline_image(file_name, content_type, content_base64)
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
) -> CommandResult<bool> {
    state
        .composer
        .discard_empty_draft(&account_id, &draft_id)
        .await
}

#[tauri::command]
pub async fn discard_draft_session(
    state: State<'_, AppState>,
    account_id: String,
    draft_id: String,
) -> CommandResult<()> {
    state
        .composer
        .discard_draft_session(&account_id, &draft_id)
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

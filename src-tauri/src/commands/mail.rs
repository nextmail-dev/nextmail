use super::*;

#[tauri::command]
pub async fn list_mailboxes(
    state: State<'_, AppState>,
    account_id: String,
) -> CommandResult<Vec<MailboxSummary>> {
    state.mail.list_mailboxes(&account_id).await
}

#[tauri::command]
pub async fn create_mailbox(
    state: State<'_, AppState>,
    account_id: String,
    parent_mailbox_id: Option<String>,
    name: String,
) -> CommandResult<()> {
    state
        .mail
        .create_mailbox(&account_id, parent_mailbox_id.as_deref(), &name)
        .await
}

#[tauri::command]
pub async fn rename_mailbox(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    name: String,
) -> CommandResult<()> {
    state
        .mail
        .rename_mailbox(&account_id, &mailbox_id, &name)
        .await
}

#[tauri::command]
pub async fn move_mailbox(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    destination_parent_mailbox_id: Option<String>,
) -> CommandResult<()> {
    state
        .mail
        .move_mailbox(
            &account_id,
            &mailbox_id,
            destination_parent_mailbox_id.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn delete_mailbox(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
) -> CommandResult<()> {
    state.mail.delete_mailbox(&account_id, &mailbox_id).await
}

#[tauri::command]
pub async fn mark_mailbox_all_read(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
) -> CommandResult<()> {
    state
        .mail
        .mark_mailbox_all_read(&account_id, &mailbox_id)
        .await
}

#[tauri::command]
pub async fn set_mailbox_favorite(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: String,
    favorite: bool,
) -> CommandResult<()> {
    state
        .mail
        .set_mailbox_favorite(&account_id, &mailbox_id, favorite)
        .await
}

#[tauri::command]
pub async fn reorder_mailboxes(
    state: State<'_, AppState>,
    account_id: String,
    ordered_mailbox_ids: Vec<String>,
) -> CommandResult<()> {
    state
        .mail
        .reorder_mailboxes(&account_id, &ordered_mailbox_ids)
        .await
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
pub async fn list_unread_messages(
    state: State<'_, AppState>,
    account_id: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<MessageListPage> {
    state
        .mail
        .list_unread_messages(&account_id, cursor.as_deref(), limit)
        .await
}

#[tauri::command]
pub async fn list_starred_messages(
    state: State<'_, AppState>,
    account_id: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<MessageListPage> {
    state
        .mail
        .list_starred_messages(&account_id, cursor.as_deref(), limit)
        .await
}

#[tauri::command]
pub async fn search_messages(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_id: Option<String>,
    query: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<MessageListPage> {
    state
        .mail
        .search_messages(
            &account_id,
            mailbox_id.as_deref(),
            &query,
            cursor.as_deref(),
            limit,
        )
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
pub async fn list_contacts(
    state: State<'_, AppState>,
    account_id: String,
    query: String,
    cursor: Option<String>,
    limit: u32,
) -> CommandResult<ContactListPage> {
    state
        .mail
        .list_contacts(&account_id, &query, cursor.as_deref(), limit)
        .await
}

#[tauri::command]
pub async fn list_contact_suggestions(
    state: State<'_, AppState>,
    account_id: String,
    query: String,
    limit: u32,
) -> CommandResult<Vec<ContactSummary>> {
    state
        .mail
        .list_contact_suggestions(&account_id, &query, limit)
        .await
}

#[tauri::command]
pub async fn resolve_contact_addresses(
    state: State<'_, AppState>,
    account_id: String,
    addresses: Vec<MessageAddress>,
) -> CommandResult<Vec<AddressPresentation>> {
    state
        .mail
        .resolve_contact_addresses(&account_id, &addresses)
        .await
}

#[tauri::command]
pub async fn get_contact_detail(
    state: State<'_, AppState>,
    account_id: String,
    contact_id: String,
) -> CommandResult<ContactDetail> {
    state
        .mail
        .get_contact_detail(&account_id, &contact_id)
        .await
}

#[tauri::command]
pub async fn get_contact_summary(
    state: State<'_, AppState>,
    account_id: String,
    contact_id: String,
) -> CommandResult<ContactSummary> {
    state
        .mail
        .get_contact_summary(&account_id, &contact_id)
        .await
}

#[tauri::command]
pub async fn create_contact(
    state: State<'_, AppState>,
    account_id: String,
    draft: ContactDraft,
) -> CommandResult<ContactSummary> {
    state.mail.create_contact(&account_id, &draft).await
}

#[tauri::command]
pub async fn update_contact_name(
    state: State<'_, AppState>,
    account_id: String,
    contact_id: String,
    name: String,
    expected_revision: u64,
) -> CommandResult<ContactSummary> {
    state
        .mail
        .update_contact_name(&account_id, &contact_id, &name, expected_revision)
        .await
}

#[tauri::command]
pub async fn delete_contacts(
    state: State<'_, AppState>,
    account_id: String,
    contact_ids: Vec<String>,
) -> CommandResult<()> {
    state.mail.delete_contacts(&account_id, &contact_ids).await
}

#[tauri::command]
pub async fn open_contact_composer(
    state: State<'_, AppState>,
    account_id: String,
    contact_id: String,
) -> CommandResult<String> {
    let contact = state
        .mail
        .get_contact_detail(&account_id, &contact_id)
        .await?
        .contact;
    state
        .composer
        .open_composer_to_contact(
            &account_id,
            crate::domain::MessageAddress {
                name: Some(contact.name),
                email: contact.email,
            },
        )
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
pub async fn set_account_sync_interval(
    state: State<'_, AppState>,
    account_id: String,
    sync_interval: SyncInterval,
) -> CommandResult<SyncInterval> {
    state
        .mail
        .set_account_sync_interval(&account_id, sync_interval)
        .await
}

#[tauri::command]
pub async fn set_account_download_full_messages(
    state: State<'_, AppState>,
    account_id: String,
    enabled: bool,
) -> CommandResult<bool> {
    state
        .mail
        .set_account_download_full_messages(&account_id, enabled)
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
pub async fn save_message_as(
    state: State<'_, AppState>,
    account_id: String,
    message_id: String,
) -> CommandResult<bool> {
    state.mail.save_message_as(&account_id, &message_id).await
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
        .request_message_body_with_progress(&account_id, &message_id, mailbox_id.as_deref())
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
pub async fn reveal_message_attachment(
    state: State<'_, AppState>,
    account_id: String,
    attachment_id: String,
) -> CommandResult<()> {
    state
        .mail
        .reveal_message_attachment(&account_id, &attachment_id)
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

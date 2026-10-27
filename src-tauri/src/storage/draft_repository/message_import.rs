use super::*;

impl DraftRepository {
    pub async fn ensure_import_source(
        &self,
        slot: &str,
        mailbox: &str,
        message: &str,
        role: &MailboxRole,
    ) -> CommandResult<()> {
        let mut connection = self.pool.acquire().await.map_err(import_error)?;
        validate_source(&mut connection, slot, mailbox, message, role).await
    }

    /// Bcc is envelope-only for mail sent here. Recover it only for a byte-identical
    /// MIME in the same account; never infer hidden recipients from contacts.
    pub async fn locally_sent_bcc(
        &self,
        slot: &str,
        message: &str,
    ) -> CommandResult<Option<Vec<MessageAddress>>> {
        let json = sqlx::query_scalar::<_, String>(
            "SELECT d.bcc_json FROM messages m JOIN send_jobs j ON j.mime_hash = m.raw_content_hash AND j.account_slot_id = m.account_slot_id \
             JOIN drafts d ON d.id = j.draft_id AND d.account_slot_id = j.account_slot_id \
             WHERE m.id = ? AND m.account_slot_id = ? AND j.status = 'sent' LIMIT 1")
            .bind(message).bind(slot).fetch_optional(&self.pool).await.map_err(import_error)?;
        json.map(decode_addresses).transpose()
    }

    pub async fn persist_imported_draft(
        &self,
        request: PersistImportedDraftRequest<'_>,
    ) -> CommandResult<DraftDetail> {
        let PersistImportedDraftRequest {
            account_id,
            account_slot_id,
            message_id,
            mailbox_id,
            source_role,
            source,
            content,
            attachments,
        } = request;
        // Complete slow content writes before acquiring SQLite's write lock.
        let mut hashes = Vec::with_capacity(attachments.len());
        for attachment in attachments {
            hashes.push(self.content.write_attachment(&attachment.bytes).await?);
        }
        let mut transaction = super::super::begin_write(&self.pool)
            .await
            .map_err(import_error)?;
        validate_source(
            &mut transaction,
            account_slot_id,
            mailbox_id,
            message_id,
            &source_role,
        )
        .await?;
        let is_remote_draft = source_role == MailboxRole::Drafts;
        if is_remote_draft {
            let existing = sqlx::query_scalar::<_, String>("SELECT id FROM drafts WHERE source_message_id = ? AND account_slot_id = ? AND status = 'editing'")
                .bind(message_id).bind(account_slot_id).fetch_optional(&mut *transaction).await.map_err(import_error)?;
            if let Some(id) = existing {
                transaction.rollback().await.map_err(import_error)?;
                return self.get_draft(account_id, account_slot_id, &id).await;
            }
        }
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO drafts(id, account_slot_id, source_message_id, related_message_id, to_json, cc_json, bcc_json, subject, editor_json, html, plain_text, discard_if_untouched, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)")
            .bind(&id).bind(account_slot_id)
            .bind(is_remote_draft.then_some(message_id)).bind((!is_remote_draft).then_some(message_id))
            .bind(encode_addresses(&source.recipients.to)?).bind(encode_addresses(&source.recipients.cc)?).bind(encode_addresses(&source.recipients.bcc)?)
            .bind(&source.subject).bind(&content.editor_json).bind(&content.html).bind(&content.plain_text)
            .bind(timestamp).bind(timestamp).execute(&mut *transaction).await.map_err(import_error)?;
        for (index, (attachment, hash)) in attachments.iter().zip(hashes).enumerate() {
            sqlx::query("INSERT INTO draft_attachments(id, draft_id, file_name, content_type, size, content_hash, content_id, is_inline, sort_order, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(Uuid::new_v4().to_string()).bind(&id).bind(sanitize_attachment_file_name(&attachment.file_name))
                .bind(&attachment.content_type).bind(attachment.bytes.len() as i64).bind(hash)
                .bind(&attachment.content_id).bind(i64::from(attachment.content_id.is_some())).bind(index as i64).bind(timestamp)
                .execute(&mut *transaction).await.map_err(import_error)?;
        }
        transaction.commit().await.map_err(import_error)?;
        self.get_draft(account_id, account_slot_id, &id).await
    }
}

async fn validate_source(
    connection: &mut sqlx::SqliteConnection,
    slot: &str,
    mailbox: &str,
    message: &str,
    role: &MailboxRole,
) -> CommandResult<()> {
    let expected_role = match role {
        MailboxRole::Sent => "sent",
        MailboxRole::Drafts => "drafts",
        _ => return Err(CommandError::new("draft.source_not_editable")),
    };
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM message_locations l JOIN messages m ON m.id = l.message_id \
         JOIN mailboxes b ON b.id = l.mailbox_id AND b.account_slot_id = m.account_slot_id \
         LEFT JOIN mailbox_role_overrides o ON o.mailbox_id = b.id AND o.account_slot_id = b.account_slot_id \
         WHERE m.id = ? AND m.account_slot_id = ? AND b.id = ? AND l.local_hidden = 0 AND b.selectable = 1 \
         AND (CASE WHEN o.role IS NOT NULL THEN o.role \
           WHEN EXISTS(SELECT 1 FROM mailbox_role_overrides x WHERE x.account_slot_id = b.account_slot_id AND x.role = b.role) THEN 'other' ELSE b.role END) = ?)")
        .bind(message).bind(slot).bind(mailbox).bind(expected_role).fetch_one(connection).await.map_err(import_error)?;
    if !valid {
        return Err(CommandError::new("draft.source_not_editable"));
    }
    Ok(())
}

fn import_error(error: sqlx::Error) -> CommandError {
    crate::diagnostics::command_error("draft.import_failed", false, &error)
}

#[cfg(test)]
mod tests;

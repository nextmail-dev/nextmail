use std::path::Path;

use crate::core::{CommandError, CommandResult, RemoteMessageBody};
use async_trait::async_trait;
use sqlx::SqlitePool;

use super::{
    begin_write, map_storage_err, now, CompositionDefinitionRepository, ContactRepository,
    ContentStore, DraftRepository, MailboxRepository, MailboxRoleRepository, OperationRepository,
    SendJobRepository,
};

#[derive(Clone)]
pub struct MailRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone)]
pub struct MailReadRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[derive(Clone)]
pub struct SyncSinkRepository {
    pub(crate) pool: SqlitePool,
    pub(crate) content: ContentStore,
}

#[async_trait]
pub trait MailRepositoryProvider: Send + Sync {
    async fn open(&self, data_dir: &Path) -> CommandResult<MailRepository>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteMailRepositoryProvider;

#[async_trait]
impl MailRepositoryProvider for SqliteMailRepositoryProvider {
    async fn open(&self, data_dir: &Path) -> CommandResult<MailRepository> {
        MailRepository::open(data_dir).await
    }
}

#[derive(Clone, Debug)]
pub struct RemoteMessageContext {
    pub mailbox_id: String,
    pub mailbox_name: String,
    pub uid: u32,
    pub uid_validity: u32,
}

impl MailRepository {
    pub fn read(&self) -> MailReadRepository {
        MailReadRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn sync_sink(&self) -> SyncSinkRepository {
        SyncSinkRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn contacts(&self) -> ContactRepository {
        ContactRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn drafts(&self) -> DraftRepository {
        DraftRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn send_jobs(&self) -> SendJobRepository {
        SendJobRepository {
            pool: self.pool.clone(),
            content: self.content.clone(),
        }
    }

    pub fn operations(&self) -> OperationRepository {
        OperationRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn mailbox_roles(&self) -> MailboxRoleRepository {
        MailboxRoleRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn mailboxes(&self) -> MailboxRepository {
        MailboxRepository {
            pool: self.pool.clone(),
        }
    }

    pub fn composition_definitions(&self) -> CompositionDefinitionRepository {
        CompositionDefinitionRepository {
            pool: self.pool.clone(),
        }
    }
}

impl SyncSinkRepository {
    pub async fn replace_message_body(
        &self,
        account_slot_id: &str,
        message_id: &str,
        body: &RemoteMessageBody,
    ) -> CommandResult<()> {
        let mut transaction = begin_write(&self.pool)
            .await
            .map_err(map_storage_err("storage.message_body_write_failed"))?;
        let result = sqlx::query(
            "UPDATE messages SET body_availability = 'available', remote_images_blocked = ?, \
             preview = COALESCE(?, preview), \
             revision = revision + 1 WHERE id = ? AND account_slot_id = ?",
        )
        .bind(i64::from(body.remote_images_blocked))
        .bind(&body.preview)
        .bind(message_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        if result.rows_affected() != 1 {
            return Err(CommandError::new("message.not_found"));
        }
        sqlx::query(
            "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
             VALUES (?, ?, ?, ?) ON CONFLICT(message_id) DO UPDATE SET \
             plain_text = excluded.plain_text, safe_html = excluded.safe_html, \
             updated_at = excluded.updated_at",
        )
        .bind(message_id)
        .bind(&body.plain_text)
        .bind(&body.safe_html)
        .bind(now())
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        for attachment in &body.attachments {
            sqlx::query(
                "INSERT INTO attachments(id, message_id, part_index, imap_section, file_name, content_type, size, content_id) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(message_id, part_index) DO UPDATE SET \
                 imap_section = COALESCE(excluded.imap_section, attachments.imap_section), \
                 file_name = excluded.file_name, content_type = excluded.content_type, \
                 size = CASE WHEN attachments.availability = 'available' THEN attachments.size ELSE excluded.size END, \
                 content_id = excluded.content_id",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(message_id)
            .bind(i64::from(attachment.part_index))
            .bind(&attachment.imap_section)
            .bind(&attachment.file_name)
            .bind(&attachment.content_type)
            .bind(attachment.size as i64)
            .bind(&attachment.content_id)
            .execute(&mut *transaction)
            .await
            .map_err(map_storage_err("storage.attachment_write_failed"))?;
        }
        for content_id in &body.inline_content_ids {
            sqlx::query("DELETE FROM attachments WHERE message_id = ? AND lower(content_id) = ?")
                .bind(message_id)
                .bind(content_id)
                .execute(&mut *transaction)
                .await
                .map_err(map_storage_err("storage.message_body_write_failed"))?;
        }
        sqlx::query(
            "UPDATE messages SET has_attachments = EXISTS(SELECT 1 FROM attachments WHERE message_id = ?) \
             WHERE id = ? AND account_slot_id = ?",
        )
        .bind(message_id)
        .bind(message_id)
        .bind(account_slot_id)
        .execute(&mut *transaction)
        .await
        .map_err(map_storage_err("storage.message_body_write_failed"))?;
        transaction
            .commit()
            .await
            .map_err(map_storage_err("storage.message_body_write_failed"))
    }
}

#[cfg(test)]
mod tests;

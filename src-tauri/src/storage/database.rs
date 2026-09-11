use std::{path::Path, str::FromStr, time::Duration};

use crate::core::{CommandError, CommandResult};
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    ConnectOptions, SqlitePool,
};

use super::{ContentStore, MailRepository};

pub const CONTENT_DATABASE_FILENAME: &str = "content.sqlite";

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

impl MailRepository {
    pub async fn open(data_dir: &Path) -> CommandResult<Self> {
        let pool = open_pool(data_dir, false).await?;
        MIGRATOR.run(&pool).await.map_err(|error| {
            crate::diagnostics::command_error(
                "data_directory.database_migration_failed",
                false,
                &error,
            )
        })?;
        Ok(Self {
            pool,
            content: ContentStore::new(data_dir),
        })
    }
}

pub async fn initialize_content_database(data_dir: &Path) -> CommandResult<()> {
    let pool = open_pool(data_dir, true).await?;
    MIGRATOR.run(&pool).await.map_err(|error| {
        crate::diagnostics::command_error("data_directory.database_migration_failed", false, &error)
    })?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error(
                "data_directory.database_checkpoint_failed",
                false,
                &error,
            )
        })?;
    pool.close().await;
    Ok(())
}

pub async fn create_account_slot(
    data_dir: &Path,
    slot_id: &str,
    created_at: i64,
) -> CommandResult<()> {
    let pool = open_pool(data_dir, false).await?;
    MIGRATOR.run(&pool).await.map_err(|error| {
        crate::diagnostics::command_error("data_directory.database_migration_failed", false, &error)
    })?;
    sqlx::query("INSERT INTO account_slots (id, created_at) VALUES (?, ?)")
        .bind(slot_id)
        .bind(created_at)
        .execute(&pool)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("account.slot_create_failed", false, &error)
        })?;
    pool.close().await;
    Ok(())
}

pub async fn delete_account_slot(data_dir: &Path, slot_id: &str) {
    if let Ok(pool) = open_pool(data_dir, false).await {
        let _ = sqlx::query("DELETE FROM account_slots WHERE id = ?")
            .bind(slot_id)
            .execute(&pool)
            .await;
        pool.close().await;
    }
}

pub(super) async fn open_pool(data_dir: &Path, create: bool) -> CommandResult<SqlitePool> {
    let database_path = data_dir.join(CONTENT_DATABASE_FILENAME);
    if !create && !database_path.is_file() {
        return Err(CommandError::new("data_directory.database_missing"));
    }
    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", database_path.display()))
        .map_err(|error| {
            crate::diagnostics::command_error("data_directory.database_open_failed", false, &error)
        })?
        .create_if_missing(create)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        // SQLite serializes all writers through a single lock even in WAL mode.
        // The sync runtime fans out up to 3 worker sessions that each upsert in
        // their own transaction; without an explicit busy timeout a worker that
        // loses the write lock fails instantly with SQLITE_BUSY instead of
        // waiting for the in-progress write to finish. 15s is far beyond the
        // per-message upsert cost, so this turns contention into brief waits.
        .busy_timeout(Duration::from_secs(15))
        .disable_statement_logging();
    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("data_directory.database_open_failed", false, &error)
        })
}

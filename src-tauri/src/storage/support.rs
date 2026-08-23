use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::{CommandError, CommandResult, MailboxRole};
use sqlx::SqlitePool;

pub(crate) async fn begin_write(
    pool: &SqlitePool,
) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, sqlx::Error> {
    // BEGIN IMMEDIATE obtains SQLite's single writer slot before any reads in
    // the transaction, so busy_timeout can wait instead of a later read-to-write
    // upgrade failing immediately with SQLITE_BUSY.
    pool.begin_with("BEGIN IMMEDIATE").await
}

pub(crate) fn encode_json<T: serde::Serialize>(value: &T) -> CommandResult<String> {
    serde_json::to_string(value).map_err(map_storage_err("storage.json_encode_failed"))
}

pub(crate) fn role_to_db(role: &MailboxRole) -> &'static str {
    match role {
        MailboxRole::Inbox => "inbox",
        MailboxRole::Sent => "sent",
        MailboxRole::Drafts => "drafts",
        MailboxRole::Trash => "trash",
        MailboxRole::Junk => "junk",
        MailboxRole::Archive => "archive",
        MailboxRole::Other => "other",
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn storage_read_error(error: sqlx::Error) -> CommandError {
    tracing::warn!(?error, "storage read failed");
    CommandError::new("storage.read_failed")
}

// Mirrors `map_imap_err`: preserves the underlying storage error in the log
// instead of discarding it via `.map_err(|_| ...)`. Without this every storage
// write failure surfaces only a generic code (e.g. "storage.message_write_failed")
// and the real cause - SQLITE_BUSY, disk I/O, a trigger fault - is lost.
pub(crate) fn map_storage_err<E: std::fmt::Debug>(
    code: &'static str,
) -> impl FnOnce(E) -> CommandError {
    move |error| {
        tracing::warn!(%code, ?error, "storage operation failed");
        CommandError::new(code)
    }
}

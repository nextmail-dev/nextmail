use std::time::Duration;

use super::super::database::open_pool;
use super::*;
use crate::core::{
    ContactAddressRole, ContactDraft, ContactGroupDraft, ContentAvailability, MailSyncSink,
    MailboxRole, MessageAddress, RemoteContactAddress, RemoteMailbox, RemoteMessage, StoredMailbox,
    SyncInterval, MISSING_MESSAGE_PREVIEW,
};
use crate::storage::{create_account_slot, initialize_content_database};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn write_transactions_wait_for_the_active_writer() {
    let directory = tempfile::tempdir().unwrap();
    let pool = open_pool(directory.path(), true).await.unwrap();
    let first = begin_write(&pool).await.unwrap();
    let waiting_pool = pool.clone();
    let mut waiting = tokio::spawn(async move {
        begin_write(&waiting_pool)
            .await
            .unwrap()
            .rollback()
            .await
            .unwrap();
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(30), &mut waiting)
            .await
            .is_err()
    );
    first.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .expect("waiting writer should acquire the released slot")
        .unwrap();
}

#[tokio::test]
async fn stylesheet_policy_migration_invalidates_only_cached_html_bodies() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            remote_images_blocked INTEGER NOT NULL,
            revision INTEGER NOT NULL
         );
         CREATE TABLE message_bodies(
            message_id TEXT PRIMARY KEY,
            plain_text TEXT,
            safe_html TEXT
         );
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '9');
         INSERT INTO messages VALUES ('html', 'available', 1, 4);
         INSERT INTO messages VALUES ('plain', 'available', 0, 2);
         INSERT INTO message_bodies VALUES ('html', 'HTML fallback', '<p>Cached</p>');
         INSERT INTO message_bodies VALUES ('plain', 'Plain only', NULL);",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '10' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let html_message: (String, i64, i64) = sqlx::query_as(
        "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(html_message, ("missing".to_owned(), 0, 5));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let plain_message: (String, i64) =
        sqlx::query_as("SELECT body_availability, revision FROM messages WHERE id = 'plain'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(plain_message, ("available".to_owned(), 2));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "Plain only"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "10"
    );
}

#[tokio::test]
async fn transient_controlled_link_schema_is_removed_by_direct_link_migration() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            remote_images_blocked INTEGER NOT NULL,
            revision INTEGER NOT NULL
         );
         CREATE TABLE message_bodies(
            message_id TEXT PRIMARY KEY,
            plain_text TEXT,
            safe_html TEXT
         );
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '10');
         INSERT INTO messages VALUES ('html', 'available', 0, 3);
         INSERT INTO message_bodies VALUES ('html', 'fallback', '<a>old linkless cache</a>');",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"CREATE TABLE IF NOT EXISTS message_links (
id TEXT PRIMARY KEY NOT NULL,
message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
ordinal INTEGER NOT NULL,
target_url TEXT NOT NULL,
UNIQUE(message_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_message_links_message
ON message_links(message_id, ordinal);

UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '11' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let message: (String, i64) =
        sqlx::query_as("SELECT body_availability, revision FROM messages WHERE id = 'html'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(message, ("missing".to_owned(), 4));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message_bodies")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "11"
    );

    sqlx::raw_sql(
        "UPDATE messages SET body_availability = 'available';
         INSERT INTO message_bodies(message_id, plain_text, safe_html)
         VALUES ('html', 'fallback', '<a>cached bridge</a>');",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(
        r#"DROP TABLE IF EXISTS message_links;

UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '12' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "12"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'message_links'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message_bodies")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn functional_selector_policy_migration_invalidates_cached_html() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            remote_images_blocked INTEGER NOT NULL,
            revision INTEGER NOT NULL
         );
         CREATE TABLE message_bodies(
            message_id TEXT PRIMARY KEY,
            plain_text TEXT,
            safe_html TEXT
         );
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '13');
         INSERT INTO messages VALUES ('html', 'available', 1, 7);
         INSERT INTO messages VALUES ('plain', 'available', 0, 3);
         INSERT INTO message_bodies VALUES ('html', 'fallback', '<table>stale</table>');
         INSERT INTO message_bodies VALUES ('plain', 'plain only', NULL);",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '14' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("missing".to_owned(), 0, 8)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "plain only"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "14"
    );
}

async fn assert_html_cache_invalidation_migration(
    migration: &'static str,
    initial_version: &str,
    expected_version: &str,
) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            remote_images_blocked INTEGER NOT NULL,
            revision INTEGER NOT NULL
         );
             CREATE TABLE message_bodies(
                message_id TEXT PRIMARY KEY,
                plain_text TEXT,
                safe_html TEXT
             );
             CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO messages VALUES ('html', 'available', 1, 9);
             INSERT INTO messages VALUES ('plain', 'available', 0, 4);
             INSERT INTO message_bodies VALUES ('html', 'fallback', '<img>');
             INSERT INTO message_bodies VALUES ('plain', 'plain only', NULL);",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', ?)")
        .bind(initial_version)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(migration).execute(&pool).await.unwrap();

    assert_eq!(
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT body_availability, remote_images_blocked, revision FROM messages WHERE id = 'html'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("missing".to_owned(), 0, 10)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'html'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT plain_text FROM message_bodies WHERE message_id = 'plain'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "plain only"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        expected_version
    );
}

#[tokio::test]
async fn inline_image_policy_migration_invalidates_only_cached_html() {
    assert_html_cache_invalidation_migration(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '21' WHERE key = 'data_format_version';
"#,
        "20",
        "21",
    )
    .await;
}

#[tokio::test]
async fn octet_stream_cid_policy_migration_invalidates_only_cached_html() {
    assert_html_cache_invalidation_migration(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '22' WHERE key = 'data_format_version';
"#,
        "21",
        "22",
    )
    .await;
}

#[tokio::test]
async fn bmp_inline_image_policy_migration_invalidates_only_cached_html() {
    assert_html_cache_invalidation_migration(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT message_id
FROM message_bodies
WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '23' WHERE key = 'data_format_version';
"#,
        "22",
        "23",
    )
    .await;
}

#[tokio::test]
async fn selective_cid_refresh_invalidates_only_mislabeled_image_candidates() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            remote_images_blocked INTEGER NOT NULL,
            revision INTEGER NOT NULL
         );
         CREATE TABLE message_bodies(
            message_id TEXT PRIMARY KEY,
            plain_text TEXT,
            safe_html TEXT
         );
         CREATE TABLE attachments(
            message_id TEXT NOT NULL,
            content_type TEXT NOT NULL,
            content_id TEXT
         );
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata VALUES ('data_format_version', '27');
         INSERT INTO messages VALUES ('mislabeled', 'available', 1, 4);
         INSERT INTO messages VALUES ('ordinary', 'available', 1, 7);
         INSERT INTO message_bodies VALUES ('mislabeled', NULL, '<img>');
         INSERT INTO message_bodies VALUES ('ordinary', NULL, '<p>body</p>');
         INSERT INTO attachments VALUES (
            'mislabeled', 'application/octet-stream', 'logo@example.test'
         );
         INSERT INTO attachments VALUES (
            'ordinary', 'application/pdf', 'report@example.test'
         );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"UPDATE messages
SET body_availability = 'missing',
remote_images_blocked = 0,
revision = revision + 1
WHERE id IN (
SELECT body.message_id
FROM message_bodies body
WHERE body.safe_html IS NOT NULL
  AND EXISTS (
      SELECT 1
      FROM attachments attachment
      WHERE attachment.message_id = body.message_id
        AND attachment.content_id IS NOT NULL
        AND lower(attachment.content_type) = 'application/octet-stream'
  )
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL
  AND EXISTS (
  SELECT 1
  FROM attachments attachment
  WHERE attachment.message_id = message_bodies.message_id
    AND attachment.content_id IS NOT NULL
    AND lower(attachment.content_type) = 'application/octet-stream'
  );

UPDATE schema_metadata SET value = '28' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT body_availability, remote_images_blocked, revision \
             FROM messages WHERE id = 'mislabeled'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("missing".to_owned(), 0, 5)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'mislabeled'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_as::<_, (String, i64)>(
            "SELECT body_availability, revision FROM messages WHERE id = 'ordinary'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("available".to_owned(), 7)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "28"
    );
}

#[tokio::test]
async fn attachment_filename_refresh_invalidates_only_encoded_name_candidates() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            body_availability TEXT NOT NULL,
            revision INTEGER NOT NULL
         );
         CREATE TABLE message_bodies(message_id TEXT PRIMARY KEY, safe_html TEXT);
         CREATE TABLE attachments(message_id TEXT NOT NULL, file_name TEXT NOT NULL);
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata VALUES ('data_format_version', '28');
         INSERT INTO messages VALUES ('encoded', 'available', 4);
         INSERT INTO messages VALUES ('ordinary', 'available', 7);
         INSERT INTO message_bodies VALUES ('encoded', '<p>body</p>');
         INSERT INTO message_bodies VALUES ('ordinary', '<p>body</p>');
         INSERT INTO attachments VALUES ('encoded', '=?utf-8?B?5rWZ5rGfLmRvY3g=?=');
         INSERT INTO attachments VALUES ('ordinary', 'report.docx');",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"UPDATE messages
SET body_availability = 'missing',
revision = revision + 1
WHERE id IN (
SELECT attachment.message_id
FROM attachments attachment
WHERE substr(attachment.file_name, 1, 2) = '=?'
);

DELETE FROM message_bodies
WHERE message_id IN (
SELECT attachment.message_id
FROM attachments attachment
WHERE substr(attachment.file_name, 1, 2) = '=?'
);

UPDATE schema_metadata SET value = '29' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        sqlx::query_as::<_, (String, i64)>(
            "SELECT body_availability, revision FROM messages WHERE id = 'encoded'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("missing".to_owned(), 5)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM message_bodies WHERE message_id = 'encoded'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_as::<_, (String, i64)>(
            "SELECT body_availability, revision FROM messages WHERE id = 'ordinary'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("available".to_owned(), 7)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "29"
    );
}

#[tokio::test]
async fn local_search_migration_backfills_existing_searchable_content() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages(
            id TEXT PRIMARY KEY,
            account_slot_id TEXT NOT NULL,
            subject TEXT NOT NULL,
            from_json TEXT NOT NULL,
            to_json TEXT NOT NULL,
            cc_json TEXT NOT NULL,
            preview TEXT NOT NULL
         );
         CREATE TABLE message_bodies(
            message_id TEXT PRIMARY KEY,
            plain_text TEXT
         );
         CREATE TABLE attachments(
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            file_name TEXT NOT NULL
         );
         CREATE TABLE schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO schema_metadata(key, value) VALUES ('data_format_version', '14');
         INSERT INTO messages VALUES (
            'legacy-message', 'slot', 'Legacy subject',
            '[{\"name\":\"Alice\",\"email\":\"alice@example.com\"}]', '[]', '[]',
            'Legacy preview'
         );
         INSERT INTO message_bodies VALUES ('legacy-message', 'Legacy offline body');
         INSERT INTO attachments VALUES (
            'legacy-attachment', 'legacy-message', 'legacy-report.pdf'
         );",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        r#"CREATE VIRTUAL TABLE message_search USING fts5(
message_id UNINDEXED,
account_slot_id UNINDEXED,
subject,
addresses,
preview,
body,
attachment_names,
tokenize = 'trigram case_sensitive 0 remove_diacritics 1'
);

INSERT INTO message_search(
message_id,
account_slot_id,
subject,
addresses,
preview,
body,
attachment_names
)
SELECT
m.id,
m.account_slot_id,
m.subject,
m.from_json || ' ' || m.to_json || ' ' || m.cc_json,
m.preview,
COALESCE(b.plain_text, ''),
COALESCE((
    SELECT group_concat(a.file_name, ' ')
    FROM attachments a
    WHERE a.message_id = m.id
), '')
FROM messages m
LEFT JOIN message_bodies b ON b.message_id = m.id;

CREATE TRIGGER message_search_messages_ai
AFTER INSERT ON messages
BEGIN
INSERT INTO message_search(
    message_id,
    account_slot_id,
    subject,
    addresses,
    preview,
    body,
    attachment_names
) VALUES (
    NEW.id,
    NEW.account_slot_id,
    NEW.subject,
    NEW.from_json || ' ' || NEW.to_json || ' ' || NEW.cc_json,
    NEW.preview,
    '',
    ''
);
END;

CREATE TRIGGER message_search_messages_au
AFTER UPDATE OF account_slot_id, subject, from_json, to_json, cc_json, preview ON messages
BEGIN
DELETE FROM message_search WHERE message_id = OLD.id;
INSERT INTO message_search(
    message_id,
    account_slot_id,
    subject,
    addresses,
    preview,
    body,
    attachment_names
)
SELECT
    NEW.id,
    NEW.account_slot_id,
    NEW.subject,
    NEW.from_json || ' ' || NEW.to_json || ' ' || NEW.cc_json,
    NEW.preview,
    COALESCE(b.plain_text, ''),
    COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = NEW.id
    ), '')
FROM (SELECT 1)
LEFT JOIN message_bodies b ON b.message_id = NEW.id;
END;

CREATE TRIGGER message_search_messages_ad
AFTER DELETE ON messages
BEGIN
DELETE FROM message_search WHERE message_id = OLD.id;
END;

CREATE TRIGGER message_search_bodies_ai
AFTER INSERT ON message_bodies
BEGIN
UPDATE message_search
SET body = COALESCE(NEW.plain_text, '')
WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_bodies_au
AFTER UPDATE OF message_id, plain_text ON message_bodies
BEGIN
UPDATE message_search SET body = '' WHERE message_id = OLD.message_id;
UPDATE message_search
SET body = COALESCE(NEW.plain_text, '')
WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_bodies_ad
AFTER DELETE ON message_bodies
BEGIN
UPDATE message_search SET body = '' WHERE message_id = OLD.message_id;
END;

CREATE TRIGGER message_search_attachments_ai
AFTER INSERT ON attachments
BEGIN
UPDATE message_search
SET attachment_names = COALESCE((
    SELECT group_concat(a.file_name, ' ')
    FROM attachments a
    WHERE a.message_id = NEW.message_id
), '')
WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_attachments_au
AFTER UPDATE OF message_id, file_name ON attachments
BEGIN
UPDATE message_search
SET attachment_names = COALESCE((
    SELECT group_concat(a.file_name, ' ')
    FROM attachments a
    WHERE a.message_id = OLD.message_id
), '')
WHERE message_id = OLD.message_id;
UPDATE message_search
SET attachment_names = COALESCE((
    SELECT group_concat(a.file_name, ' ')
    FROM attachments a
    WHERE a.message_id = NEW.message_id
), '')
WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_attachments_ad
AFTER DELETE ON attachments
BEGIN
UPDATE message_search
SET attachment_names = COALESCE((
    SELECT group_concat(a.file_name, ' ')
    FROM attachments a
    WHERE a.message_id = OLD.message_id
), '')
WHERE message_id = OLD.message_id;
END;

UPDATE schema_metadata SET value = '15' WHERE key = 'data_format_version';
"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    for query in [
        "\"Legacy subject\"",
        "\"Alice\"",
        "\"offline body\"",
        "\"report.pdf\"",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM message_search WHERE message_search MATCH ?"
            )
            .bind(query)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        "15"
    );
}

#[tokio::test]
async fn rebuilt_message_bodies_are_written_atomically_with_account_isolation() {
    let (_directory, repository, mailbox) = repository_with_mailbox(1).await;
    let mut remote = remote_message(1, 1, "Cached");
    remote.attachments = vec![crate::core::RemoteAttachment {
        part_index: 0,
        imap_section: None,
        file_name: "inline.png".to_owned(),
        content_type: "image/png".to_owned(),
        size: 128,
        content_id: Some("Logo@Example.Test".to_owned()),
    }];
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote)
        .await
        .unwrap();
    let message = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 20)
        .await
        .unwrap()
        .items
        .remove(0);

    let error = repository
        .sync_sink()
        .replace_message_body(
            "another-slot",
            &message.id,
            &crate::core::RemoteMessageBody {
                plain_text: Some("wrong account".to_owned()),
                safe_html: Some("<p>wrong account</p>".to_owned()),
                preview: None,
                attachments: Vec::new(),
                remote_images_blocked: false,
                inline_content_ids: Vec::new(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "message.not_found");

    repository
        .sync_sink()
        .replace_message_body(
            "slot",
            &message.id,
            &crate::core::RemoteMessageBody {
                plain_text: Some("offline body".to_owned()),
                safe_html: Some("<p>offline body</p>".to_owned()),
                preview: None,
                attachments: Vec::new(),
                remote_images_blocked: true,
                inline_content_ids: vec!["logo@example.test".to_owned()],
            },
        )
        .await
        .unwrap();
    let detail = repository
        .read()
        .get_message_detail("slot", &message.id, Some(&mailbox.id))
        .await
        .unwrap();
    assert_eq!(detail.plain_text.as_deref(), Some("offline body"));
    assert!(detail.attachments.is_empty());
    assert_eq!(detail.safe_html.as_deref(), Some("<p>offline body</p>"));
    assert!(detail.remote_images_blocked);
    assert_eq!(detail.body_availability, ContentAvailability::Available);
}

#[tokio::test]
async fn virtual_views_filter_messages_across_account_mailboxes() {
    let (_directory, repository, inbox) = repository_with_mailbox(7).await;
    let archive = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                name: "Archive".to_owned(),
                display_name: "Archive".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Archive,
                selectable: true,
                uid_validity: 8,
                uid_next: 3,
                total_count: 2,
                unread_count: 1,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();
    let mailboxes = repository
        .read()
        .list_mailboxes("account", "slot")
        .await
        .unwrap();
    assert!(
        mailboxes
            .iter()
            .find(|mailbox| mailbox.id == inbox.id)
            .unwrap()
            .is_favorite
    );
    assert!(
        !mailboxes
            .iter()
            .find(|mailbox| mailbox.id == archive.id)
            .unwrap()
            .is_favorite
    );
    let mut inbox_unread = remote_message(1, 7, "Inbox unread");
    inbox_unread.received_at = 100;
    repository
        .sync_sink()
        .upsert_message("slot", &inbox.id, &inbox_unread)
        .await
        .unwrap();
    let mut archive_unread = remote_message(1, 8, "Archive unread");
    archive_unread.received_at = 200;
    repository
        .sync_sink()
        .upsert_message("slot", &archive.id, &archive_unread)
        .await
        .unwrap();
    let mut archive_read = remote_message(2, 8, "Archive read");
    archive_read.received_at = 300;
    archive_read.unread = false;
    archive_read.flagged = true;
    repository
        .sync_sink()
        .upsert_message("slot", &archive.id, &archive_read)
        .await
        .unwrap();

    let first = repository
        .read()
        .list_unread_messages("slot", None, 1)
        .await
        .unwrap();
    assert_eq!(first.items[0].subject, "Archive unread");
    let second = repository
        .read()
        .list_unread_messages("slot", first.next_cursor.as_deref(), 10)
        .await
        .unwrap();
    assert_eq!(second.items[0].subject, "Inbox unread");
    assert!(second.items.iter().all(|message| message.unread));

    let starred = repository
        .read()
        .list_starred_messages("slot", None, 10)
        .await
        .unwrap();
    assert_eq!(starred.items.len(), 1);
    assert_eq!(starred.items[0].subject, "Archive read");
    assert!(starred.items[0].flagged);
}

#[tokio::test]
async fn local_search_indexes_message_content_with_mailbox_and_account_isolation() {
    let (directory, repository, inbox) = repository_with_mailbox(7).await;
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let archive = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                name: "Archive".to_owned(),
                display_name: "Archive".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Archive,
                selectable: true,
                uid_validity: 8,
                uid_next: 2,
                total_count: 1,
                unread_count: 0,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();
    let private_inbox = repository
        .sync_sink()
        .upsert_mailbox(
            "slot-b",
            &RemoteMailbox {
                name: "INBOX".to_owned(),
                display_name: "INBOX".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Inbox,
                selectable: true,
                uid_validity: 9,
                uid_next: 2,
                total_count: 1,
                unread_count: 1,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();

    let mut first = remote_message(1, 7, "Quarterly roadmap");
    first.received_at = 100;
    first.from = vec![MessageAddress {
        name: Some("Alice Example".to_owned()),
        email: "alice@example.com".to_owned(),
    }];
    first.to = vec![MessageAddress {
        name: Some("Bob".to_owned()),
        email: "bob@example.com".to_owned(),
    }];
    first.preview = "Finance update".to_owned();
    first.plain_text = Some("请核对电子发票和本地正文索引".to_owned());
    first.attachments = vec![crate::core::RemoteAttachment {
        part_index: 1,
        imap_section: None,
        file_name: "financial-report.pdf".to_owned(),
        content_type: "application/pdf".to_owned(),
        size: 42,
        content_id: None,
    }];
    repository
        .sync_sink()
        .upsert_message("slot", &inbox.id, &first)
        .await
        .unwrap();

    let mut second = remote_message(2, 7, "Quarterly follow-up");
    second.received_at = 200;
    repository
        .sync_sink()
        .upsert_message("slot", &inbox.id, &second)
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message("slot", &archive.id, &remote_message(1, 8, "Archive secret"))
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message(
            "slot-b",
            &private_inbox.id,
            &remote_message(1, 9, "Private account message"),
        )
        .await
        .unwrap();

    for query in [
        "Alice Example",
        "alice@example.com",
        "bob@example.com",
        "电子发票",
        "发票",
        "票",
    ] {
        let page = repository
            .read()
            .search_messages("slot", Some(&inbox.id), query, None, 20)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1, "query {query:?} must find the message");
        assert_eq!(page.items[0].subject, "Quarterly roadmap");
    }
    for excluded in ["Finance update", "report.pdf"] {
        assert!(repository
            .read()
            .search_messages("slot", Some(&inbox.id), excluded, None, 20)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    let global_archive = repository
        .read()
        .search_messages("slot", None, "Archive secret", None, 20)
        .await
        .unwrap();
    assert_eq!(global_archive.items.len(), 1);
    assert_eq!(global_archive.items[0].mailbox_id, archive.id);
    let global_archive_short = repository
        .read()
        .search_messages("slot", None, "v", None, 20)
        .await
        .unwrap();
    assert_eq!(global_archive_short.items.len(), 1);
    assert_eq!(global_archive_short.items[0].mailbox_id, archive.id);
    assert!(repository
        .read()
        .search_messages("slot", None, "Private account", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());

    let first_page = repository
        .read()
        .search_messages("slot", Some(&inbox.id), "Quarterly", None, 1)
        .await
        .unwrap();
    assert_eq!(first_page.items[0].subject, "Quarterly follow-up");
    let second_page = repository
        .read()
        .search_messages(
            "slot",
            Some(&inbox.id),
            "Quarterly",
            first_page.next_cursor.as_deref(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(second_page.items[0].subject, "Quarterly roadmap");
    assert!(second_page.next_cursor.is_none());

    assert!(repository
        .read()
        .search_messages("slot", Some(&inbox.id), "Archive secret", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(repository
        .read()
        .search_messages("slot", Some(&inbox.id), "Alice OR Private", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(repository
        .read()
        .search_messages("slot", Some(&inbox.id), "Alice\"", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(repository
        .read()
        .search_messages("slot", Some(&private_inbox.id), "Private account", None, 20,)
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        repository
            .read()
            .search_messages(
                "slot-b",
                Some(&private_inbox.id),
                "Private account",
                None,
                20,
            )
            .await
            .unwrap()
            .items
            .len(),
        1
    );

    let first_id = repository
        .read()
        .search_messages("slot", Some(&inbox.id), "Alice Example", None, 20)
        .await
        .unwrap()
        .items
        .remove(0)
        .id;
    repository
        .sync_sink()
        .replace_message_body(
            "slot",
            &first_id,
            &crate::core::RemoteMessageBody {
                plain_text: Some("replacement searchable content".to_owned()),
                safe_html: None,
                preview: None,
                attachments: Vec::new(),
                remote_images_blocked: false,
                inline_content_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert!(repository
        .read()
        .search_messages("slot", Some(&inbox.id), "电子发票", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        repository
            .read()
            .search_messages("slot", Some(&inbox.id), "searchable content", None, 20,)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn account_sync_interval_defaults_to_one_minute_and_round_trips() {
    let (_directory, repository, _mailbox) = repository_with_mailbox(1).await;
    let read = repository.read();

    assert_eq!(
        read.get_sync_interval("slot").await.unwrap(),
        SyncInterval::Minutes1
    );
    for interval in [
        SyncInterval::Manual,
        SyncInterval::Minutes1,
        SyncInterval::Minutes5,
        SyncInterval::Minutes10,
    ] {
        assert_eq!(
            read.set_sync_interval("slot", interval.clone())
                .await
                .unwrap(),
            interval
        );
        assert_eq!(read.get_sync_interval("slot").await.unwrap(), interval);
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM schema_metadata WHERE key = 'data_format_version'"
        )
        .fetch_one(&repository.pool)
        .await
        .unwrap(),
        "33"
    );
}

#[tokio::test]
async fn full_message_sync_defaults_off_and_round_trips() {
    let (_directory, repository, _mailbox) = repository_with_mailbox(1).await;
    let read = repository.read();

    assert!(!read.get_download_full_messages("slot").await.unwrap());
    assert!(read.set_download_full_messages("slot", true).await.unwrap());
    assert!(read.get_download_full_messages("slot").await.unwrap());
    assert!(!read
        .set_download_full_messages("slot", false)
        .await
        .unwrap());
    assert!(!read.get_download_full_messages("slot").await.unwrap());
}

async fn repository_with_mailbox(
    uid_validity: u32,
) -> (tempfile::TempDir, MailRepository, StoredMailbox) {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let mailbox = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                name: "INBOX".to_owned(),
                display_name: "INBOX".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Inbox,
                selectable: true,
                uid_validity,
                uid_next: 3,
                total_count: 2,
                unread_count: 2,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();
    (directory, repository, mailbox)
}

#[tokio::test]
async fn notification_baseline_and_message_upsert_are_durable() {
    let (_directory, repository, mailbox) = repository_with_mailbox(11).await;
    assert!(mailbox.notification_baseline_required);
    assert!(!repository
        .read()
        .notification_baseline_ready("slot")
        .await
        .unwrap());

    let first = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 11, "First"))
        .await
        .unwrap();
    let duplicate = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 11, "First"))
        .await
        .unwrap();
    assert!(first.is_new_location);
    assert!(!duplicate.is_new_location);
    assert_eq!(first.message_id, duplicate.message_id);

    repository
        .sync_sink()
        .complete_notification_baseline("slot")
        .await
        .unwrap();
    assert!(repository
        .read()
        .notification_baseline_ready("slot")
        .await
        .unwrap());
    let existing_mailbox = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                name: "INBOX".to_owned(),
                display_name: "INBOX".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Inbox,
                selectable: true,
                uid_validity: 11,
                uid_next: 3,
                total_count: 2,
                unread_count: 1,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();
    assert!(!existing_mailbox.notification_baseline_required);
}

#[tokio::test]
async fn fallback_preview_does_not_replace_existing_text() {
    let (_directory, repository, mailbox) = repository_with_mailbox(11).await;
    let mut message = remote_message(1, 11, "Existing preview");
    message.preview = "A real text preview".to_owned();
    message.plain_text = None;
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();

    message.preview = MISSING_MESSAGE_PREVIEW.to_owned();
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();

    let mut missing = remote_message(2, 11, "Missing preview");
    missing.preview = MISSING_MESSAGE_PREVIEW.to_owned();
    missing.plain_text = None;
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &missing)
        .await
        .unwrap();

    let messages = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 20)
        .await
        .unwrap()
        .items;
    assert_eq!(
        messages
            .iter()
            .find(|item| item.subject == "Existing preview")
            .unwrap()
            .preview,
        "A real text preview"
    );
    assert_eq!(
        messages
            .iter()
            .find(|item| item.subject == "Missing preview")
            .unwrap()
            .preview,
        MISSING_MESSAGE_PREVIEW
    );
}

#[tokio::test]
async fn synchronized_contacts_are_account_scoped_and_override_header_names() {
    let (directory, repository, mailbox) = repository_with_mailbox(11).await;
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let mut message = remote_message(1, 11, "Contact identity");
    message.from = vec![MessageAddress {
        name: Some("Header Alias".to_owned()),
        email: "Alice@Example.COM".to_owned(),
    }];
    message.to = vec![MessageAddress {
        name: None,
        email: "bob@example.com".to_owned(),
    }];
    message.contact_addresses = vec![
        RemoteContactAddress {
            role: ContactAddressRole::From,
            address: message.from[0].clone(),
        },
        RemoteContactAddress {
            role: ContactAddressRole::To,
            address: message.to[0].clone(),
        },
    ];

    let inserted = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();
    assert!(inserted.contacts_changed);
    let duplicate = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();
    assert!(!duplicate.contacts_changed);

    let contacts = repository
        .contacts()
        .list_contacts("slot", "", None, 20)
        .await
        .unwrap();
    assert_eq!(contacts.total, 2);
    assert!(repository
        .contacts()
        .list_contacts("slot-b", "", None, 20)
        .await
        .unwrap()
        .items
        .is_empty());
    let alice = contacts
        .items
        .into_iter()
        .find(|contact| contact.email.eq_ignore_ascii_case("alice@example.com"))
        .unwrap();
    assert_eq!(alice.name, "Header Alias");
    let mut renamed_message = message.clone();
    renamed_message.from[0].name = Some("Changed Header".to_owned());
    renamed_message.contact_addresses[0].address.name = Some("Changed Header".to_owned());
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &renamed_message)
        .await
        .unwrap();
    assert_eq!(
        repository
            .contacts()
            .get_contact_summary("slot", &alice.id)
            .await
            .unwrap()
            .name,
        "Header Alias"
    );
    let alice = repository
        .contacts()
        .update_contact_name("slot", &alice.id, "Alice Local", alice.revision)
        .await
        .unwrap();
    assert_eq!(alice.name, "Alice Local");
    assert_eq!(
        repository
            .contacts()
            .create_contact(
                "slot",
                &ContactDraft {
                    name: "Duplicate".to_owned(),
                    email: "alice@example.com".to_owned(),
                },
            )
            .await
            .unwrap_err()
            .code,
        "contact.already_exists"
    );

    let listed_message = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 20)
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(listed_message.from[0].name.as_deref(), Some("Alice Local"));
    assert_eq!(
        listed_message.from[0].header_name.as_deref(),
        Some("Changed Header")
    );
    assert_eq!(
        listed_message.from[0].contact_id.as_deref(),
        Some(alice.id.as_str())
    );

    let detail = repository
        .contacts()
        .get_contact_detail("slot", &alice.id, 20)
        .await
        .unwrap();
    assert_eq!(detail.recent_messages.len(), 1);
    assert_eq!(detail.recent_messages[0].subject, "Contact identity");

    assert_eq!(
        repository
            .contacts()
            .delete_contacts("slot-b", std::slice::from_ref(&alice.id))
            .await
            .unwrap_err()
            .code,
        "contact.not_found"
    );
    repository
        .contacts()
        .delete_contacts("slot", std::slice::from_ref(&alice.id))
        .await
        .unwrap();
    assert!(repository
        .contacts()
        .get_contact_summary("slot", &alice.id)
        .await
        .is_err());
    let recreated = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &renamed_message)
        .await
        .unwrap();
    assert!(recreated.contacts_changed);
    assert_eq!(
        repository
            .contacts()
            .list_contacts("slot", "alice@example.com", None, 20)
            .await
            .unwrap()
            .items[0]
            .name,
        "Changed Header"
    );
}

#[tokio::test]
async fn contact_backfill_indexes_messages_stored_before_contact_support() {
    let (_directory, repository, mailbox) = repository_with_mailbox(12).await;
    let mut message = remote_message(1, 12, "Historical contact");
    message.from = vec![MessageAddress {
        name: Some("Historical Header".to_owned()),
        email: "history@example.com".to_owned(),
    }];
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();
    assert_eq!(
        repository
            .contacts()
            .list_contacts("slot", "", None, 20)
            .await
            .unwrap()
            .total,
        0
    );

    let batch = repository
        .contacts()
        .backfill_next_batch("slot")
        .await
        .unwrap();
    assert_eq!(batch.processed, 1);
    assert!(batch.changed);
    assert!(batch.complete);
    let contacts = repository
        .contacts()
        .list_contacts("slot", "history", None, 20)
        .await
        .unwrap();
    assert_eq!(contacts.total, 1);
    assert_eq!(contacts.items[0].name, "Historical Header");
    assert!(
        repository
            .contacts()
            .backfill_next_batch("slot")
            .await
            .unwrap()
            .complete
    );
}

#[tokio::test]
async fn contact_groups_preserve_account_boundaries_and_expand_current_members() {
    let (directory, repository, _) = repository_with_mailbox(12).await;
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let contacts = repository.contacts();
    let alice = contacts
        .create_contact(
            "slot",
            &ContactDraft {
                name: "Alice".into(),
                email: "alice@example.com".into(),
            },
        )
        .await
        .unwrap();
    let bob = contacts
        .create_contact(
            "slot",
            &ContactDraft {
                name: "Bob".into(),
                email: "bob@example.com".into(),
            },
        )
        .await
        .unwrap();
    let foreign = contacts
        .create_contact(
            "slot-b",
            &ContactDraft {
                name: "Foreign".into(),
                email: "foreign@example.com".into(),
            },
        )
        .await
        .unwrap();
    let draft = ContactGroupDraft {
        name: "  Project Team  ".into(),
        contact_ids: vec![alice.id.clone(), bob.id.clone(), alice.id.clone()],
    };
    let detail = contacts
        .save_contact_group("slot", None, &draft, None)
        .await
        .unwrap();
    let id = &detail.group.id;
    assert_eq!(detail.group.name, "Project Team");
    assert_eq!(detail.group.member_count, 2);
    assert_eq!(detail.members.len(), 2);
    assert!(contacts
        .list_contact_groups("slot-b")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        contacts
            .get_contact_group("slot-b", id)
            .await
            .unwrap_err()
            .code,
        "contact_group.not_found"
    );

    assert_eq!(
        contacts
            .save_contact_group(
                "slot",
                None,
                &ContactGroupDraft {
                    name: "project team".into(),
                    contact_ids: vec![],
                },
                None
            )
            .await
            .unwrap_err()
            .code,
        "contact_group.already_exists"
    );
    for name in [" ".to_string(), "x".repeat(81), "Project\nTeam".into()] {
        assert!(contacts
            .save_contact_group(
                "slot",
                None,
                &ContactGroupDraft {
                    name,
                    contact_ids: vec![]
                },
                None
            )
            .await
            .is_err());
    }
    // Same names in different accounts are independent.
    contacts
        .save_contact_group(
            "slot-b",
            None,
            &ContactGroupDraft {
                name: "Project Team".into(),
                contact_ids: vec![foreign.id.clone()],
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        contacts
            .save_contact_group("slot-b", Some(id), &draft, Some(1))
            .await
            .unwrap_err()
            .code,
        "contact_group.conflict"
    );
    assert_eq!(
        contacts
            .delete_contact_group("slot-b", id, 1)
            .await
            .unwrap_err()
            .code,
        "contact_group.conflict"
    );

    // An invalid member rolls back both the rename and membership replacement.
    assert_eq!(
        contacts
            .save_contact_group(
                "slot",
                Some(id),
                &ContactGroupDraft {
                    name: "Changed".into(),
                    contact_ids: vec![alice.id.clone(), foreign.id.clone()],
                },
                Some(1)
            )
            .await
            .unwrap_err()
            .code,
        "contact_group.member_unavailable"
    );
    let unchanged = contacts.get_contact_group("slot", id).await.unwrap();
    assert_eq!(unchanged.group.name, "Project Team");
    assert_eq!(unchanged.group.revision, 1);
    assert_eq!(unchanged.members.len(), 2);
    // The database also rejects cross-account membership independently of application validation.
    assert!(sqlx::query("INSERT INTO contact_group_members(account_slot_id, group_id, contact_id) VALUES ('slot', ?, ?)")
        .bind(id).bind(&foreign.id).execute(&repository.pool).await.is_err());

    let renamed = contacts
        .save_contact_group(
            "slot",
            Some(id),
            &ContactGroupDraft {
                name: "Design Team".into(),
                contact_ids: vec![alice.id.clone(), bob.id.clone()],
            },
            Some(1),
        )
        .await
        .unwrap();
    assert_eq!(renamed.group.revision, 2);
    assert_eq!(
        contacts
            .save_contact_group("slot", Some(id), &draft, Some(1))
            .await
            .unwrap_err()
            .code,
        "contact_group.conflict"
    );
    assert_eq!(
        contacts
            .delete_contact_group("slot", id, 1)
            .await
            .unwrap_err()
            .code,
        "contact_group.conflict"
    );
    contacts
        .save_contact_group(
            "slot",
            None,
            &ContactGroupDraft {
                name: "Empty Team".into(),
                contact_ids: vec![],
            },
            None,
        )
        .await
        .unwrap();
    contacts
        .update_contact_name("slot", &alice.id, "Alice Updated", alice.revision)
        .await
        .unwrap();
    let suggestions = contacts.list_suggestions("slot", "team", 8).await.unwrap();
    assert!(suggestions.contacts.is_empty());
    assert_eq!(suggestions.groups.len(), 1);
    assert_eq!(suggestions.groups[0].members[0].name, "Alice Updated");
    assert_eq!(suggestions.groups[0].members.len(), 2);
    assert!(contacts
        .list_suggestions("slot", "", 8)
        .await
        .unwrap()
        .groups
        .is_empty());
    assert_eq!(
        contacts
            .list_suggestions("slot-b", "team", 8)
            .await
            .unwrap()
            .groups[0]
            .members[0]
            .id,
        foreign.id
    );

    contacts
        .delete_contacts("slot", std::slice::from_ref(&alice.id))
        .await
        .unwrap();
    let remaining = contacts.get_contact_group("slot", id).await.unwrap();
    assert_eq!(remaining.group.member_count, 1);
    assert_eq!(remaining.members[0].id, bob.id);
    contacts.delete_contact_group("slot", id, 2).await.unwrap();
    assert!(contacts.get_contact_summary("slot", &bob.id).await.is_ok());
    assert!(contacts.get_contact_group("slot", id).await.is_err());
    sqlx::query("DELETE FROM account_slots WHERE id = 'slot-b'")
        .execute(&repository.pool)
        .await
        .unwrap();
    assert!(contacts
        .list_contact_groups("slot-b")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn contact_list_search_and_cursor_stay_bounded_with_ten_thousand_rows() {
    let (_directory, repository, _mailbox) = repository_with_mailbox(13).await;
    sqlx::raw_sql(
        "WITH digits(value) AS (
            VALUES (0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
         ), numbered(value) AS (
            SELECT a.value * 1000 + b.value * 100 + c.value * 10 + d.value
            FROM digits a CROSS JOIN digits b CROSS JOIN digits c CROSS JOIN digits d
         )
         INSERT INTO contacts(
            id, account_slot_id, normalized_email, email, name, name_source,
            created_at, updated_at, revision
         )
         SELECT printf('contact-%05d', value), 'slot',
                printf('person-%05d@example.com', value),
                printf('person-%05d@example.com', value),
                printf('Person %05d', value), 'auto', 1, 1, 1
         FROM numbered;",
    )
    .execute(&repository.pool)
    .await
    .unwrap();

    let first = repository
        .contacts()
        .list_contacts("slot", "", None, 100)
        .await
        .unwrap();
    assert_eq!(first.total, 10_000);
    assert_eq!(first.items.len(), 100);
    assert_eq!(first.items[0].name, "Person 00000");
    let second = repository
        .contacts()
        .list_contacts("slot", "", first.next_cursor.as_deref(), 100)
        .await
        .unwrap();
    assert_eq!(second.items.len(), 100);
    assert_eq!(second.items[0].name, "Person 00100");
    let search = repository
        .contacts()
        .list_contacts("slot", "09999", None, 20)
        .await
        .unwrap();
    assert_eq!(search.total, 1);
    assert_eq!(search.items[0].email, "person-09999@example.com");
}

fn remote_message(uid: u32, uid_validity: u32, subject: &str) -> RemoteMessage {
    RemoteMessage {
        uid,
        uid_validity,
        subject: subject.to_owned(),
        from: vec![],
        to: vec![],
        cc: vec![],
        contact_addresses: vec![],
        received_at: i64::from(uid),
        preview: "body".to_owned(),
        unread: true,
        flagged: false,
        high_priority: false,
        size: 20,
        message_id: Some(format!("message-{uid}@example.com")),
        references: vec![],
        in_reply_to: None,
        plain_text: Some("body".to_owned()),
        safe_html: None,
        raw: None,
        attachments: vec![],
        remote_images_blocked: false,
        modseq: None,
    }
}

#[tokio::test]
async fn migration_and_mailbox_round_trip_work() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let mailbox = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                name: "INBOX".to_owned(),
                display_name: "INBOX".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Inbox,
                selectable: true,
                uid_validity: 1,
                uid_next: 2,
                total_count: 1,
                unread_count: 1,
                highest_modseq: None,
            },
        )
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message(
            "slot",
            &mailbox.id,
            &RemoteMessage {
                uid: 1,
                uid_validity: 1,
                subject: "Stored locally".to_owned(),
                from: vec![MessageAddress {
                    name: Some("Alice".to_owned()),
                    email: "alice@example.com".to_owned(),
                }],
                to: vec![],
                cc: vec![],
                contact_addresses: vec![],
                received_at: 10,
                preview: "Hello".to_owned(),
                unread: true,
                flagged: false,
                high_priority: true,
                size: 28,
                message_id: Some("message@example.com".to_owned()),
                references: vec![],
                in_reply_to: None,
                plain_text: Some("Hello from disk".to_owned()),
                safe_html: None,
                raw: Some(b"Subject: Stored locally\r\n\r\nHello".to_vec()),
                attachments: vec![],
                remote_images_blocked: false,
                modseq: None,
            },
        )
        .await
        .unwrap();
    let mailboxes = repository
        .read()
        .list_mailboxes("account", "slot")
        .await
        .unwrap();
    assert_eq!(mailboxes.len(), 1);
    assert_eq!(mailboxes[0].role, MailboxRole::Inbox);

    let page = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 50)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].high_priority);
    let detail = repository
        .read()
        .get_message_detail("slot", &page.items[0].id, Some(&mailbox.id))
        .await
        .unwrap();
    assert_eq!(detail.plain_text.as_deref(), Some("Hello from disk"));
    assert!(detail.high_priority);
    assert!(repository
        .read()
        .raw_message("slot", &detail.id)
        .await
        .unwrap()
        .is_some());
    let context = repository
        .read()
        .remote_message_context("slot", &detail.id)
        .await
        .unwrap();
    assert_eq!(context.mailbox_name, "INBOX");
    assert_eq!(context.uid, 1);

    repository
        .sync_sink()
        .upsert_message(
            "slot",
            &mailbox.id,
            &RemoteMessage {
                uid: 2,
                uid_validity: 1,
                subject: "Header only".to_owned(),
                from: vec![],
                to: vec![],
                cc: vec![],
                contact_addresses: vec![],
                received_at: 20,
                preview: String::new(),
                unread: false,
                flagged: false,
                high_priority: false,
                size: 100,
                message_id: None,
                references: vec![],
                in_reply_to: None,
                plain_text: None,
                safe_html: None,
                raw: None,
                attachments: vec![],
                remote_images_blocked: false,
                modseq: None,
            },
        )
        .await
        .unwrap();
    let pending = repository
        .sync_sink()
        .pending_body_locations(&mailbox.id, Some(15))
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].uid, 2);
    let header_only = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 50)
        .await
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.subject == "Header only")
        .unwrap();
    assert!(repository
        .read()
        .raw_message("slot", &header_only.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn attachment_content_cannot_be_written_through_another_account_slot() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot-a", 1)
        .await
        .unwrap();
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    sqlx::query(
        "INSERT INTO messages(id, account_slot_id, subject, received_at) \
         VALUES ('message-a', 'slot-a', 'Private', 1)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attachments(id, message_id, part_index, file_name, content_type, size) \
         VALUES ('attachment-a', 'message-a', 1, 'private.txt', 'text/plain', 6)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();

    let error = repository
        .read()
        .store_attachment_content("slot-b", "attachment-a", b"secret")
        .await
        .unwrap_err();
    assert_eq!(error.code, "attachment.not_found");
    repository
        .read()
        .store_attachment_content("slot-a", "attachment-a", b"secret")
        .await
        .unwrap();
    let error = repository
        .read()
        .prepare_attachment_file("slot-b", "attachment-a")
        .await
        .unwrap_err();
    assert_eq!(error.code, "attachment.not_found");
    let prepared = repository
        .read()
        .prepare_attachment_file("slot-a", "attachment-a")
        .await
        .unwrap();
    assert_eq!(tokio::fs::read(prepared.path).await.unwrap(), b"secret");
    let availability: String =
        sqlx::query_scalar("SELECT availability FROM attachments WHERE id = 'attachment-a'")
            .fetch_one(&repository.pool)
            .await
            .unwrap();
    assert_eq!(availability, "available");
}

#[tokio::test]
async fn upsert_message_writes_multiple_attachments_in_one_atomic_unit() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    let mut message = remote_message(1, 7, "Attachments");
    message.attachments = vec![
        crate::core::RemoteAttachment {
            part_index: 1,
            imap_section: None,
            file_name: "one.txt".to_owned(),
            content_type: "text/plain".to_owned(),
            size: 3,
            content_id: None,
        },
        crate::core::RemoteAttachment {
            part_index: 2,
            imap_section: Some("2".to_owned()),
            file_name: "two.txt".to_owned(),
            content_type: "text/plain".to_owned(),
            size: 3,
            content_id: Some("part-two".to_owned()),
        },
    ];

    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();

    let attachment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachments")
        .fetch_one(&repository.pool)
        .await
        .unwrap();
    assert_eq!(attachment_count, 2);
    let section: Option<String> =
        sqlx::query_scalar("SELECT imap_section FROM attachments WHERE part_index = 2")
            .fetch_one(&repository.pool)
            .await
            .unwrap();
    assert_eq!(section.as_deref(), Some("2"));

    sqlx::query("UPDATE attachments SET availability = 'available', size = 2 WHERE part_index = 2")
        .execute(&repository.pool)
        .await
        .unwrap();
    message.attachments[1].size = 4;
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap();
    let size: i64 = sqlx::query_scalar("SELECT size FROM attachments WHERE part_index = 2")
        .fetch_one(&repository.pool)
        .await
        .unwrap();
    assert_eq!(size, 2);
}

#[tokio::test]
async fn ensure_mailbox_inserts_missing_rows_without_touching_existing_ones() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let preliminary = RemoteMailbox {
        name: "INBOX".to_owned(),
        display_name: "INBOX".to_owned(),
        delimiter: None,
        role: MailboxRole::Inbox,
        selectable: true,
        uid_validity: 0,
        uid_next: 0,
        total_count: 0,
        unread_count: 0,
        highest_modseq: None,
    };
    let created = repository
        .sync_sink()
        .ensure_mailbox("slot", &preliminary)
        .await
        .unwrap()
        .expect("first call creates the row");
    assert!(
        repository
            .sync_sink()
            .ensure_mailbox("slot", &preliminary)
            .await
            .unwrap()
            .is_none(),
        "existing row must be left untouched"
    );
    let upserted = repository
        .sync_sink()
        .upsert_mailbox(
            "slot",
            &RemoteMailbox {
                uid_validity: 7,
                uid_next: 8,
                total_count: 3,
                unread_count: 2,
                ..preliminary.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(upserted.id, created.id);
    assert!(repository
        .sync_sink()
        .ensure_mailbox("slot", &preliminary)
        .await
        .unwrap()
        .is_none());
    let (validity, next, total): (i64, i64, i64) =
        sqlx::query_as("SELECT uid_validity, uid_next, total_count FROM mailboxes WHERE id = ?")
            .bind(&created.id)
            .fetch_one(&repository.pool)
            .await
            .unwrap();
    assert_eq!((validity, next, total), (7, 8, 3));
}

#[tokio::test]
async fn stored_uids_returns_every_stored_uid_for_the_mailbox_validity() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "First"))
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(3, 7, "Third"))
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(5, 7, "Fifth"))
        .await
        .unwrap();

    let mut uids = repository
        .sync_sink()
        .stored_uids(&mailbox.id, 7)
        .await
        .unwrap();
    uids.sort_unstable();
    assert_eq!(uids, vec![1, 3, 5]);

    // Locations are uid_validity-scoped: a different validity yields nothing,
    // which is what makes the resumable-sync diff correct after a reset.
    assert!(repository
        .sync_sink()
        .stored_uids(&mailbox.id, 99)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn upsert_message_preserves_pending_read_flag_against_stale_server_state() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    let outcome = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
        .await
        .unwrap();
    assert!(outcome.is_new_location);

    // Opening the message locally marks it read and queues a pending set_read.
    repository
        .operations()
        .queue_set_read(
            "slot",
            &mailbox.id,
            std::slice::from_ref(&outcome.message_id),
            true,
        )
        .await
        .unwrap();

    // A later body fetch re-upserts the message carrying the server's stale
    // (still-unread) flags. This must not clobber the pending read.
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
        .await
        .unwrap();
    let item = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 50)
        .await
        .unwrap()
        .items
        .remove(0);
    assert!(!item.unread, "pending set_read must survive a stale upsert");

    // With no pending op in flight, upsert flags apply normally again.
    sqlx::query("DELETE FROM pending_operations")
        .execute(&repository.pool)
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Unread"))
        .await
        .unwrap();
    let item = repository
        .read()
        .list_messages("slot", &mailbox.id, None, 50)
        .await
        .unwrap()
        .items
        .remove(0);
    assert!(
        item.unread,
        "without a pending op the upsert flag is applied"
    );
}

#[tokio::test]
async fn upsert_message_rolls_back_all_database_rows_when_attachment_write_fails() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    sqlx::query(
        "CREATE TRIGGER fail_attachment_insert BEFORE INSERT ON attachments \
         BEGIN SELECT RAISE(FAIL, 'forced attachment failure'); END",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    let mut message = remote_message(1, 7, "Atomic");
    message.attachments = vec![crate::core::RemoteAttachment {
        part_index: 1,
        imap_section: None,
        file_name: "failure.txt".to_owned(),
        content_type: "text/plain".to_owned(),
        size: 7,
        content_id: None,
    }];

    let error = repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &message)
        .await
        .unwrap_err();
    assert_eq!(error.code, "storage.attachment_write_failed");

    for (table, query) in [
        ("messages", "SELECT COUNT(*) FROM messages"),
        (
            "message_locations",
            "SELECT COUNT(*) FROM message_locations",
        ),
        ("message_bodies", "SELECT COUNT(*) FROM message_bodies"),
        ("attachments", "SELECT COUNT(*) FROM attachments"),
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .fetch_one(&repository.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} must not retain a partial upsert");
    }
}

#[tokio::test]
async fn reconcile_mailbox_deletes_missing_locations_but_preserves_pending_work() {
    let (_directory, repository, mailbox) = repository_with_mailbox(7).await;
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(1, 7, "Pending"))
        .await
        .unwrap();
    repository
        .sync_sink()
        .upsert_message("slot", &mailbox.id, &remote_message(2, 7, "Removed"))
        .await
        .unwrap();
    let pending_message_id: String = sqlx::query_scalar(
        "SELECT message_id FROM message_locations WHERE mailbox_id = ? AND uid = 1",
    )
    .bind(&mailbox.id)
    .fetch_one(&repository.pool)
    .await
    .unwrap();
    repository
        .operations()
        .queue_set_read(
            "slot",
            &mailbox.id,
            std::slice::from_ref(&pending_message_id),
            true,
        )
        .await
        .unwrap();

    repository
        .sync_sink()
        .reconcile_mailbox(&mailbox.id, 7, Some(12), &[])
        .await
        .unwrap();

    let remaining_uids = sqlx::query_scalar::<_, i64>(
        "SELECT uid FROM message_locations WHERE mailbox_id = ? ORDER BY uid",
    )
    .bind(&mailbox.id)
    .fetch_all(&repository.pool)
    .await
    .unwrap();
    assert_eq!(remaining_uids, vec![1]);
}

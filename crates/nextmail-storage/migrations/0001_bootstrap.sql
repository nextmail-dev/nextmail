CREATE TABLE IF NOT EXISTS schema_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS account_slots (
    id TEXT PRIMARY KEY NOT NULL,
    created_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_metadata (key, value)
VALUES ('data_format_version', '1');

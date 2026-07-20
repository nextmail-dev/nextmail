CREATE TABLE mail_templates (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    subject_template TEXT NOT NULL DEFAULT '',
    editor_json TEXT NOT NULL,
    html TEXT NOT NULL,
    plain_text TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (length(trim(name)) BETWEEN 1 AND 80)
);

CREATE INDEX mail_templates_scope_name_idx
    ON mail_templates(account_slot_id, name COLLATE NOCASE, id);

CREATE TABLE mail_signatures (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    editor_json TEXT NOT NULL,
    html TEXT NOT NULL,
    plain_text TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (length(trim(name)) BETWEEN 1 AND 80)
);

CREATE INDEX mail_signatures_scope_name_idx
    ON mail_signatures(account_slot_id, name COLLATE NOCASE, id);

UPDATE schema_metadata SET value = '8' WHERE key = 'data_format_version';

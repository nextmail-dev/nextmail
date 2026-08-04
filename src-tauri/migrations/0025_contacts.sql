CREATE TABLE contacts (
    id TEXT PRIMARY KEY NOT NULL,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    normalized_email TEXT NOT NULL,
    email TEXT NOT NULL,
    name TEXT NOT NULL,
    name_source TEXT NOT NULL DEFAULT 'auto' CHECK (name_source IN ('auto', 'manual')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    UNIQUE(account_slot_id, normalized_email)
);

CREATE INDEX contacts_account_name_idx
ON contacts(account_slot_id, name COLLATE NOCASE, normalized_email, id);

CREATE TABLE message_contacts (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    contact_id TEXT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('from', 'sender', 'reply_to', 'to', 'cc', 'bcc')),
    PRIMARY KEY(message_id, contact_id, role)
);

CREATE INDEX message_contacts_contact_message_idx
ON message_contacts(contact_id, message_id);

CREATE TABLE contact_backfill_state (
    account_slot_id TEXT PRIMARY KEY NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    last_message_rowid INTEGER NOT NULL DEFAULT 0,
    completed_at INTEGER
);

UPDATE schema_metadata SET value = '25' WHERE key = 'data_format_version';

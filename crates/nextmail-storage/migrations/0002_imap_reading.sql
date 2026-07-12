CREATE TABLE IF NOT EXISTS account_sync_settings (
    account_slot_id TEXT PRIMARY KEY NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    sync_policy TEXT NOT NULL DEFAULT 'days90',
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS mailboxes (
    id TEXT PRIMARY KEY NOT NULL,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    remote_name TEXT NOT NULL,
    delimiter TEXT,
    role TEXT NOT NULL DEFAULT 'other',
    selectable INTEGER NOT NULL DEFAULT 1,
    uid_validity INTEGER NOT NULL DEFAULT 0,
    uid_next INTEGER NOT NULL DEFAULT 0,
    last_uid INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER,
    total_count INTEGER NOT NULL DEFAULT 0,
    unread_count INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0,
    UNIQUE(account_slot_id, remote_name)
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    subject TEXT NOT NULL DEFAULT '',
    from_json TEXT NOT NULL DEFAULT '[]',
    to_json TEXT NOT NULL DEFAULT '[]',
    cc_json TEXT NOT NULL DEFAULT '[]',
    received_at INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL DEFAULT '',
    rfc822_size INTEGER NOT NULL DEFAULT 0,
    message_id TEXT,
    references_json TEXT NOT NULL DEFAULT '[]',
    in_reply_to TEXT,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    raw_content_hash TEXT,
    body_availability TEXT NOT NULL DEFAULT 'missing',
    remote_images_blocked INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_messages_account_received
ON messages(account_slot_id, received_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_messages_dedup
ON messages(account_slot_id, message_id, rfc822_size, received_at);

CREATE TABLE IF NOT EXISTS message_locations (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    mailbox_id TEXT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    uid_validity INTEGER NOT NULL,
    flags_json TEXT NOT NULL DEFAULT '[]',
    unread INTEGER NOT NULL DEFAULT 0,
    flagged INTEGER NOT NULL DEFAULT 0,
    internal_date INTEGER NOT NULL DEFAULT 0,
    modseq INTEGER,
    UNIQUE(mailbox_id, uid_validity, uid)
);

CREATE INDEX IF NOT EXISTS idx_locations_mailbox_date
ON message_locations(mailbox_id, internal_date DESC, message_id DESC);

CREATE TABLE IF NOT EXISTS message_bodies (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    plain_text TEXT,
    safe_html TEXT,
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    part_index INTEGER NOT NULL,
    file_name TEXT NOT NULL DEFAULT '',
    content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    size INTEGER NOT NULL DEFAULT 0,
    content_id TEXT,
    availability TEXT NOT NULL DEFAULT 'missing',
    content_hash TEXT,
    UNIQUE(message_id, part_index)
);

CREATE TABLE IF NOT EXISTS sync_states (
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    mailbox_id TEXT REFERENCES mailboxes(id) ON DELETE CASCADE,
    phase TEXT NOT NULL DEFAULT 'idle',
    last_success_at INTEGER,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_code TEXT,
    revision INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(account_slot_id, mailbox_id)
);

CREATE TABLE IF NOT EXISTS remote_image_permissions (
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    sender_key TEXT NOT NULL,
    allowed INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(account_slot_id, sender_key)
);

UPDATE schema_metadata SET value = '2' WHERE key = 'data_format_version';

ALTER TABLE message_locations ADD COLUMN local_hidden INTEGER NOT NULL DEFAULT 0;
ALTER TABLE drafts ADD COLUMN source_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX drafts_source_message_idx ON drafts(source_message_id) WHERE source_message_id IS NOT NULL;

CREATE TABLE pending_operations (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'set_read', 'set_flagged', 'copy', 'move', 'delete', 'append_sent', 'append_draft'
    )),
    message_id TEXT REFERENCES messages(id) ON DELETE CASCADE,
    source_mailbox_id TEXT REFERENCES mailboxes(id) ON DELETE CASCADE,
    destination_mailbox_id TEXT REFERENCES mailboxes(id) ON DELETE SET NULL,
    uid INTEGER,
    uid_validity INTEGER,
    payload_json TEXT NOT NULL DEFAULT '{}',
    base_modseq INTEGER,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'running', 'retry_wait', 'needs_reconcile', 'succeeded', 'failed'
    )),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER,
    error_code TEXT,
    cleanup_pending INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX pending_operations_ready_idx
ON pending_operations(account_slot_id, status, next_attempt_at, created_at);

CREATE INDEX pending_operations_message_idx
ON pending_operations(message_id, source_mailbox_id, status);

CREATE TABLE mailbox_role_overrides (
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('sent', 'drafts', 'trash', 'archive')),
    mailbox_id TEXT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(account_slot_id, role),
    UNIQUE(account_slot_id, mailbox_id)
);

UPDATE schema_metadata SET value = '5' WHERE key = 'data_format_version';

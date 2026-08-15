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

ALTER TABLE mailboxes ADD COLUMN display_name TEXT NOT NULL DEFAULT '';

UPDATE mailboxes
SET display_name = remote_name
WHERE display_name = '';

-- Re-fetch headers and bodies once so values previously decoded without
-- multi-byte charset support are replaced from their original IMAP source.
UPDATE mailboxes SET last_uid = 0;
UPDATE messages
SET body_availability = 'missing', revision = revision + 1;
DELETE FROM message_bodies;

UPDATE schema_metadata SET value = '3' WHERE key = 'data_format_version';

CREATE TABLE drafts (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'editing' CHECK (status IN ('editing', 'queued', 'sent')),
    to_json TEXT NOT NULL DEFAULT '[]',
    cc_json TEXT NOT NULL DEFAULT '[]',
    bcc_json TEXT NOT NULL DEFAULT '[]',
    subject TEXT NOT NULL DEFAULT '',
    editor_json TEXT NOT NULL DEFAULT '{"type":"doc","content":[{"type":"paragraph"}]}',
    html TEXT NOT NULL DEFAULT '<p></p>',
    plain_text TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX drafts_account_updated_idx ON drafts(account_slot_id, updated_at DESC);

CREATE TABLE draft_attachments (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(draft_id, id)
);

CREATE INDEX draft_attachments_draft_idx ON draft_attachments(draft_id, sort_order);

CREATE TABLE send_jobs (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL UNIQUE REFERENCES drafts(id) ON DELETE RESTRICT,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    mime_hash TEXT NOT NULL,
    envelope_recipients_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'sent', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER,
    error_code TEXT,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sent_at INTEGER
);

CREATE INDEX send_jobs_pending_idx ON send_jobs(status, next_attempt_at, created_at);

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

ALTER TABLE drafts ADD COLUMN related_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL;
ALTER TABLE drafts ADD COLUMN in_reply_to TEXT;
ALTER TABLE drafts ADD COLUMN references_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX drafts_related_message_idx ON drafts(related_message_id) WHERE related_message_id IS NOT NULL;

UPDATE schema_metadata SET value = '6' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '7' WHERE key = 'data_format_version';

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

CREATE TABLE composition_scene_rules (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    scene TEXT NOT NULL CHECK (scene IN ('new', 'reply', 'reply_all', 'forward')),
    template_id TEXT REFERENCES mail_templates(id) ON DELETE RESTRICT,
    signature_id TEXT REFERENCES mail_signatures(id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX composition_scene_rules_global_scene_idx
    ON composition_scene_rules(scene)
    WHERE account_slot_id IS NULL;

CREATE UNIQUE INDEX composition_scene_rules_account_scene_idx
    ON composition_scene_rules(account_slot_id, scene)
    WHERE account_slot_id IS NOT NULL;

UPDATE schema_metadata SET value = '9' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '10' WHERE key = 'data_format_version';

CREATE TABLE IF NOT EXISTS message_links (
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

DROP TABLE IF EXISTS message_links;

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

ALTER TABLE draft_attachments ADD COLUMN content_id TEXT;
ALTER TABLE draft_attachments ADD COLUMN is_inline INTEGER NOT NULL DEFAULT 0 CHECK (is_inline IN (0, 1));

CREATE UNIQUE INDEX draft_attachments_inline_cid_idx
ON draft_attachments(draft_id, content_id)
WHERE content_id IS NOT NULL;

UPDATE schema_metadata SET value = '13' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '14' WHERE key = 'data_format_version';

CREATE VIRTUAL TABLE message_search USING fts5(
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

ALTER TABLE account_sync_settings
ADD COLUMN download_non_inbox_bodies INTEGER NOT NULL DEFAULT 0
CHECK (download_non_inbox_bodies IN (0, 1));

ALTER TABLE drafts
ADD COLUMN discard_if_untouched INTEGER NOT NULL DEFAULT 0
CHECK (discard_if_untouched IN (0, 1));

ALTER TABLE drafts
ADD COLUMN user_edited INTEGER NOT NULL DEFAULT 0
CHECK (user_edited IN (0, 1));

UPDATE schema_metadata SET value = '16' WHERE key = 'data_format_version';

CREATE TABLE signature_preferences (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    default_signature_id TEXT REFERENCES mail_signatures(id) ON DELETE SET NULL,
    auto_insert INTEGER NOT NULL DEFAULT 1 CHECK (auto_insert IN (0, 1)),
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX signature_preferences_global_idx
    ON signature_preferences((1))
    WHERE account_slot_id IS NULL;

CREATE UNIQUE INDEX signature_preferences_account_idx
    ON signature_preferences(account_slot_id)
    WHERE account_slot_id IS NOT NULL;

WITH ranked_rules AS (
    SELECT
        account_slot_id,
        signature_id,
        ROW_NUMBER() OVER (
            PARTITION BY account_slot_id
            ORDER BY CASE scene
                WHEN 'new' THEN 0
                WHEN 'reply' THEN 1
                WHEN 'reply_all' THEN 2
                ELSE 3
            END
        ) AS priority
    FROM composition_scene_rules
    WHERE signature_id IS NOT NULL
)
INSERT INTO signature_preferences(
    id, account_slot_id, default_signature_id, auto_insert,
    revision, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))), account_slot_id, signature_id, 1,
    1, unixepoch(), unixepoch()
FROM ranked_rules
WHERE priority = 1;

WITH ranked_signatures AS (
    SELECT
        id,
        account_slot_id,
        ROW_NUMBER() OVER (
            PARTITION BY account_slot_id
            ORDER BY created_at, id
        ) AS priority
    FROM mail_signatures
)
INSERT INTO signature_preferences(
    id, account_slot_id, default_signature_id, auto_insert,
    revision, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))), signatures.account_slot_id, signatures.id, 1,
    1, unixepoch(), unixepoch()
FROM ranked_signatures signatures
WHERE signatures.priority = 1
  AND NOT EXISTS (
      SELECT 1
      FROM signature_preferences preferences
      WHERE preferences.account_slot_id IS signatures.account_slot_id
  );

UPDATE composition_scene_rules SET signature_id = NULL WHERE signature_id IS NOT NULL;

UPDATE schema_metadata SET value = '17' WHERE key = 'data_format_version';

ALTER TABLE account_slots ADD COLUMN notification_baseline_at INTEGER;

UPDATE account_slots
SET notification_baseline_at = (
    SELECT MAX(last_synced_at)
    FROM mailboxes
    WHERE mailboxes.account_slot_id = account_slots.id
)
WHERE EXISTS (
    SELECT 1
    FROM mailboxes
    WHERE mailboxes.account_slot_id = account_slots.id
      AND mailboxes.last_synced_at IS NOT NULL
);

UPDATE schema_metadata SET value = '18' WHERE key = 'data_format_version';

ALTER TABLE account_sync_settings
ADD COLUMN sync_interval_minutes INTEGER NOT NULL DEFAULT 1
CHECK (sync_interval_minutes IN (0, 1, 5, 10));

UPDATE schema_metadata SET value = '19' WHERE key = 'data_format_version';

ALTER TABLE mailboxes
ADD COLUMN local_sort_order INTEGER;

UPDATE schema_metadata SET value = '20' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '21' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '22' WHERE key = 'data_format_version';

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

UPDATE schema_metadata SET value = '23' WHERE key = 'data_format_version';

ALTER TABLE account_sync_settings
ADD COLUMN download_full_messages INTEGER NOT NULL DEFAULT 0
CHECK (download_full_messages IN (0, 1));

UPDATE schema_metadata SET value = '24' WHERE key = 'data_format_version';

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

ALTER TABLE mail_templates ADD COLUMN recipients_json TEXT;

UPDATE schema_metadata SET value = '26' WHERE key = 'data_format_version';

ALTER TABLE attachments ADD COLUMN imap_section TEXT;

UPDATE messages
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

UPDATE messages
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

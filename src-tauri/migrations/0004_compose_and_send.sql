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

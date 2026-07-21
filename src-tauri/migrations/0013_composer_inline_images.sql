ALTER TABLE draft_attachments ADD COLUMN content_id TEXT;
ALTER TABLE draft_attachments ADD COLUMN is_inline INTEGER NOT NULL DEFAULT 0 CHECK (is_inline IN (0, 1));

CREATE UNIQUE INDEX draft_attachments_inline_cid_idx
ON draft_attachments(draft_id, content_id)
WHERE content_id IS NOT NULL;

UPDATE schema_metadata SET value = '13' WHERE key = 'data_format_version';

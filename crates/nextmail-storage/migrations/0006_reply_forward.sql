ALTER TABLE drafts ADD COLUMN related_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL;
ALTER TABLE drafts ADD COLUMN in_reply_to TEXT;
ALTER TABLE drafts ADD COLUMN references_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX drafts_related_message_idx ON drafts(related_message_id) WHERE related_message_id IS NOT NULL;

UPDATE schema_metadata SET value = '6' WHERE key = 'data_format_version';

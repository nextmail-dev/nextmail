CREATE INDEX IF NOT EXISTS idx_locations_message_date
ON message_locations(message_id, internal_date DESC, mailbox_id DESC);

UPDATE schema_metadata SET value = '32' WHERE key = 'data_format_version';

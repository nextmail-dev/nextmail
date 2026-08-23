ALTER TABLE messages
ADD COLUMN high_priority INTEGER NOT NULL DEFAULT 0 CHECK (high_priority IN (0, 1));

UPDATE schema_metadata SET value = '31' WHERE key = 'data_format_version';

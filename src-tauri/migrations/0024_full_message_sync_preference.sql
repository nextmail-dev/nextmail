ALTER TABLE account_sync_settings
ADD COLUMN download_full_messages INTEGER NOT NULL DEFAULT 0
CHECK (download_full_messages IN (0, 1));

UPDATE schema_metadata SET value = '24' WHERE key = 'data_format_version';

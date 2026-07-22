ALTER TABLE account_sync_settings
ADD COLUMN sync_interval_minutes INTEGER NOT NULL DEFAULT 1
CHECK (sync_interval_minutes IN (0, 1, 5, 10));

UPDATE schema_metadata SET value = '19' WHERE key = 'data_format_version';

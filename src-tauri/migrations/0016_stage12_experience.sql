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

ALTER TABLE mailboxes
ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1));

UPDATE mailboxes
SET is_favorite = 1
WHERE role = 'inbox';

UPDATE schema_metadata SET value = '30' WHERE key = 'data_format_version';

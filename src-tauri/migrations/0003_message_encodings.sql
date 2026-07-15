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

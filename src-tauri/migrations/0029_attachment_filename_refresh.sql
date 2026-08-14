UPDATE messages
SET body_availability = 'missing',
    revision = revision + 1
WHERE id IN (
    SELECT attachment.message_id
    FROM attachments attachment
    WHERE substr(attachment.file_name, 1, 2) = '=?'
);

DELETE FROM message_bodies
WHERE message_id IN (
    SELECT attachment.message_id
    FROM attachments attachment
    WHERE substr(attachment.file_name, 1, 2) = '=?'
);

UPDATE schema_metadata SET value = '29' WHERE key = 'data_format_version';

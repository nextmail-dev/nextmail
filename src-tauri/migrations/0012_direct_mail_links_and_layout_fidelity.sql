DROP TABLE IF EXISTS message_links;

UPDATE messages
SET body_availability = 'missing',
    remote_images_blocked = 0,
    revision = revision + 1
WHERE id IN (
    SELECT message_id
    FROM message_bodies
    WHERE safe_html IS NOT NULL
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL;

UPDATE schema_metadata SET value = '12' WHERE key = 'data_format_version';

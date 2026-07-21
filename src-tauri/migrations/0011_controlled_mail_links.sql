CREATE TABLE IF NOT EXISTS message_links (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    target_url TEXT NOT NULL,
    UNIQUE(message_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_message_links_message
ON message_links(message_id, ordinal);

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

UPDATE schema_metadata SET value = '11' WHERE key = 'data_format_version';

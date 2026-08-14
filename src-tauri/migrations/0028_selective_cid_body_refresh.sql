UPDATE messages
SET body_availability = 'missing',
    remote_images_blocked = 0,
    revision = revision + 1
WHERE id IN (
    SELECT body.message_id
    FROM message_bodies body
    WHERE body.safe_html IS NOT NULL
      AND EXISTS (
          SELECT 1
          FROM attachments attachment
          WHERE attachment.message_id = body.message_id
            AND attachment.content_id IS NOT NULL
            AND lower(attachment.content_type) = 'application/octet-stream'
      )
);

DELETE FROM message_bodies
WHERE safe_html IS NOT NULL
  AND EXISTS (
      SELECT 1
      FROM attachments attachment
      WHERE attachment.message_id = message_bodies.message_id
        AND attachment.content_id IS NOT NULL
        AND lower(attachment.content_type) = 'application/octet-stream'
  );

UPDATE schema_metadata SET value = '28' WHERE key = 'data_format_version';

CREATE VIRTUAL TABLE message_search USING fts5(
    message_id UNINDEXED,
    account_slot_id UNINDEXED,
    subject,
    addresses,
    preview,
    body,
    attachment_names,
    tokenize = 'trigram case_sensitive 0 remove_diacritics 1'
);

INSERT INTO message_search(
    message_id,
    account_slot_id,
    subject,
    addresses,
    preview,
    body,
    attachment_names
)
SELECT
    m.id,
    m.account_slot_id,
    m.subject,
    m.from_json || ' ' || m.to_json || ' ' || m.cc_json,
    m.preview,
    COALESCE(b.plain_text, ''),
    COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = m.id
    ), '')
FROM messages m
LEFT JOIN message_bodies b ON b.message_id = m.id;

CREATE TRIGGER message_search_messages_ai
AFTER INSERT ON messages
BEGIN
    INSERT INTO message_search(
        message_id,
        account_slot_id,
        subject,
        addresses,
        preview,
        body,
        attachment_names
    ) VALUES (
        NEW.id,
        NEW.account_slot_id,
        NEW.subject,
        NEW.from_json || ' ' || NEW.to_json || ' ' || NEW.cc_json,
        NEW.preview,
        '',
        ''
    );
END;

CREATE TRIGGER message_search_messages_au
AFTER UPDATE OF account_slot_id, subject, from_json, to_json, cc_json, preview ON messages
BEGIN
    DELETE FROM message_search WHERE message_id = OLD.id;
    INSERT INTO message_search(
        message_id,
        account_slot_id,
        subject,
        addresses,
        preview,
        body,
        attachment_names
    )
    SELECT
        NEW.id,
        NEW.account_slot_id,
        NEW.subject,
        NEW.from_json || ' ' || NEW.to_json || ' ' || NEW.cc_json,
        NEW.preview,
        COALESCE(b.plain_text, ''),
        COALESCE((
            SELECT group_concat(a.file_name, ' ')
            FROM attachments a
            WHERE a.message_id = NEW.id
        ), '')
    FROM (SELECT 1)
    LEFT JOIN message_bodies b ON b.message_id = NEW.id;
END;

CREATE TRIGGER message_search_messages_ad
AFTER DELETE ON messages
BEGIN
    DELETE FROM message_search WHERE message_id = OLD.id;
END;

CREATE TRIGGER message_search_bodies_ai
AFTER INSERT ON message_bodies
BEGIN
    UPDATE message_search
    SET body = COALESCE(NEW.plain_text, '')
    WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_bodies_au
AFTER UPDATE OF message_id, plain_text ON message_bodies
BEGIN
    UPDATE message_search SET body = '' WHERE message_id = OLD.message_id;
    UPDATE message_search
    SET body = COALESCE(NEW.plain_text, '')
    WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_bodies_ad
AFTER DELETE ON message_bodies
BEGIN
    UPDATE message_search SET body = '' WHERE message_id = OLD.message_id;
END;

CREATE TRIGGER message_search_attachments_ai
AFTER INSERT ON attachments
BEGIN
    UPDATE message_search
    SET attachment_names = COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = NEW.message_id
    ), '')
    WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_attachments_au
AFTER UPDATE OF message_id, file_name ON attachments
BEGIN
    UPDATE message_search
    SET attachment_names = COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = OLD.message_id
    ), '')
    WHERE message_id = OLD.message_id;
    UPDATE message_search
    SET attachment_names = COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = NEW.message_id
    ), '')
    WHERE message_id = NEW.message_id;
END;

CREATE TRIGGER message_search_attachments_ad
AFTER DELETE ON attachments
BEGIN
    UPDATE message_search
    SET attachment_names = COALESCE((
        SELECT group_concat(a.file_name, ' ')
        FROM attachments a
        WHERE a.message_id = OLD.message_id
    ), '')
    WHERE message_id = OLD.message_id;
END;

UPDATE schema_metadata SET value = '15' WHERE key = 'data_format_version';

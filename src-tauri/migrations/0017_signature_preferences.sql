CREATE TABLE signature_preferences (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    default_signature_id TEXT REFERENCES mail_signatures(id) ON DELETE SET NULL,
    auto_insert INTEGER NOT NULL DEFAULT 1 CHECK (auto_insert IN (0, 1)),
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX signature_preferences_global_idx
    ON signature_preferences((1))
    WHERE account_slot_id IS NULL;

CREATE UNIQUE INDEX signature_preferences_account_idx
    ON signature_preferences(account_slot_id)
    WHERE account_slot_id IS NOT NULL;

WITH ranked_rules AS (
    SELECT
        account_slot_id,
        signature_id,
        ROW_NUMBER() OVER (
            PARTITION BY account_slot_id
            ORDER BY CASE scene
                WHEN 'new' THEN 0
                WHEN 'reply' THEN 1
                WHEN 'reply_all' THEN 2
                ELSE 3
            END
        ) AS priority
    FROM composition_scene_rules
    WHERE signature_id IS NOT NULL
)
INSERT INTO signature_preferences(
    id, account_slot_id, default_signature_id, auto_insert,
    revision, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))), account_slot_id, signature_id, 1,
    1, unixepoch(), unixepoch()
FROM ranked_rules
WHERE priority = 1;

WITH ranked_signatures AS (
    SELECT
        id,
        account_slot_id,
        ROW_NUMBER() OVER (
            PARTITION BY account_slot_id
            ORDER BY created_at, id
        ) AS priority
    FROM mail_signatures
)
INSERT INTO signature_preferences(
    id, account_slot_id, default_signature_id, auto_insert,
    revision, created_at, updated_at
)
SELECT
    lower(hex(randomblob(16))), signatures.account_slot_id, signatures.id, 1,
    1, unixepoch(), unixepoch()
FROM ranked_signatures signatures
WHERE signatures.priority = 1
  AND NOT EXISTS (
      SELECT 1
      FROM signature_preferences preferences
      WHERE preferences.account_slot_id IS signatures.account_slot_id
  );

UPDATE composition_scene_rules SET signature_id = NULL WHERE signature_id IS NOT NULL;

UPDATE schema_metadata SET value = '17' WHERE key = 'data_format_version';

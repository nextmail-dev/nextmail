CREATE TABLE composition_scene_rules (
    id TEXT PRIMARY KEY,
    account_slot_id TEXT REFERENCES account_slots(id) ON DELETE CASCADE,
    scene TEXT NOT NULL CHECK (scene IN ('new', 'reply', 'reply_all', 'forward')),
    template_id TEXT REFERENCES mail_templates(id) ON DELETE RESTRICT,
    signature_id TEXT REFERENCES mail_signatures(id) ON DELETE RESTRICT,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX composition_scene_rules_global_scene_idx
    ON composition_scene_rules(scene)
    WHERE account_slot_id IS NULL;

CREATE UNIQUE INDEX composition_scene_rules_account_scene_idx
    ON composition_scene_rules(account_slot_id, scene)
    WHERE account_slot_id IS NOT NULL;

UPDATE schema_metadata SET value = '9' WHERE key = 'data_format_version';

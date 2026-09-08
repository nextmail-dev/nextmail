CREATE TABLE contact_groups (
    id TEXT PRIMARY KEY NOT NULL,
    account_slot_id TEXT NOT NULL REFERENCES account_slots(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    UNIQUE(account_slot_id, normalized_name),
    UNIQUE(account_slot_id, id)
);

CREATE UNIQUE INDEX contacts_account_id_idx ON contacts(account_slot_id, id);

CREATE TABLE contact_group_members (
    account_slot_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    contact_id TEXT NOT NULL,
    PRIMARY KEY(account_slot_id, group_id, contact_id),
    FOREIGN KEY(account_slot_id, group_id) REFERENCES contact_groups(account_slot_id, id) ON DELETE CASCADE,
    FOREIGN KEY(account_slot_id, contact_id) REFERENCES contacts(account_slot_id, id) ON DELETE CASCADE
);

CREATE INDEX contact_group_members_contact_idx ON contact_group_members(account_slot_id, contact_id);

UPDATE schema_metadata SET value = '33' WHERE key = 'data_format_version';

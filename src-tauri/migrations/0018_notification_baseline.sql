ALTER TABLE account_slots ADD COLUMN notification_baseline_at INTEGER;

UPDATE account_slots
SET notification_baseline_at = (
    SELECT MAX(last_synced_at)
    FROM mailboxes
    WHERE mailboxes.account_slot_id = account_slots.id
)
WHERE EXISTS (
    SELECT 1
    FROM mailboxes
    WHERE mailboxes.account_slot_id = account_slots.id
      AND mailboxes.last_synced_at IS NOT NULL
);

UPDATE schema_metadata SET value = '18' WHERE key = 'data_format_version';

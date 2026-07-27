ALTER TABLE mailboxes
ADD COLUMN local_sort_order INTEGER;

UPDATE schema_metadata SET value = '20' WHERE key = 'data_format_version';

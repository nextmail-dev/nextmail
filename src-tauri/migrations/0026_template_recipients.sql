ALTER TABLE mail_templates ADD COLUMN recipients_json TEXT;

UPDATE schema_metadata SET value = '26' WHERE key = 'data_format_version';

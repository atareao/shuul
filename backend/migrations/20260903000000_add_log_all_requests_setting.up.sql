-- Add log_all_requests setting (default: 'all')
INSERT INTO settings (key, value) VALUES ('log_all_requests', 'all')
ON CONFLICT (key) DO UPDATE SET value = excluded.value;
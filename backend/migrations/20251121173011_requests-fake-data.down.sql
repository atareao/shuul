-- Add down migration script here
DELETE FROM requests
WHERE id IN (
    SELECT id FROM requests
    ORDER BY created_at DESC
    LIMIT 100
);

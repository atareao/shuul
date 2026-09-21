-- Add UNIQUE index on (ip_address, rule_id) for bans table
-- and clean up legacy duplicates (keep only the most recent per pair).

-- Step 1: Mark duplicates as expired (keep only the most recent per ip_address + rule_id)
-- Uses a subquery to find the rowid of the most recent ban per (ip_address, COALESCE(rule_id, 0))
UPDATE bans
SET expired = 1
WHERE expired = 0
  AND rowid NOT IN (
    SELECT MIN(rowid) FROM (
      SELECT rowid, ROW_NUMBER() OVER (
        PARTITION BY ip_address, COALESCE(rule_id, 0)
        ORDER BY banned_at DESC
      ) AS rn
      FROM bans
      WHERE expired = 0
    ) WHERE rn = 1
  );

-- Step 2: Add UNIQUE index to prevent future duplicates
-- Note: SQLite treats NULL as distinct in UNIQUE constraints, so
-- multiple rows with rule_id=NULL are still allowed (by design).
CREATE UNIQUE INDEX IF NOT EXISTS idx_bans_ip_rule_id_active
ON bans (ip_address, rule_id)
WHERE expired = 0;
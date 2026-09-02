-- Remove store column from rules (no longer needed, audit via logs + stats in memory)
ALTER TABLE rules DROP COLUMN store;

-- Stats cache table for persisting StatsCollector snapshots
CREATE TABLE IF NOT EXISTS stats_cache (
    id INTEGER PRIMARY KEY DEFAULT 1,
    snapshot JSON NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
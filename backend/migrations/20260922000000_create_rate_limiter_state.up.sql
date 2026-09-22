-- Create rate_limiter_state table for persisting rate limiter ring buffers.
--
-- Stores the circular timestamp buffer state per (profile_id, ip_address)
-- so that rate limiting state survives server restarts.
--
-- The timestamps column stores a JSON array of epoch-millisecond values.
-- head, count, and capacity describe the ring buffer position.
--
-- See: openspec/changes/ban-system-hardening/specs/rate-limiter/spec.md

CREATE TABLE IF NOT EXISTS rate_limiter_state (
    profile_id INTEGER NOT NULL,
    ip_address TEXT NOT NULL,
    timestamps TEXT NOT NULL,       -- JSON array of epoch millis
    head INTEGER NOT NULL,
    count INTEGER NOT NULL,
    capacity INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (profile_id, ip_address)
);
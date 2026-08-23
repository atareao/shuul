-- Add rate limiting columns to rules table
ALTER TABLE rules
    ADD COLUMN IF NOT EXISTS rate_limit_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS max_retry INT NOT NULL DEFAULT 5,
    ADD COLUMN IF NOT EXISTS find_time_seconds INT NOT NULL DEFAULT 600,
    ADD COLUMN IF NOT EXISTS ban_time_seconds INT NOT NULL DEFAULT 3600,
    ADD COLUMN IF NOT EXISTS bantime_increment BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS bantime_multipliers INT[] NOT NULL DEFAULT '{1,2,4,8}',
    ADD COLUMN IF NOT EXISTS bantime_maxtime_seconds INT NOT NULL DEFAULT 604800,
    ADD COLUMN IF NOT EXISTS ban_count_decay_days INT NOT NULL DEFAULT 30,
    ADD COLUMN IF NOT EXISTS ignoreip TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS webhook TEXT;

-- Create bans table
CREATE TABLE IF NOT EXISTS bans (
    id SERIAL PRIMARY KEY,
    ip_address TEXT NOT NULL,
    rule_id INT REFERENCES rules(id) ON DELETE SET NULL,
    jail_name TEXT NOT NULL,
    banned_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ban_duration_seconds INT NOT NULL,
    escalation_level INT NOT NULL DEFAULT 0,
    reason TEXT,
    expired BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bans_ip_address ON bans(ip_address);
CREATE INDEX IF NOT EXISTS idx_bans_expired ON bans(expired);
-- Shuul Initial Schema
-- Drops all existing tables and creates the new normalized schema
-- with rate_limit_profiles as a separate table.
--
-- This is the ONLY migration file. All previous incremental migrations
-- have been consolidated here.

-- Drop existing tables in reverse dependency order
DROP TABLE IF EXISTS requests CASCADE;
DROP TABLE IF EXISTS bans CASCADE;
DROP TABLE IF EXISTS rules CASCADE;
DROP TABLE IF EXISTS rate_limit_profiles CASCADE;
DROP TABLE IF EXISTS settings CASCADE;

-- Rate limit profiles (standalone, reusable config)
CREATE TABLE rate_limit_profiles (
    id SERIAL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    max_retry INT NOT NULL DEFAULT 5,
    find_time_seconds INT NOT NULL DEFAULT 600,
    ban_time_seconds INT NOT NULL DEFAULT 3600,
    bantime_increment BOOL NOT NULL DEFAULT false,
    bantime_multipliers INT[] NOT NULL DEFAULT '{1,2,4,8}',
    bantime_maxtime_seconds INT NOT NULL DEFAULT 604800,
    ban_count_decay_days INT NOT NULL DEFAULT 30,
    fail_codes INT[] NOT NULL DEFAULT '{401,403,404}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rules (matching + action + FK to rate_limit_profiles)
CREATE TABLE rules (
    id SERIAL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    weight INT NOT NULL DEFAULT 100,
    mode VARCHAR NOT NULL DEFAULT 'log_only',
    allow BOOL NOT NULL DEFAULT true,
    store BOOL NOT NULL DEFAULT true,
    ip_address VARCHAR,
    protocol VARCHAR,
    fqdn VARCHAR,
    path VARCHAR,
    query VARCHAR,
    city_name VARCHAR,
    country_name VARCHAR,
    country_code VARCHAR,
    user_agent VARCHAR,
    method VARCHAR,
    referer VARCHAR,
    content_type VARCHAR,
    accept_language VARCHAR,
    x_request_id VARCHAR,
    rate_limit_profile_id INT REFERENCES rate_limit_profiles(id) ON DELETE SET NULL,
    active BOOL NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Bans (persistent storage, complements in-memory BanManager)
CREATE TABLE bans (
    id SERIAL PRIMARY KEY,
    ip_address VARCHAR NOT NULL,
    rule_id INT REFERENCES rules(id) ON DELETE SET NULL,
    reason TEXT NOT NULL DEFAULT '',
    banned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ban_duration_seconds BIGINT NOT NULL,
    escalation_level INT NOT NULL DEFAULT 0,
    expired BOOL NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Requests (captured HTTP requests)
CREATE TABLE requests (
    id SERIAL PRIMARY KEY,
    ip_address VARCHAR,
    protocol VARCHAR,
    fqdn VARCHAR,
    path VARCHAR,
    query VARCHAR,
    city_name VARCHAR,
    country_name VARCHAR,
    country_code VARCHAR,
    user_agent VARCHAR,
    method VARCHAR,
    referer VARCHAR,
    content_type VARCHAR,
    accept_language VARCHAR,
    x_request_id VARCHAR,
    rule_id INT REFERENCES rules(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Settings (key-value store for global configuration)
CREATE TABLE settings (
    key VARCHAR PRIMARY KEY,
    value TEXT NOT NULL
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_requests_created_at ON requests (created_at);
CREATE INDEX IF NOT EXISTS idx_requests_rule_id ON requests (rule_id);
CREATE INDEX IF NOT EXISTS idx_requests_country_code ON requests (country_code);
CREATE INDEX IF NOT EXISTS idx_requests_country_name ON requests (country_name);
CREATE INDEX IF NOT EXISTS idx_rules_active_weight ON rules (active, weight ASC);
CREATE INDEX IF NOT EXISTS idx_bans_ip_address ON bans(ip_address);
CREATE INDEX IF NOT EXISTS idx_bans_expired ON bans(expired);

-- Seed default settings
INSERT INTO settings (key, value) VALUES
    ('safe_paths', '^/api/v1/auth/, ^/api/v1/health/'),
    ('trusted_ips', '10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16'),
    ('trusted_user_agents', 'pocketid/.*'),
    ('default_rule_mode', 'log_only'),
    ('log_retention_days', '30');

-- Seed default rate limit profiles
INSERT INTO rate_limit_profiles (name, description, max_retry, find_time_seconds, ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, ban_count_decay_days, fail_codes) VALUES
    ('Strict', '3 requests in 10 minutes → 24h ban with escalation', 3, 600, 86400, true, '{1,2,4,8}', 604800, 30, '{401,403,404,429}'),
    ('Moderate', '5 requests in 10 minutes → 1h ban with escalation', 5, 600, 3600, true, '{1,2,4,8}', 604800, 30, '{401,403,404}'),
    ('Relaxed', '30 requests in 5 minutes → 30min ban, no escalation', 30, 300, 1800, false, '{1}', 3600, 30, '{401,403}'),
    ('Scraping', '60 requests in 1 minute → 10min ban with escalation', 60, 60, 600, true, '{1,2,4,8}', 86400, 7, '{401,403,404,429,500}');
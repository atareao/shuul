-- Shuul Initial Schema (SQLite)
-- Consolidación única: todas las tablas, índices y seeds.
-- PG → SQLite: SERIAL → INTEGER PRIMARY KEY AUTOINCREMENT,
-- BOOL → INTEGER 0/1, TIMESTAMPTZ → TEXT (ISO 8601),
-- INT[] → TEXT (JSON array), $N → ?, no INTERVAL.

DROP TABLE IF EXISTS stats_cache;
DROP TABLE IF EXISTS bans;
DROP TABLE IF EXISTS rules;
DROP TABLE IF EXISTS rate_limit_profiles;
DROP TABLE IF EXISTS settings;

CREATE TABLE rate_limit_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    max_retry INTEGER NOT NULL DEFAULT 5,
    find_time_seconds INTEGER NOT NULL DEFAULT 600,
    ban_time_seconds INTEGER NOT NULL DEFAULT 3600,
    bantime_increment INTEGER NOT NULL DEFAULT 0,
    bantime_multipliers TEXT NOT NULL DEFAULT '[1,2,4,8]',
    bantime_maxtime_seconds INTEGER NOT NULL DEFAULT 604800,
    ban_count_decay_days INTEGER NOT NULL DEFAULT 30,
    fail_codes TEXT NOT NULL DEFAULT '[401,403,404]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    weight INTEGER NOT NULL DEFAULT 100,
    mode TEXT NOT NULL DEFAULT 'log_only',
    pipeline TEXT NOT NULL DEFAULT 'waf',
    allow INTEGER NOT NULL DEFAULT 1,
    ip_address TEXT,
    protocol TEXT,
    fqdn TEXT,
    path TEXT,
    query TEXT,
    city_name TEXT,
    country_name TEXT,
    country_code TEXT,
    user_agent TEXT,
    method TEXT,
    referer TEXT,
    content_type TEXT,
    accept_language TEXT,
    x_request_id TEXT,
    rate_limit_profile_id INTEGER REFERENCES rate_limit_profiles(id) ON DELETE SET NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE bans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ip_address TEXT NOT NULL,
    rule_id INTEGER REFERENCES rules(id) ON DELETE SET NULL,
    reason TEXT NOT NULL DEFAULT '',
    banned_at TEXT NOT NULL,
    ban_duration_seconds INTEGER NOT NULL,
    escalation_level INTEGER NOT NULL DEFAULT 0,
    expired INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE stats_cache (
    id INTEGER PRIMARY KEY DEFAULT 1,
    snapshot TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Índices
CREATE INDEX idx_rules_active_weight ON rules (active, weight ASC);
CREATE INDEX idx_bans_ip_address ON bans (ip_address);
CREATE INDEX idx_bans_expired ON bans (expired);
CREATE INDEX idx_bans_rule_id ON bans (rule_id);

-- Seed defaults
INSERT INTO settings (key, value) VALUES
    ('safe_paths', '^/api/v1/auth/, ^/api/v1/health/'),
    ('trusted_ips', '10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16'),
    ('trusted_user_agents', 'pocketid/.*'),
    ('default_rule_mode', 'log_only'),
    ('log_retention_days', '30');

-- Seed default rate limit profiles (fail_codes y bantime_multipliers como JSON TEXT)
INSERT INTO rate_limit_profiles (name, description, max_retry, find_time_seconds, ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, ban_count_decay_days, fail_codes, created_at, updated_at) VALUES
    ('Auth Brute Force', '5 requests in 5 minutes → 15min ban with escalation', 5, 300, 900, 1, '[1,2,4,8]', 604800, 30, '[401]', datetime('now'), datetime('now')),
    ('Admin Guard', '3 requests in 5 minutes → 24h ban with escalation', 3, 300, 86400, 1, '[1,2,4,8]', 604800, 30, '[401,403]', datetime('now'), datetime('now')),
    ('Path Scanning', '20 requests in 60 seconds → 5min ban with escalation. Un humano jamás genera 20 errores 404 en 1 minuto.', 20, 60, 300, 1, '[1,2,4,8]', 86400, 30, '[403,404]', datetime('now'), datetime('now')),
    ('API Abuse', '30 requests in 1 minute → 10min ban', 30, 60, 600, 0, '[1]', 600, 30, '[401,403,429,500]', datetime('now'), datetime('now')),
    ('Scraping', '60 requests in 1 minute → 5min ban with escalation', 60, 60, 300, 1, '[1,2,4,8]', 86400, 7, '[403,429,500]', datetime('now'), datetime('now')),
    ('Health & Webhooks', '100 requests in 60 seconds → 1min ban. Para health checks y webhooks.', 100, 60, 60, 0, '[1]', 60, 30, '[500,502,503]', datetime('now'), datetime('now')),
    ('Recidive', '3 reincidences in 48h → 7-day ban (max 30 days)', 3, 172800, 604800, 1, '[1,2,4,8]', 2592000, 60, '[403,429]', datetime('now'), datetime('now')),
    ('Scanner Aggressive', '50 requests in 10 seconds → 30min ban with escalation. Para escáneres rápidos incluso con retardo aleatorio.', 50, 10, 1800, 1, '[1,2,4,8]', 604800, 30, '[403,404,405,500]', datetime('now'), datetime('now')),
    ('Global Shield', 'Catch-all: 300 errores en 60s → 5min ban with escalation. Sin filtros, matchea todo. Atrapa cualquier IP que genere demasiados errores.', 300, 60, 300, 1, '[1,2,4,8]', 86400, 30, '[403,404,429,500,502,503]', datetime('now'), datetime('now'));
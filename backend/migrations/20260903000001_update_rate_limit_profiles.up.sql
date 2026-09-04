-- Update existing rate limit profiles for bots/malignos detection
-- Applied on top of 20260902000000_initial_schema

-- Profile 3: Path Scanning — más sensible (20/60s en vez de 15/600s)
UPDATE rate_limit_profiles SET
    max_retry = 20,
    find_time_seconds = 60,
    ban_time_seconds = 300,
    bantime_increment = 1,
    bantime_multipliers = '[1,2,4,8]',
    bantime_maxtime_seconds = 86400,
    fail_codes = '[403,404]',
    description = '20 requests in 60 seconds → 5min ban with escalation. Un humano jamás genera 20 errores 404 en 1 minuto.',
    updated_at = datetime('now')
WHERE id = 3;

-- Profile 6: Health & Webhooks — fail_codes realistas (errores de backend, no 429)
UPDATE rate_limit_profiles SET
    fail_codes = '[500,502,503]',
    description = '100 requests in 60 seconds → 1min ban. Para health checks y webhooks.',
    updated_at = datetime('now')
WHERE id = 6;

-- Profile 9: Global Shield (catch-all) — nuevo
INSERT OR IGNORE INTO rate_limit_profiles (id, name, description, max_retry, find_time_seconds, ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, ban_count_decay_days, fail_codes, created_at, updated_at)
VALUES (9, 'Global Shield', 'Catch-all: 300 errores en 60s → 5min ban with escalation. Sin filtros, matchea todo. Atrapa cualquier IP que genere demasiados errores.', 300, 60, 300, 1, '[1,2,4,8]', 86400, 30, '[403,404,429,500,502,503]', datetime('now'), datetime('now'));
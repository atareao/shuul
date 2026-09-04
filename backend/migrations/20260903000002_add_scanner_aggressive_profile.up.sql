-- Add Scanner Aggressive rate limit profile
-- Applied on top of 20260903000001_update_rate_limit_profiles

INSERT OR IGNORE INTO rate_limit_profiles (id, name, description, max_retry, find_time_seconds, ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, ban_count_decay_days, fail_codes, created_at, updated_at)
VALUES (9, 'Scanner Aggressive', '50 requests in 10 seconds → 30min ban with escalation. Para escáneres rápidos incluso con retardo aleatorio.', 50, 10, 1800, 1, '[1,2,4,8]', 604800, 30, '[403,404,405,500]', datetime('now'), datetime('now'));
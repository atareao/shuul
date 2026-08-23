DROP TABLE IF EXISTS bans;

ALTER TABLE rules
    DROP COLUMN IF EXISTS webhook,
    DROP COLUMN IF EXISTS ignoreip,
    DROP COLUMN IF EXISTS ban_count_decay_days,
    DROP COLUMN IF EXISTS bantime_maxtime_seconds,
    DROP COLUMN IF EXISTS bantime_multipliers,
    DROP COLUMN IF EXISTS bantime_increment,
    DROP COLUMN IF EXISTS ban_time_seconds,
    DROP COLUMN IF EXISTS find_time_seconds,
    DROP COLUMN IF EXISTS max_retry,
    DROP COLUMN IF EXISTS rate_limit_enabled;
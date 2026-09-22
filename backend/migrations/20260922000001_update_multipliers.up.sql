-- Update escalation_multipliers for all rate limit profiles with escalation_enabled=true
-- Applied on top of 20260922000000_create_rate_limiter_state
-- New multipliers: [1,2,4,8,16,32,64,128] — more aggressive escalation to reach bantime_maxtime

-- Profile 1: Auth Brute Force (max_ban=604800, ban_time=900 → 115200s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 1;

-- Profile 2: Admin Guard (max_ban=604800, ban_time=3600 → 460800s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 2;

-- Profile 3: Path Scanning (max_ban=86400, ban_time=300 → 38400s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 3;

-- Profile 4: API Abuse (max_ban=86400, ban_time=300 → 38400s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 4;

-- Profile 5: Scraping (max_ban=86400, ban_time=300 → 38400s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 5;

-- Profile 7: Recidive (max_ban=2592000, ban_time=604800 → 77414400s capped @ 2592000 @ level 2+)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 7;

-- Profile 8: Global Shield (max_ban=86400, ban_time=300 → 38400s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 8;

-- Profile 9: Scanner Aggressive (max_ban=604800, ban_time=1800 → 230400s @ level 7)
UPDATE rate_limit_profiles SET
    bantime_multipliers = '[1,2,4,8,16,32,64,128]',
    updated_at = datetime('now')
WHERE id = 9;

-- Profile 6: Health & Webhooks — escalation_enabled=false, se queda igual
-- No update needed
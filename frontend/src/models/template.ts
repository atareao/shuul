export interface RuleTemplate {
    name: string;
    description: string;
    category: string;
    severity: string;
    path: string | null;
    query: string | null;
    country_code: string | null;
    allow: boolean;
    store: boolean;
    pipeline: string;
    rate_limit_profile_id: number | null;
    rate_limit_profile_name: string | null;
    requires_fqdn: boolean;
}

export interface RateLimitProfileTemplate {
    id: number;
    name: string;
    description: string;
    max_retry: number;
    find_time_seconds: number;
    ban_time_seconds: number;
    bantime_increment: boolean;
    bantime_multipliers: number[];
    bantime_maxtime_seconds: number;
    ban_count_decay_days: number;
    fail_codes: number[];
}

export interface TemplatesResponse {
    waf: RuleTemplate[];
    jail: RuleTemplate[];
    profiles: RateLimitProfileTemplate[];
}
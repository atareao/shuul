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
    recommended_profile: string | null;
    requires_fqdn: boolean;
}

export interface RateLimitProfileTemplate {
    name: string;
    description: string;
    max_retry: number;
    find_time_seconds: number;
    ban_time_seconds: number;
    bantime_increment: boolean;
    bantime_multipliers: number[];
    bantime_maxtime_seconds: number;
    ban_count_decay_days: number;
}
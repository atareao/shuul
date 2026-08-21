export default interface Template {
    name: string;
    description: string;
    category: string;
    severity: string;
    path: string | null;
    query: string | null;
    country_code: string | null;
    allow: boolean;
    store: boolean;
    rate_limit_enabled: boolean;
    max_retry: number | null;
    find_time_seconds: number | null;
    ban_time_seconds: number | null;
    bantime_increment: boolean;
    bantime_multipliers: number[];
    bantime_maxtime_seconds: number;
    ban_count_decay_days: number;
    requires_fqdn: boolean;
}
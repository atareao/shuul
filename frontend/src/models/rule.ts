export default interface Rule {
    id: number;
    weight?: number;
    allow?: boolean;
    store?: boolean;
    ip_address?: string;
    protocol?: string;
    fqdn?: string;
    path?: string;
    query?: string;
    city_name?: string;
    country_name?: string;
    country_code?: string;
    active?: boolean;
    created_at?: Date;
    updated_at?: Date;

    // Rate limiting fields
    rate_limit_enabled?: boolean;
    max_retry?: number;
    find_time_seconds?: number;
    ban_time_seconds?: number;
    bantime_increment?: boolean;
    bantime_multipliers?: number[];
    bantime_maxtime_seconds?: number;
    ban_count_decay_days?: number;
    ignoreip?: string[];
    webhook?: string;
}

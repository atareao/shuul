export default interface RateLimitProfile {
    id: number;
    name: string;
    description?: string;
    max_retry?: number;
    find_time_seconds?: number;
    ban_time_seconds?: number;
    bantime_increment?: boolean;
    bantime_multipliers?: number[];
    bantime_maxtime_seconds?: number;
    ban_count_decay_days?: number;
    fail_codes?: number[];
    created_at?: Date;
    updated_at?: Date;
}
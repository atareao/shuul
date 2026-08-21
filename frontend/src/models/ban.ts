export default interface Ban {
    id: number;
    ip_address: string;
    rule_id?: number;
    jail_name: string;
    banned_at: string;
    ban_duration_seconds: number;
    escalation_level: number;
    reason?: string;
    expired: boolean;
    created_at: string;
}
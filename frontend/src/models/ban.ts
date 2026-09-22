export default interface Ban {
  id: string;
  ip_address: string;
  rule_id?: number;
  rule_name?: string;
  reason: string;
  ban_duration_seconds: number;
  escalation_level: number;
  time_remaining_seconds: number;
}

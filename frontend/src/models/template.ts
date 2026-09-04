export interface RuleTemplate {
  name: string;
  description: string;
  category: string;
  severity: string;
  // Existing filters:
  path: string | null;
  query: string | null;
  country_code: string | null;
  // NEW filters:
  ip_address: string | null;
  protocol: string | null;
  city_name: string | null;
  country_name: string | null;
  user_agent: string | null;
  method: string | null;
  referer: string | null;
  content_type: string | null;
  accept_language: string | null;
  x_request_id: string | null;
  // Existing fields:
  allow: boolean;
  pipeline: string;
  rate_limit_profile_id: number | null;
  rate_limit_profile_name: string | null;
  requires_fqdn: boolean;
  // NEW field:
  must_have: boolean;
  weight: number;
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

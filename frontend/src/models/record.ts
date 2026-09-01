export default interface Record {
    id: number;
    ip_address?: string;
    protocol?: string;
    fqdn?: string;
    path?: string;
    query?: string;
    city_name?: string;
    country_name?: string;
    country_code?: string;
    user_agent?: string;
    method?: string;
    referer?: string;
    content_type?: string;
    accept_language?: string;
    x_request_id?: string;
    rule_id?: number;
    rule_name?: string;
    created_at?: Date;
}
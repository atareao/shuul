use shuul_backend::models::{Rule, NewRequest, CacheRule};
use chrono::Utc;

#[tokio::test]
async fn cache_rule_matches_ip() {
    let rule = Rule {
        id: 1,
        weight: 1,
        allow: true,
        store: true,
        ip_address: Some("^127\\.0\\.0\\.1$".to_string()),
        protocol: None,
        fqdn: None,
        path: None,
        query: None,
        method: None,
        content_type: None,
        country_code: None,
        country_name: None,
        city_name: None,
        rule: None,
        active: true,
        created_at: Utc::now(),
    };
    let cache_rule = CacheRule::from_rule(rule);
    let req = NewRequest {
        ip_address: Some("127.0.0.1".to_string()),
        protocol: None,
        fqdn: None,
        path: None,
        query: None,
        method: None,
        content_type: None,
        country_code: None,
        country_name: None,
        city_name: None,
        rule_id: None,
        created_at: Utc::now(),
    };
    assert!(cache_rule.matches(&req));
}

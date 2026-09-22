//! # Endpoint principal de captura y filtrado (WAF)
//!
//! Pipeline:
//! 1. Extraer `NewRequest` de los encabezados HTTP
//! 2. Cargar settings desde AppState.settings (para `log_all_requests`)
//! 3. WAF rules (first match wins, weight ASC)
//!    - `allow=true` → 200 OK
//!    - `allow=false` → 403 FORBIDDEN
//!    - `mode=log_only` → 200 OK
//!    - `mode=off` → skip
//! 4. Si ninguna regla matcheó → check Ban
//!    - Baneado → 403 FORBIDDEN
//!    - No baneado → 200 OK
//! 5. Stats + audit log
//!
//! # Seguridad de concurrencia
//!
//! Todos los `MutexGuard` se liberan antes de cualquier `.await` para
//! garantizar que el future sea `Send` (requerido por axum/tokio).

use crate::audit_log;
use crate::models::{AppState, BanManager, EmptyResponse};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::sync::Arc;
use tracing::{error, trace, warn};

/// Determines if a log category should be logged based on the current mode.
fn should_log(mode: &str, category: &str) -> bool {
    match mode {
        "all" => true,
        "pass" => matches!(category, "admitted" | "cleared" | "unmatched"),
        "audit" => matches!(category, "denied" | "banned" | "registered" | "sanctioned"),
        _ => false,
    }
}

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

/// Information extracted from a matched rule, used after releasing the rules lock.
struct RuleMatch {
    rule_id: i32,
    rule_name: String,
    allow: bool,
}

/// Main entry point for the shuul service.
#[allow(clippy::too_many_lines)]
pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let mut request = crate::models::NewRequest::from_request(
        &headers,
        Some(&app_state.geoip),
        Some(&app_state.tor_service),
    );

    // ── Step 1: Load settings (sync, no await after this) ──
    let settings = match app_state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            error!("Settings mutex poisoned: {e}");
            return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
        },
    };
    let log_all_requests = settings.log_all_requests;

    // ── Step 2: Match against cached WAF rules (sync, releases lock before any await) ──
    let matched: Option<RuleMatch> = {
        let rules = match app_state.rules.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Rules mutex poisoned: {e}");
                return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
            },
        };

        let mut matched: Option<RuleMatch> = None;

        for cache_rule in rules.iter() {
            if !cache_rule.matches(&request) {
                continue;
            }

            // Skip jail-only rules in WAF pipeline
            if cache_rule.rule.pipeline == "jail" {
                continue;
            }

            // Skip rules that are turned off
            if cache_rule.rule.mode.as_str() == "off" {
                trace!("Rule mode is 'off', skipping");
                continue;
            }

            // First matching rule wins
            if cache_rule.rule.mode.as_str() == "log_only" {
                if should_log(&log_all_requests, "admitted") {
                    audit_log!("admitted",
                        "pipeline": "waf",
                        "rule_id": cache_rule.rule.id,
                        "rule_name": cache_rule.rule.name,
                        "ip": request.ip_address,
                        "country": request.country_code,
                        "path": request.path,
                        "method": request.method,
                        "ua": request.user_agent,
                        "fqdn": request.fqdn,
                        "query": request.query,
                        "referer": request.referer,
                    );
                }
                matched = Some(RuleMatch {
                    rule_id: cache_rule.rule.id,
                    rule_name: cache_rule.rule.name.clone(),
                    allow: true,
                });
                break;
            }

            // 'enforce' or any other mode — normal enforcement
            matched = Some(RuleMatch {
                rule_id: cache_rule.rule.id,
                rule_name: cache_rule.rule.name.clone(),
                allow: cache_rule.rule.allow,
            });
            break;
        }

        drop(rules);
        matched
    };
    // rules lock is released here

    // ── Step 3: Apply matched rule or check Ban (async operations allowed now) ──
    let method = request.method.clone().unwrap_or_default();
    let path = request.path.clone().unwrap_or_default();
    let user_agent = request.user_agent.clone().unwrap_or_default();
    let country_code = request.country_code.clone().unwrap_or_default();
    let ip = request.ip_address.clone().unwrap_or_default();

    if let Some(ref rm) = matched {
        // A WAF rule matched — apply it immediately (no ban check)
        request.rule_id = Some(rm.rule_id);
        trace!("Selected rule: id={}, allow={}", rm.rule_id, rm.allow);

        if rm.allow {
            // ALLOW
            app_state
                .stats
                .record_allowed(Some(&method), Some(&path), request.fqdn.as_deref());
            if should_log(&log_all_requests, "admitted") {
                audit_log!("admitted",
                    "pipeline": "waf",
                    "rule_id": rm.rule_id,
                    "rule_name": rm.rule_name,
                    "ip": ip,
                    "country": country_code,
                    "path": path,
                    "method": method,
                    "ua": user_agent,
                    "fqdn": request.fqdn,
                    "query": request.query,
                    "referer": request.referer,
                );
            }
            EmptyResponse::create(StatusCode::OK, "Ok")
        } else {
            // BLOCK
            app_state.stats.record_blocked(
                Some(rm.rule_id),
                Some(&country_code),
                Some(&method),
                Some(&path),
                request.fqdn.as_deref(),
            );
            if should_log(&log_all_requests, "denied") {
                audit_log!("denied",
                    "pipeline": "waf",
                    "rule_id": rm.rule_id,
                    "rule_name": rm.rule_name,
                    "ip": ip,
                    "country": country_code,
                    "path": path,
                    "method": method,
                    "ua": user_agent,
                    "fqdn": request.fqdn,
                    "query": request.query,
                    "referer": request.referer,
                );
            }
            EmptyResponse::create(StatusCode::FORBIDDEN, "Ko")
        }
    } else {
        // No WAF rule matched → check Ban with rule matching
        let ip_addr: Option<std::net::IpAddr> =
            request.ip_address.as_ref().and_then(|s| s.parse().ok());
        if let Some(ip) = ip_addr {
            let (reason, should_block, ban_rule_id, ban_rule_name) = {
                let rules = match app_state.rules.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Rules mutex poisoned: {e}");
                        return EmptyResponse::create(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        );
                    },
                };
                let ban_manager = match app_state.ban_manager.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Ban manager mutex poisoned: {e}");
                        return EmptyResponse::create(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        );
                    },
                };
                ban_manager
                    .is_banned(&ip)
                    .map_or((None, false, None, None), |ban| {
                        let rule_matches = ban
                            .rule_id
                            .and_then(|rid| rules.iter().find(|cr| cr.rule.id == rid))
                            .is_none_or(|cr| cr.matches(&request));
                        let ban_rule_name = ban
                            .rule_id
                            .and_then(|rid| rules.iter().find(|cr| cr.rule.id == rid))
                            .map(|cr| cr.rule.name.clone());
                        (
                            Some(ban.reason.clone()),
                            rule_matches,
                            ban.rule_id,
                            ban_rule_name,
                        )
                    })
            };
            if let Some(reason) = reason
                && should_block
            {
                if should_log(&log_all_requests, "banned") {
                    audit_log!("banned",
                        "pipeline": "waf",
                        "rule_id": ban_rule_id,
                        "rule_name": ban_rule_name,
                        "ip": request.ip_address,
                        "country": request.country_code,
                        "path": request.path,
                        "method": request.method,
                        "ua": request.user_agent,
                        "fqdn": request.fqdn,
                        "query": request.query,
                        "referer": request.referer,
                        "reason": reason,
                    );
                }
                app_state.stats.record_blocked(
                    None,
                    request.country_code.as_deref(),
                    Some(&method),
                    Some(&path),
                    request.fqdn.as_deref(),
                );
                return EmptyResponse::create(StatusCode::FORBIDDEN, &format!("Banned: {reason}"));
            }
            // IP is banned but request doesn't match the rule → allow through
        }

        // ── Check pending_bans (created by Jail pipeline, executed here) ──
        if let Some(ip) = ip_addr {
            let should_execute = {
                let rules = match app_state.rules.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Rules mutex poisoned: {e}");
                        return EmptyResponse::create(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        );
                    },
                };

                let pending_bans = match app_state.pending_bans.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Pending bans mutex poisoned: {e}");
                        return EmptyResponse::create(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        );
                    },
                };

                // Find a pending ban that matches this IP and whose rule matches the request
                let matched_pending = pending_bans
                    .iter()
                    .find(|pb| {
                        pb.ip == ip
                            && rules
                                .iter()
                                .any(|cr| cr.rule.id == pb.rule_id && cr.matches(&request))
                    })
                    .cloned();

                drop(rules);
                drop(pending_bans);
                matched_pending
            };

            if let Some(pb) = should_execute {
                // Execute the ban (sync, releases locks before await)
                let ban_info = {
                    let mut ban_manager = match app_state.ban_manager.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            error!("Ban manager mutex poisoned: {e}");
                            return EmptyResponse::create(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Internal error",
                            );
                        },
                    };

                    let info = ban_manager
                        .ban(
                            ip,
                            Some(pb.rule_id),
                            pb.reason.clone(),
                            pb.ban_duration_seconds,
                        )
                        .clone();
                    drop(ban_manager);
                    info
                };

                // Persist to database (async, no locks held)
                if let Err(e) = BanManager::persist_ban(
                    &app_state.pool,
                    ip,
                    Some(pb.rule_id),
                    &pb.reason,
                    ban_info.ban_duration_seconds,
                    ban_info.escalation_level,
                )
                .await
                {
                    warn!("Failed to persist ban from pending: {e}");
                }

                // Remove the executed pending ban
                if let Ok(mut pending) = app_state.pending_bans.lock() {
                    pending.retain(|p| p.ip != ip || p.rule_id != pb.rule_id);
                }

                // Audit log sanctioned
                if should_log(&log_all_requests, "sanctioned") {
                    audit_log!("sanctioned",
                        "pipeline": "waf",
                        "rule_id": pb.rule_id,
                        "rule_name": null,
                        "ip": request.ip_address,
                        "country": request.country_code,
                        "path": request.path,
                        "method": request.method,
                        "ua": request.user_agent,
                        "fqdn": request.fqdn,
                        "query": request.query,
                        "referer": request.referer,
                        "reason": pb.reason,
                    );
                }

                app_state.stats.record_blocked(
                    Some(pb.rule_id),
                    request.country_code.as_deref(),
                    Some(&method),
                    Some(&path),
                    request.fqdn.as_deref(),
                );

                return EmptyResponse::create(
                    StatusCode::FORBIDDEN,
                    &format!("Banned: {}", pb.reason),
                );
            }
        }

        // Not banned or banned but doesn't match → ALLOW (pass)
        if should_log(&log_all_requests, "unmatched") {
            audit_log!("unmatched",
                "pipeline": "waf",
                "rule_id": null,
                "rule_name": null,
                "ip": request.ip_address,
                "country": request.country_code,
                "path": request.path,
                "method": request.method,
                "ua": request.user_agent,
                "fqdn": request.fqdn,
                "query": request.query,
                "referer": request.referer,
            );
        }
        app_state
            .stats
            .record_allowed(Some(&method), Some(&path), request.fqdn.as_deref());
        EmptyResponse::create(StatusCode::OK, "Ok")
    }
}

#[cfg(test)]
mod tests {
    use crate::models::log_collector::LOG_COLLECTOR;
    use crate::models::{
        AppState, BanManager, CacheRule, GeoIpService, NewRequest, Rule, Settings, StatsCollector,
    };
    use axum::extract::State;
    use axum::response::IntoResponse;
    use maxminddb::Reader;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::{Arc, Mutex};

    /// Build HTTP headers for testing.
    fn build_headers(ip: &str, host: &str, uri: &str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-for", ip.parse().unwrap());
        headers.insert("x-forwarded-host", host.parse().unwrap());
        headers.insert("x-forwarded-uri", uri.parse().unwrap());
        headers
    }

    /// Load the GeoIP service from the mmdb file, trying multiple paths.
    fn load_geoip_service() -> Option<GeoIpService> {
        let paths = [
            "../geo/GeoLite2-City.mmdb",
            "/app/geo/GeoLite2-City.mmdb",
            "geo/GeoLite2-City.mmdb",
        ];
        for path in &paths {
            if let Ok(reader) = Reader::open_readfile(path) {
                return Some(GeoIpService::new(reader));
            }
        }
        eprintln!("GeoIP database not found — skipping GeoIP assertions");
        None
    }

    /// RED TEST: When `geoip = None` is passed, country_code should be None
    /// even for a known public IP (8.8.8.8).
    ///
    /// This simulates the OLD behavior where the WAF pipeline skips GeoIP
    /// when no rules use geoip filters.
    #[tokio::test]
    async fn test_from_request_geoip_none() {
        let headers = build_headers("8.8.8.8", "example.com", "/test");
        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.ip_address.as_deref(),
            Some("8.8.8.8"),
            "IP should be extracted from x-forwarded-for"
        );
        assert_eq!(
            request.country_code, None,
            "Without GeoIP service, country_code must be None"
        );
        assert_eq!(
            request.country_name, None,
            "Without GeoIP service, country_name must be None"
        );
        assert_eq!(
            request.city_name, None,
            "Without GeoIP service, city_name must be None"
        );
    }

    /// RED TEST: When `geoip = Some(service)` is passed, country_code should
    /// be populated for a known public IP (8.8.8.8 -> US).
    ///
    /// This simulates the NEW behavior where the WAF pipeline always performs
    /// GeoIP lookup regardless of whether rules use geoip filters.
    #[tokio::test]
    async fn test_from_request_with_geoip() {
        let geoip = load_geoip_service();
        let headers = build_headers("8.8.8.8", "example.com", "/test");

        if let Some(ref geoip) = geoip {
            let request = NewRequest::from_request(&headers, Some(geoip), None);

            assert_eq!(
                request.ip_address.as_deref(),
                Some("8.8.8.8"),
                "IP should be extracted from x-forwarded-for"
            );
            assert!(
                request.country_code.is_some(),
                "With GeoIP service, country_code should be Some for 8.8.8.8. Got: {:?}",
                request.country_code
            );
            assert!(
                request.country_name.is_some(),
                "With GeoIP service, country_name should be Some for 8.8.8.8. Got: {:?}",
                request.country_name
            );
        } else {
            eprintln!("Skipping test_from_request_with_geoip — mmdb file not found");
        }
    }

    /// RED TEST: Verify that non-geo fields like fqdn and path are populated
    /// correctly regardless of whether geoip is provided.
    #[tokio::test]
    async fn test_from_request_fields_present_without_geoip() {
        let headers = build_headers("8.8.8.8", "example.com", "/test");
        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.fqdn.as_deref(), Some("example.com"));
        assert_eq!(request.path.as_deref(), Some("/test"));
        assert_eq!(request.ip_address.as_deref(), Some("8.8.8.8"));
    }

    /// RED TEST: Verifies that `should_log` with "audit" mode
    /// includes the new event names (denied, banned, registered, sanctioned)
    /// and excludes old names (block, report_block, report_ban).
    ///
    /// This test asserts the NEW behavior after rename-log-events.
    /// Currently uses old names, so this should fail (RED).
    #[test]
    fn test_should_log_audit_new_event_names() {
        // New event names that SHOULD be in audit category
        assert!(
            super::should_log("audit", "denied"),
            "denied should be in audit category"
        );
        assert!(
            super::should_log("audit", "banned"),
            "banned should remain in audit category"
        );
        assert!(
            super::should_log("audit", "registered"),
            "registered should be in audit category"
        );
        assert!(
            super::should_log("audit", "sanctioned"),
            "sanctioned should be in audit category"
        );

        // Old event names that should NO LONGER be in audit
        assert!(
            !super::should_log("audit", "block"),
            "block should NOT be in audit category after rename"
        );
        assert!(
            !super::should_log("audit", "report_block"),
            "report_block should NOT be in audit category after rename"
        );
        assert!(
            !super::should_log("audit", "report_ban"),
            "report_ban should NOT be in audit category after rename"
        );
    }

    /// RED TEST: Verifies that `should_log` with "pass" mode
    /// includes the new pass event names (admitted, cleared, unmatched).
    #[test]
    fn test_should_log_pass_new_event_names() {
        assert!(
            super::should_log("pass", "admitted"),
            "admitted should be in pass category"
        );
        assert!(
            super::should_log("pass", "cleared"),
            "cleared should be in pass category"
        );

        // unmatched should be in pass and all categories
        assert!(
            super::should_log("pass", "unmatched"),
            "unmatched should be in pass category"
        );
        assert!(
            super::should_log("all", "unmatched"),
            "unmatched should be in all category"
        );
        assert!(
            !super::should_log("audit", "unmatched"),
            "unmatched should NOT be in audit category"
        );

        // Old pass event name should NOT match
        assert!(
            !super::should_log("pass", "pass"),
            "pass event should NOT be in pass category after rename"
        );
    }

    /// RED TEST: Verifies that `should_log` with "audit" mode no longer
    /// includes "whitelist" and "blacklist" categories after pipeline refactor.
    /// Currently returns true (FAIL), after refactor should return false.
    #[test]
    fn test_should_log_audit_excludes_whitelist_blacklist() {
        // After pipeline refactor, whitelist and blacklist categories should
        // be removed from the audit log. This test asserts the NEW behavior.
        assert!(
            !super::should_log("audit", "whitelist"),
            "whitelist should NOT be in audit category after pipeline refactor"
        );
        assert!(
            !super::should_log("audit", "blacklist"),
            "blacklist should NOT be in audit category after pipeline refactor"
        );
    }

    /// Build a minimal AppState for testing the banned audit log.
    async fn build_test_app_state() -> Arc<AppState> {
        // Create an in-memory SQLite pool (required by AppState, not used in ban check path)
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create a jail rule that matches /wp-admin
        let rule = Rule {
            id: 9,
            name: "Scanner Aggressive".to_string(),
            description: "Test rule".to_string(),
            weight: 10,
            mode: "enforce".to_string(),
            pipeline: "jail".to_string(),
            allow: false,
            is_tor: None,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: Some(r"/wp-admin".to_string()),
            query: None,
            city_name: None,
            country_name: None,
            country_code: None,
            user_agent: None,
            method: None,
            referer: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            rate_limit_profile_id: Some(9),
            rate_limit_profile_name: Some("Scanner Aggressive".to_string()),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let cache_rule = CacheRule::from_rule(&rule);

        // Create a BanManager with a ban for IP 10.0.0.1 with rule_id=9
        let mut ban_manager = BanManager::new(3600, false, vec![1], 86400, 7);
        let _ = ban_manager.ban(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            Some(9),
            "Scanner Aggressive - exceeded rate limit".to_string(),
            None,
        );

        // Settings with log_all_requests = "all"
        let settings = Settings {
            default_rule_mode: "enforce".to_string(),
            log_retention_days: 30,
            log_all_requests: "all".to_string(),
        };

        Arc::new(AppState {
            pool,
            secret: "test-secret".to_string(),
            geoip: {
                // Try to load GeoIP from common paths, fall back to /tmp/test.mmdb
                let paths = [
                    "../geo/GeoLite2-City.mmdb",
                    "/app/geo/GeoLite2-City.mmdb",
                    "geo/GeoLite2-City.mmdb",
                    "/tmp/test.mmdb",
                ];
                let mut geoip = None;
                for path in &paths {
                    if let Ok(reader) = Reader::open_readfile(path) {
                        geoip = Some(GeoIpService::new(reader));
                        break;
                    }
                }
                geoip
                    .expect("No GeoIP database found. Create one with: python3 create_test_mmdb.py")
            },
            tor_service: crate::models::TorService::new(),
            rules: Mutex::new(vec![cache_rule]),
            stats: StatsCollector::new(),
            static_dir: "static".to_string(),
            ban_manager: Mutex::new(ban_manager),
            rate_limiter: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            pending_bans: Mutex::new(Vec::new()),
            oidc_metadata: tokio::sync::RwLock::new(None),
            jwt_validator: tokio::sync::RwLock::new(None),
            oidc_states: tokio::sync::Mutex::new(HashMap::new()),
            oidc_client_id: None,
            oidc_redirect_url: None,
        })
    }

    /// RED TEST: Verifies that the banned audit log includes the ban's rule_id
    /// and rule_name instead of hardcoded null.
    ///
    /// Currently the audit_log! macro hardcodes `rule_id: null` and
    /// `rule_name: null` for banned events. This test asserts the NEW
    /// behavior where the ban's rule info is propagated.
    #[tokio::test]
    async fn test_banned_audit_includes_rule_info() {
        // Clear the LOG_COLLECTOR before the test
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            // Create a fresh collector
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let app_state = build_test_app_state().await;
        let headers = build_headers("10.0.0.1", "example.com", "/wp-admin");

        let response = super::shuul(State(app_state), headers).await;
        let status = response.into_response().status();
        assert_eq!(status, 403, "Banned IP should get 403 FORBIDDEN");

        // Check the LOG_COLLECTOR for the banned entry
        let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();

        let banned_entry = entries.iter().find(|e| e.event == "banned");
        assert!(
            banned_entry.is_some(),
            "Should have a 'banned' audit log entry"
        );

        let entry = banned_entry.unwrap();
        assert_eq!(
            entry.rule_id,
            Some(9),
            "banned audit log should include rule_id from the ban"
        );
        assert_eq!(
            entry.rule_name.as_deref(),
            Some("Scanner Aggressive"),
            "banned audit log should include rule_name from the ban"
        );
    }
}

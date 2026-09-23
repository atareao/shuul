//! # Endpoint de reporte de status codes
//!
//! Endpoint sin autenticación JWT donde el plugin de Traefik reporta
//! los status codes reales del backend. Esto permite al rate limiter
//! actuar incluso cuando el backend responde con errores internos.
//!
//! ## Flujo
//!
//! 1. Recibe `ReportPayload` (ip, `` `status_code` ``, path, method)
//! 2. Matchea contra TODAS las reglas activas (fail2ban-style: múltiples jails)
//! 3. Para cada regla que matchee con `rate_limit_profile_id`:
//!    a. Carga el perfil de rate limiting
//!    b. Si `status_code` está en `profile.fail_codes`:
//!       - Obtiene/crea el `RateLimiter` y llama a `rl.record(ip)`
//!       - Si excede el umbral → banea la IP
//! 4. Devuelve 200 OK siempre (fire-and-forget)

use crate::audit_log;
use crate::models::{
    AppState, EmptyResponse, NewRequest, PendingBan, RateLimitProfile, RateLimiter, ReportPayload,
};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{error, trace, warn};

/// Determines if a log category should be logged based on the current mode.
fn should_log(mode: &str, category: &str) -> bool {
    match mode {
        "all" => true,
        "pass" => matches!(category, "admitted" | "cleared"),
        "audit" => matches!(category, "denied" | "banned" | "registered" | "pending"),
        _ => false,
    }
}

pub fn report_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::post(report_handler))
}

/// Handler de reporte de status codes desde el plugin de Traefik.
///
/// # Seguridad de concurrencia
///
/// Todos los `MutexGuard` se liberan antes de cualquier `.await` para
/// garantizar que el future sea `Send`.
#[allow(clippy::cast_sign_loss, clippy::too_many_lines)]
async fn report_handler(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<ReportPayload>,
) -> impl IntoResponse {
    let log_all_requests = app_state
        .settings
        .lock()
        .map_or_else(|_| "all".to_string(), |g| g.log_all_requests.clone());

    // ── GeoIP lookup (needed for matching and all audit logs) ──
    let ip_data = app_state.geoip.lookup(&payload.ip_address);

    // ── Tor lookup ──
    let is_tor = app_state.tor_service.is_exit_node(&payload.ip_address);

    // ── Step 1: Build NewRequest for matching ──
    let request = NewRequest {
        ip_address: Some(payload.ip_address.clone()),
        protocol: payload.protocol.clone(),
        fqdn: payload.fqdn.clone(),
        path: payload.path.clone(),
        query: payload.query.clone(),
        city_name: ip_data
            .city_name
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned(),
        country_name: ip_data
            .country_name
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned(),
        country_code: ip_data
            .country_code
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned(),
        user_agent: payload.user_agent.clone(),
        method: payload.method.clone(),
        referer: payload.referer.clone(),
        content_type: payload.content_type.clone(),
        accept_language: payload.accept_language.clone(),
        x_request_id: payload.x_request_id.clone(),
        is_tor,
        rule_id: None,
        created_at: chrono::Utc::now(),
    };

    // ── Step 2: Match against ALL cached rules (fail2ban-style: múltiples jails) ──
    let matches: Vec<(i32, i32, String)> = {
        let rules = match app_state.rules.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Rules mutex poisoned: {e}");
                return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
            },
        };

        let mut results: Vec<(i32, i32, String)> = Vec::new();
        for cache_rule in rules.iter() {
            if !cache_rule.matches(&request) {
                continue;
            }
            // Skip waf-only rules in Jail pipeline
            if cache_rule.rule.pipeline == "waf" {
                continue;
            }
            if cache_rule.rule.mode.as_str() == "off" {
                continue;
            }
            if let Some(profile_id) = cache_rule.rule.rate_limit_profile_id {
                results.push((cache_rule.rule.id, profile_id, cache_rule.rule.name.clone()));
            }
        }
        drop(rules);
        results
    };
    // rules lock is released here

    if matches.is_empty() {
        if should_log(&log_all_requests, "cleared") {
            audit_log!("cleared",
                "pipeline": "jail",
                "rule_id": null,
                "rule_name": null,
                "ip": payload.ip_address,
                "country": ip_data.country_code,
                "path": payload.path,
                "method": payload.method,
                "fqdn": payload.fqdn,
                "query": payload.query,
                "referer": payload.referer,
                "ua": payload.user_agent,
                "status_code": payload.status_code,
            );
        }
        return EmptyResponse::create(StatusCode::OK, "Ok");
    }

    // ── Step 3: Apply rate limiting for each matched rule (fail2ban-style) ──
    let ip: Option<IpAddr> = payload.ip_address.parse().ok();

    for (rule_id, profile_id, rule_name) in &matches {
        // Load the profile from DB (async, no locks held)
        let profile = match RateLimitProfile::read(&app_state.pool, *profile_id).await {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to load RateLimitProfile {profile_id}: {e}");
                continue;
            },
        };

        // Check if the reported status_code is in the profile's fail_codes
        let status_i32 = i32::from(payload.status_code);
        if !profile.fail_codes.contains(&status_i32) {
            if should_log(&log_all_requests, "cleared") {
                audit_log!("cleared",
                    "pipeline": "jail",
                    "rule_id": rule_id,
                    "rule_name": rule_name,
                    "ip": payload.ip_address,
                    "country": ip_data.country_code,
                    "path": payload.path,
                    "method": payload.method,
                    "fqdn": payload.fqdn,
                    "query": payload.query,
                    "referer": payload.referer,
                    "ua": payload.user_agent,
                    "status_code": status_i32,
                    "fail_codes": profile.fail_codes,
                    "profile": profile.name,
                );
            }
            continue;
        }

        trace!(
            "Status code {} is in fail_codes {:?} for profile '{}'",
            status_i32, profile.fail_codes, profile.name
        );

        // ── Record stats and audit log for this match + fail_code ──
        app_state.stats.record_blocked(
            Some(*rule_id),
            None,
            payload.method.as_deref(),
            payload.path.as_deref(),
            payload.fqdn.as_deref(),
        );
        if let Some(ip) = ip {
            // Rate limiter check (sync)
            let should_ban = {
                let mut rate_limiters = match app_state.rate_limiter.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        error!("Rate limiter mutex poisoned: {e}");
                        return EmptyResponse::create(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        );
                    },
                };
                let rl = rate_limiters.entry(*profile_id).or_insert_with(|| {
                    RateLimiter::new(profile.max_retry as u32, profile.find_time_seconds)
                });
                let result = rl.record(ip);
                drop(rate_limiters);
                result
            };
            // rate_limiter lock released

            if should_ban {
                if should_log(&log_all_requests, "pending") {
                    audit_log!("pending",
                        "pipeline": "jail",
                        "rule_id": rule_id,
                        "rule_name": rule_name,
                        "ip": payload.ip_address,
                        "country": ip_data.country_code,
                        "path": payload.path,
                        "method": payload.method,
                        "fqdn": payload.fqdn,
                        "query": payload.query,
                        "referer": payload.referer,
                        "ua": payload.user_agent,
                        "profile": profile.name,
                        "status_code": payload.status_code,
                    );
                }

                // Create PendingBan instead of executing ban directly
                let pending_ban = PendingBan {
                    ip,
                    rule_id: *rule_id,
                    reason: format!(
                        "Rate limit threshold exceeded: {} requests in {}s (profile: {})",
                        profile.max_retry, profile.find_time_seconds, profile.name
                    ),
                    ban_duration_seconds: if profile.bantime_increment {
                        None
                    } else {
                        Some(i64::from(profile.ban_time_seconds))
                    },
                    escalation_level: 0, // escalation handled by WAF when executing the ban
                    created_at: std::time::Instant::now(),
                };

                if let Ok(mut pending) = app_state.pending_bans.lock() {
                    pending.push(pending_ban);
                }
            } else if should_log(&log_all_requests, "registered") {
                audit_log!("registered",
                    "pipeline": "jail",
                    "rule_id": rule_id,
                    "rule_name": rule_name,
                    "ip": payload.ip_address,
                    "country": ip_data.country_code,
                    "path": payload.path,
                    "method": payload.method,
                    "fqdn": payload.fqdn,
                    "query": payload.query,
                    "referer": payload.referer,
                    "ua": payload.user_agent,
                    "profile": profile.name,
                    "status_code": payload.status_code,
                );
            }
        }
    }

    // Always return 200 OK (fire-and-forget semantics)
    EmptyResponse::create(StatusCode::OK, "Ok")
}

#[cfg(test)]
mod tests {
    use crate::models::log_collector::LOG_COLLECTOR;
    use crate::models::{
        AppState, BanManager, CacheRule, GeoIpService, ReportPayload, Rule, Settings,
        StatsCollector,
    };
    use axum::Json;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use chrono::Utc;
    use maxminddb::Reader;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Helper to load a GeoIP service from well-known paths.
    /// Returns `None` if no database file is found.
    fn load_geoip_service() -> Option<GeoIpService> {
        let paths = [
            "../geo/GeoLite2-City.mmdb",
            "/app/geo/GeoLite2-City.mmdb",
            "geo/GeoLite2-City.mmdb",
            "/tmp/test.mmdb",
        ];
        for path in &paths {
            if let Ok(reader) = Reader::open_readfile(path) {
                return Some(GeoIpService::new(reader));
            }
        }
        None
    }

    /// Helper to create the `rate_limit_profiles` table in an in-memory SQLite pool.
    async fn create_profiles_table(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rate_limit_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                max_retry INTEGER NOT NULL DEFAULT 5,
                find_time_seconds INTEGER NOT NULL DEFAULT 600,
                ban_time_seconds INTEGER NOT NULL DEFAULT 3600,
                bantime_increment INTEGER NOT NULL DEFAULT 0,
                bantime_multipliers TEXT NOT NULL DEFAULT '[1,2,4,8]',
                bantime_maxtime_seconds INTEGER NOT NULL DEFAULT 604800,
                ban_count_decay_days INTEGER NOT NULL DEFAULT 30,
                fail_codes TEXT NOT NULL DEFAULT '[401,403,404]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    /// Helper to insert a rate limit profile for testing.
    #[allow(clippy::cast_possible_wrap)]
    async fn insert_profile(
        pool: &sqlx::SqlitePool,
        id: i32,
        name: &str,
        max_retry: i32,
        find_time_seconds: i32,
        ban_time_seconds: i32,
        fail_codes_json: &str,
    ) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO rate_limit_profiles
             (id, name, description, max_retry, find_time_seconds, ban_time_seconds,
              bantime_increment, bantime_multipliers, bantime_maxtime_seconds,
              ban_count_decay_days, fail_codes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind("")
        .bind(max_retry)
        .bind(find_time_seconds)
        .bind(ban_time_seconds)
        .bind(false) // bantime_increment
        .bind("[1]") // bantime_multipliers
        .bind(604_800) // bantime_maxtime_seconds
        .bind(30) // ban_count_decay_days
        .bind(fail_codes_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Build a minimal AppState with empty rules (no Jail matches).
    async fn build_empty_app_state() -> Arc<AppState> {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        let settings = Settings {
            default_rule_mode: "enforce".to_string(),
            log_retention_days: 30,
            log_all_requests: "all".to_string(),
        };

        let geoip = load_geoip_service().expect(
            "No GeoIP database found. Expected at geo/GeoLite2-City.mmdb or ../geo/GeoLite2-City.mmdb",
        );

        Arc::new(AppState {
            pool,
            secret: "test-secret".to_string(),
            geoip,
            tor_service: crate::models::TorService::new(),
            rules: Mutex::new(Vec::new()),
            stats: StatsCollector::new(),
            static_dir: "static".to_string(),
            ban_manager: Mutex::new(BanManager::new(3600, false, vec![1], 86400, 7)),
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

    /// Build an AppState with a single Jail rule and matching profile in DB.
    /// `max_retry` controls whether the first hit exceeds threshold or not.
    async fn build_jail_app_state(max_retry: i32) -> Arc<AppState> {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create table and insert profile
        create_profiles_table(&pool).await;
        insert_profile(&pool, 1, "Test Jail Profile", max_retry, 60, 300, "[401]").await;

        // Create a Jail rule that matches path = /test
        let rule = Rule {
            id: 1,
            name: "Test Jail Rule".to_string(),
            description: "Test".to_string(),
            weight: 10,
            mode: "enforce".to_string(),
            pipeline: "jail".to_string(),
            allow: false,
            is_tor: None,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: Some(r"/test".to_string()),
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
            rate_limit_profile_id: Some(1),
            rate_limit_profile_name: Some("Test Jail Profile".to_string()),
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let cache_rule = CacheRule::from_rule(&rule);

        let settings = Settings {
            default_rule_mode: "enforce".to_string(),
            log_retention_days: 30,
            log_all_requests: "all".to_string(),
        };

        let geoip = load_geoip_service().expect(
            "No GeoIP database found. Expected at geo/GeoLite2-City.mmdb or ../geo/GeoLite2-City.mmdb",
        );

        Arc::new(AppState {
            pool,
            secret: "test-secret".to_string(),
            geoip,
            tor_service: crate::models::TorService::new(),
            rules: Mutex::new(vec![cache_rule]),
            stats: StatsCollector::new(),
            static_dir: "static".to_string(),
            ban_manager: Mutex::new(BanManager::new(3600, false, vec![1], 86400, 7)),
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

    /// Build a default ReportPayload with a 401 status code.
    fn make_report_payload(status_code: u16) -> ReportPayload {
        ReportPayload {
            ip_address: "10.0.0.1".to_string(),
            status_code,
            path: Some("/test".to_string()),
            method: Some("GET".to_string()),
            user_agent: None,
            referer: None,
            fqdn: None,
            query: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            protocol: None,
        }
    }

    // ── Existing should_log tests ──

    #[test]
    fn test_should_log_audit_new_event_names() {
        assert!(
            super::should_log("audit", "denied"),
            "denied should be in audit"
        );
        assert!(
            super::should_log("audit", "banned"),
            "banned should be in audit"
        );
        assert!(
            super::should_log("audit", "registered"),
            "registered should be in audit"
        );
        assert!(
            super::should_log("audit", "pending"),
            "pending should be in audit (replaces sanctioned in Jail)"
        );
        assert!(
            !super::should_log("audit", "sanctioned"),
            "sanctioned should NOT be in audit in Jail (moved to WAF)"
        );
        assert!(
            !super::should_log("audit", "block"),
            "block should NOT be in audit"
        );
        assert!(
            !super::should_log("audit", "report_block"),
            "report_block should NOT be in audit"
        );
        assert!(
            !super::should_log("audit", "report_ban"),
            "report_ban should NOT be in audit"
        );
    }

    #[test]
    fn test_should_log_pass_new_event_names() {
        assert!(
            super::should_log("pass", "admitted"),
            "admitted should be in pass"
        );
        assert!(
            super::should_log("pass", "cleared"),
            "cleared should be in pass"
        );
        assert!(
            !super::should_log("pass", "pass"),
            "pass should NOT be in pass"
        );
    }

    // ── RED tests: status_code missing from audit_log! ──

    /// RED TEST: `cleared` audit log (no matches, line 129) should include `status_code`.
    ///
    /// Currently `audit_log!("cleared", ...)` at line 129 does NOT pass
    /// `"status_code"`, so this test currently FAILS. After fixing the
    /// production code, the log should contain `status_code = Some(404)`.
    #[tokio::test]
    async fn test_cleared_log_includes_status_code() {
        // Clear the LOG_COLLECTOR before the test
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let app_state = build_empty_app_state().await;
        let payload = make_report_payload(404);

        let response = super::report_handler(State(app_state), Json(payload)).await;
        let status = response.into_response().status();
        assert_eq!(status, 200, "No matches should return 200 OK");

        let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();
        let cleared = entries.iter().find(|e| e.event == "cleared");
        assert!(cleared.is_some(), "Should have a 'cleared' audit log entry");

        let entry = cleared.unwrap();
        assert_eq!(
            entry.status_code,
            Some(404),
            "cleared audit log should include status_code from payload"
        );
    }

    /// RED TEST: `pending` audit log (threshold exceeded, line 220) should include `status_code`.
    ///
    /// Configured with max_retry=1 so the first hit exceeds the threshold.
    /// Currently `audit_log!("pending", ...)` at line 220 does NOT pass
    /// `"status_code"`, so this test currently FAILS.
    #[tokio::test]
    async fn test_pending_log_includes_status_code() {
        // Clear the LOG_COLLECTOR before the test
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let app_state = build_jail_app_state(1).await; // max_retry=1 → exceeds on first hit
        let payload = make_report_payload(401);

        let response = super::report_handler(State(app_state), Json(payload)).await;
        let status = response.into_response().status();
        assert_eq!(status, 200, "Jail pipeline should always return 200 OK");

        let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();
        let pending = entries.iter().find(|e| e.event == "pending");
        assert!(
            pending.is_some(),
            "Should have a 'pending' audit log entry (threshold exceeded)"
        );

        let entry = pending.unwrap();
        assert_eq!(
            entry.status_code,
            Some(401),
            "pending audit log should include status_code from payload"
        );
    }

    /// RED TEST: `registered` audit log (within threshold, line 257) should include `status_code`.
    ///
    /// Configured with max_retry=5 so the first hit is within the threshold.
    /// Currently `audit_log!("registered", ...)` at line 257 does NOT pass
    /// `"status_code"`, so this test currently FAILS.
    #[tokio::test]
    async fn test_registered_log_includes_status_code() {
        // Clear the LOG_COLLECTOR before the test
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let app_state = build_jail_app_state(5).await; // max_retry=5 → within threshold on first hit
        let payload = make_report_payload(401);

        let response = super::report_handler(State(app_state), Json(payload)).await;
        let status = response.into_response().status();
        assert_eq!(status, 200, "Jail pipeline should always return 200 OK");

        let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();
        let registered = entries.iter().find(|e| e.event == "registered");
        assert!(
            registered.is_some(),
            "Should have a 'registered' audit log entry (within threshold)"
        );

        let entry = registered.unwrap();
        assert_eq!(
            entry.status_code,
            Some(401),
            "registered audit log should include status_code from payload"
        );
    }
}

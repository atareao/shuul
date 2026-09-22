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
                );
            }
        }
    }

    // Always return 200 OK (fire-and-forget semantics)
    EmptyResponse::create(StatusCode::OK, "Ok")
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_should_log_audit_new_event_names() {
        // New event names that SHOULD be in audit category
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
        // After B2 refactor: sanctioned moves to WAF, pending replaces it in Jail
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
}

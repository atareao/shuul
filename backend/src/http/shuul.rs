//! # Endpoint principal de captura y filtrado (WAF)
//!
//! Pipeline:
//! 1. Extraer `NewRequest` de los encabezados HTTP
//! 2. Cargar settings desde AppState.settings
//! 3. Safe paths: si request.path coincide con `safe_paths` → ALLOW (skipped)
//! 4. Trusted IPs: si `request.ip_address` está en `trusted_ips` → ALLOW (skipped)
//! 5. Trusted user agents: si `request.user_agent` coincide → ALLOW (skipped)
//! 6. Check IP baneada → 403 FORBIDDEN
//! 7. Match contra reglas cacheadas (mode = 'enforce' | 'log_only')
//! 8. Si rule match + mode='log_only' → log (allow = true)
//! 9. Si rule match + mode='off' → skip
//! 10. Stats + audit log
//! 11. 200 OK o 403 FORBIDDEN
//!
//! # Seguridad de concurrencia
//!
//! Todos los `MutexGuard` se liberan antes de cualquier `.await` para
//! garantizar que el future sea `Send` (requerido por axum/tokio).

use crate::models::{AppState, EmptyResponse};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::sync::Arc;
use tracing::{debug, error};

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
pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // ── Step 0: Determine if GeoIP lookup is needed (sync, releases lock immediately) ──
    let needs_geoip = {
        let rules = match app_state.rules.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Rules mutex poisoned: {e}");
                return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
            },
        };
        rules.iter().any(|cr| {
            cr.rule.city_name.is_some()
                || cr.rule.country_name.is_some()
                || cr.rule.country_code.is_some()
        })
    };
    // rules lock is released here

    let mut request = crate::models::NewRequest::from_request(
        &headers,
        if needs_geoip {
            Some(&app_state.geoip)
        } else {
            None
        },
    );
    debug!("Captured request: {:?}", request);

    // ── Step 1: Load settings (sync, no await after this) ──
    let settings = match app_state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            error!("Settings mutex poisoned: {e}");
            return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
        },
    };
    debug!("Loaded settings");

    // ── Step 2: Safe paths check ──
    if let Some(ref path) = request.path {
        for safe_path_re in &settings.safe_paths_re {
            if safe_path_re.is_match(path) {
                debug!(
                    "Request path '{}' matches safe_path pattern → ALLOW (skip all checks)",
                    path,
                );
                app_state.stats.record_allowed();
                return EmptyResponse::create(StatusCode::OK, "Ok");
            }
        }
    }

    // ── Step 3: Trusted IPs check ──
    if let Some(ref ip_str) = request.ip_address
        && let Ok(ip) = ip_str.parse::<std::net::IpAddr>()
    {
        for trusted_net in &settings.trusted_ips {
            if trusted_net.contains(&ip) {
                debug!(
                    "IP {} is in trusted CIDR {} → ALLOW (skip all checks)",
                    ip, trusted_net
                );
                app_state.stats.record_allowed();
                return EmptyResponse::create(StatusCode::OK, "Ok");
            }
        }
    }

    // ── Step 4: Trusted user agents check ──
    if let Some(ref ua) = request.user_agent {
        for trusted_ua_re in &settings.trusted_user_agents_re {
            if trusted_ua_re.is_match(ua) {
                debug!("User-Agent matches trusted pattern → ALLOW (skip all checks)",);
                app_state.stats.record_allowed();
                return EmptyResponse::create(StatusCode::OK, "Ok");
            }
        }
    }

    // ── Step 5: Check if IP is actively banned (sync, releases lock immediately) ──
    let ip_addr: Option<std::net::IpAddr> =
        request.ip_address.as_ref().and_then(|s| s.parse().ok());
    if let Some(ip) = ip_addr {
        let banned = {
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
            ban_manager.is_banned(&ip).map(|ban| ban.reason.clone())
        };
        if let Some(reason) = banned {
            debug!("IP {} is banned (reason: {})", ip, reason);
            app_state
                .stats
                .record_blocked(None, request.country_code.as_deref());
            return EmptyResponse::create(StatusCode::FORBIDDEN, &format!("Banned: {reason}"));
        }
    }

    // ── Step 6: Match against cached rules (sync, releases lock before any await) ──
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
                debug!("Rule mode is 'off', skipping");
                continue;
            }

            // First matching rule wins
            match cache_rule.rule.mode.as_str() {
                "log_only" => {
                    debug!("Rule mode is 'log_only', allowing without enforcement");
                    matched = Some(RuleMatch {
                        rule_id: cache_rule.rule.id,
                        rule_name: cache_rule.rule.name.clone(),
                        allow: true,
                    });
                    break;
                },
                _ => {
                    // 'enforce' or any other mode — normal enforcement
                    matched = Some(RuleMatch {
                        rule_id: cache_rule.rule.id,
                        rule_name: cache_rule.rule.name.clone(),
                        allow: cache_rule.rule.allow,
                    });
                    break;
                },
            }
        }

        matched
    };
    // rules lock is released here

    // ── Step 7: Apply matched rule (async operations allowed now) ──
    let allow = if let Some(ref rm) = matched {
        request.rule_id = Some(rm.rule_id);
        debug!("Selected rule: id={}, allow={}", rm.rule_id, rm.allow);
        rm.allow
    } else {
        debug!("No matching rule found for request: {:?}", &request);
        true
    };

    // ── Step 8: Stats + audit log ──
    let method = request.method.clone().unwrap_or_default();
    let path = request.path.clone().unwrap_or_default();
    let user_agent = request.user_agent.clone().unwrap_or_default();
    let country_code = request.country_code.clone().unwrap_or_default();
    let ip = request.ip_address.take().unwrap_or_default();

    if let Some(rm) = &matched {
        if allow {
            // Allowed (log_only mode or allow = true)
            app_state.stats.record_allowed();
            debug!(
                "→ {method} {path} → ALLOW (rule: #{} {})",
                rm.rule_id, rm.rule_name
            );
        } else {
            // Blocked
            app_state
                .stats
                .record_blocked(Some(rm.rule_id), Some(&country_code));
            tracing::info!(
                target: "shuul_audit",
                "{}",
                serde_json::json!({
                    "event": "blocked",
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "pipeline": "waf",
                    "rule_id": rm.rule_id,
                    "rule_name": rm.rule_name,
                    "ip": ip,
                    "country": country_code,
                    "path": path,
                    "method": method,
                    "ua": user_agent,
                })
            );
            debug!(
                "→ {method} {path} → BLOCK (rule: #{} {})",
                rm.rule_id, rm.rule_name
            );
        }
    } else {
        app_state.stats.record_allowed();
        debug!("→ {method} {path} → ALLOW (no match)");
    }

    if allow {
        EmptyResponse::create(StatusCode::OK, "Ok")
    } else {
        EmptyResponse::create(StatusCode::FORBIDDEN, "Ko")
    }
}

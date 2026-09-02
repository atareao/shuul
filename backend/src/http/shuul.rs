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
//! 10. Persistir si store = true
//! 11. 200 OK o 403 FORBIDDEN
//!
//! # Seguridad de concurrencia
//!
//! Todos los `MutexGuard` se liberan antes de cualquier `.await` para
//! garantizar que el future sea `Send` (requerido por axum/tokio).

use crate::models::{AppState, EmptyResponse, NewRequest, Request};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use regex::Regex;
use std::mem;
use std::sync::Arc;
use tracing::{debug, error, warn};

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

/// Information extracted from a matched rule, used after releasing the rules lock.
struct RuleMatch {
    rule_id: i32,
    allow: bool,
    store: bool,
}

/// Main entry point for the shuul service.
pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let mut request = NewRequest::from_request(&headers, &app_state.maxmind_db);
    debug!("Captured request: {:?}", request);

    // ── Step 1: Load settings (sync, no await after this) ──
    let settings = match app_state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            error!("Settings mutex poisoned: {e}");
            return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
        },
    };
    debug!("Loaded settings: {:?}", settings);

    // ── Step 2: Safe paths check ──
    if let Some(ref path) = request.path {
        for safe_path in &settings.safe_paths {
            if let Ok(re) = Regex::new(safe_path) {
                if re.is_match(path) {
                    debug!(
                        "Request path '{}' matches safe_path '{}' → ALLOW (skip all checks)",
                        path, safe_path
                    );
                    return EmptyResponse::create(StatusCode::OK, "Ok");
                }
            } else {
                warn!("Invalid safe_path regex pattern: {}", safe_path);
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
                return EmptyResponse::create(StatusCode::OK, "Ok");
            }
        }
    }

    // ── Step 4: Trusted user agents check ──
    if let Some(ref ua) = request.user_agent {
        for trusted_ua in &settings.trusted_user_agents {
            if let Ok(re) = Regex::new(trusted_ua) {
                if re.is_match(ua) {
                    debug!(
                        "User-Agent matches trusted pattern '{}' → ALLOW (skip all checks)",
                        trusted_ua
                    );
                    return EmptyResponse::create(StatusCode::OK, "Ok");
                }
            } else {
                warn!("Invalid trusted_user_agent regex pattern: {}", trusted_ua);
            }
        }
    }

    // ── Step 5: Check if IP is actively banned (sync, releases lock immediately) ──
    let ip_addr: Option<std::net::IpAddr> = request.ip_address.as_ref().and_then(|s| s.parse().ok());
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
                        allow: true,
                        store: cache_rule.rule.store,
                    });
                    break;
                },
                _ => {
                    // 'enforce' or any other mode — normal enforcement
                    matched = Some(RuleMatch {
                        rule_id: cache_rule.rule.id,
                        allow: cache_rule.rule.allow,
                        store: cache_rule.rule.store,
                    });
                    break;
                },
            }
        }

        matched
    };
    // rules lock is released here

    // ── Step 7: Apply matched rule (async operations allowed now) ──
    let mut allow = true;
    let mut save = true;
    let mut request_save = false;

    if let Some(rm) = matched {
        request.rule_id = Some(rm.rule_id);
        debug!(
            "Selected rule: id={}, allow={}, store={}",
            rm.rule_id, rm.allow, rm.store
        );

        allow = rm.allow;
        save = rm.store;
        request_save = true;
    } else {
        debug!("No matching rule found for request: {:?}", &request);
    }

    // ── Step 8: Log summary at info level ──
    let method = request.method.clone().unwrap_or_default();
    let fqdn = request.fqdn.clone().unwrap_or_default();
    let path = request.path.clone().unwrap_or_default();
    let rule_label = request
        .rule_id
        .map(|id| format!("#{id}"))
        .unwrap_or_else(|| "none".to_string());

    if allow {
        debug!("→ {method} {fqdn}{path} → ALLOW (rule: {rule_label}, store: {save})");
    } else {
        debug!("→ {method} {fqdn}{path} → BLOCK (rule: {rule_label})");
    }

    // ── Step 9: Persist the request if needed (async) ──
    if request_save && save {
        debug!("Saving request as per rule configuration");
        save_on_cache_or_db(&app_state, request).await;
    } else {
        debug!("Not saving request as per rule configuration");
    }

    if allow {
        EmptyResponse::create(StatusCode::OK, "Ok")
    } else {
        EmptyResponse::create(StatusCode::FORBIDDEN, "Ko")
    }
}

/// Saves a request either to the in-memory cache or directly to the database.
async fn save_on_cache_or_db(app_state: &AppState, request: NewRequest) {
    if app_state.cache_enabled {
        debug!("Cache is enabled, saving request to cache");
        let mut requests_to_save: Option<Vec<NewRequest>> = None;
        {
            if let Ok(mut cache_guard) = app_state.cache.lock() {
                cache_guard.push(request);
                debug!("Request saved to cache. Cache size: {}", cache_guard.len());
                if cache_guard.len() >= app_state.cache_size {
                    requests_to_save = Some(mem::take(&mut *cache_guard));
                    debug!("Cache size reached limit, preparing to bulk save to database");
                }
            }
        }
        if let Some(requests) = requests_to_save {
            debug!(
                "Caching limit reached, saving {} requests to database",
                requests.len()
            );
            match Request::create_bulk(&app_state.pool, requests).await {
                Ok(data) => debug!("Saved {} requests from cache to database", data.len()),
                Err(e) => error!("Error saving requests from cache to database: {:?}", e),
            }
        }
    } else {
        match Request::create(&app_state.pool, request).await {
            Ok(req) => debug!("Saved request to database: {:?}", req),
            Err(e) => error!("Error saving request to database: {:?}", e),
        }
    }
}
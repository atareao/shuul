//! # Endpoint principal de captura y filtrado
//!
//! Pipeline extendido:
//! 1. Extraer `NewRequest` de los encabezados HTTP
//! 2. Cargar settings desde AppState.settings
//! 3. Safe paths: si request.path coincide con `safe_paths` → ALLOW (skipped)
//! 4. Trusted IPs: si `request.ip_address` está en `trusted_ips` → ALLOW (skipped)
//! 5. Trusted user agents: si `request.user_agent` coincide → ALLOW (skipped)
//! 6. Check IP baneada → 403 FORBIDDEN
//! 7. Match contra reglas cacheadas (mode = 'enforce' | '`log_only`')
//! 8. Si rule match + `rate_limit_profile_id` → rate limit + ban si excede
//! 9. Si rule match + mode='`log_only`' → log (allow = true)
//! 10. Si rule match + mode='off' → skip
//! 11. Persistir si store = true
//! 12. 200 OK o 403 FORBIDDEN
//!
//! # Seguridad de concurrencia
//!
//! Todos los `MutexGuard` se liberan antes de cualquier `.await` para
//! garantizar que el future sea `Send` (requerido por axum/tokio).

use crate::models::{
    AppState, BanManager, EmptyResponse, NewRequest, RateLimitProfile, RateLimiter, Request,
};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use regex::Regex;
use std::mem;
use std::net::IpAddr;
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
    rate_limit_profile_id: Option<i32>,
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
        && let Ok(ip) = ip_str.parse::<IpAddr>()
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
    let ip_addr: Option<IpAddr> = request.ip_address.as_ref().and_then(|s| s.parse().ok());
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

        let mut result: Option<RuleMatch> = None;
        for cache_rule in rules.iter() {
            if !cache_rule.matches(&request) {
                continue;
            }

            match cache_rule.rule.mode.as_str() {
                "off" => {
                    // Rule is off — skip entirely, continue to next rule
                    debug!("Rule mode is 'off', skipping");
                    continue;
                },
                "log_only" => {
                    debug!("Rule mode is 'log_only', allowing without enforcement");
                    result = Some(RuleMatch {
                        rule_id: cache_rule.rule.id,
                        allow: true,
                        store: cache_rule.rule.store,
                        rate_limit_profile_id: None, // no enforcement in log_only
                    });
                    break;
                },
                _ => {
                    // 'enforce' or any other mode — normal enforcement
                    result = Some(RuleMatch {
                        rule_id: cache_rule.rule.id,
                        allow: cache_rule.rule.allow,
                        store: cache_rule.rule.store,
                        rate_limit_profile_id: cache_rule.rule.rate_limit_profile_id,
                    });
                    break;
                },
            }
        }
        result
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

        // ── Step 8: Rate limiter + ban (if profile configured) ──
        if let Some(profile_id) = rm.rate_limit_profile_id {
            debug!(
                "Rule {} has rate_limit_profile_id={}",
                rm.rule_id, profile_id
            );

            // Load the profile from DB (async, no locks held)
            let profile = match RateLimitProfile::read(&app_state.pool, profile_id).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to load RateLimitProfile {profile_id}: {e}");
                    return EmptyResponse::create(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal error",
                    );
                },
            };

            if let Some(ip) = ip_addr {
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
                    let rl = rate_limiters.entry(rm.rule_id).or_insert_with(|| {
                        RateLimiter::new(profile.max_retry as u32, profile.find_time_seconds)
                    });
                    rl.record(ip)
                };
                // rate_limiter lock released

                if should_ban {
                    debug!(
                        "IP {} exceeded rate limit for rule {}, banning with profile '{}'",
                        ip, rm.rule_id, profile.name
                    );

                    // Ban in memory (sync, releases lock before await)
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
                        let ban_duration = if profile.bantime_increment {
                            None // BanManager handles escalation internally
                        } else {
                            Some(i64::from(profile.ban_time_seconds))
                        };

                        let reason = format!(
                            "Rate limit: {} requests in {}s (profile: {})",
                            profile.max_retry, profile.find_time_seconds, profile.name
                        );

                        // Clone the BanInfo to avoid keeping the lock
                        let info = ban_manager
                            .ban(ip, Some(rm.rule_id), reason.clone(), ban_duration)
                            .clone();
                        (reason, info)
                    };
                    // ban_manager lock released

                    // Persist to database (async, no locks held)
                    if let Err(e) = BanManager::persist_ban(
                        &app_state.pool,
                        ip,
                        Some(rm.rule_id),
                        &ban_info.0,
                        ban_info.1.ban_duration_seconds,
                        ban_info.1.escalation_level,
                    )
                    .await
                    {
                        warn!("Failed to persist ban to DB: {e}");
                    }

                    allow = false;
                }
            }
        }
    } else {
        debug!("No matching rule found for request: {:?}", &request);
    }

    // ── Step 9: Log summary at info level ──
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

    // ── Step 10: Persist the request if needed (async) ──
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

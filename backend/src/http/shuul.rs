//! # Endpoint principal de captura
//!
//! Pipeline extendido:
//! 1. Extraer request de headers
//! 2. Check: ¿IP baneada? → 403
//! 3. Rate limiter: ¿IP excede threshold? → Ban + 403
//! 4. Reglas estáticas (allow/deny)
//! 5. Persistir si la regla lo indica

use crate::models::{
    AppState, Ban, BanSettings, EmptyResponse, NewBan, NewRequest, RateLimiter, Request,
};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::mem;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, error};

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

/// Main entry point for the shuul service.
pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let mut request = NewRequest::from_request(&headers, &app_state.maxmind_db);
    debug!("Captured request: {:?}", request);

    // ── Step 1: Check if IP is actively banned ──
    if let Some(ip_str) = &request.ip_address {
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            if let Ok(ban_manager) = app_state.ban_manager.lock() {
                if let Some(ban) = ban_manager.is_banned(&ip) {
                    debug!("IP {} is banned (reason: {})", ip, ban.reason);
                    return EmptyResponse::create(
                        StatusCode::FORBIDDEN,
                        &format!("Banned: {}", ban.reason),
                    );
                }
            } else {
                error!("Ban manager mutex poisoned");
            }
        }
    }

    // ── Step 2: Match against cached rules ──
    let mut allow = true;
    let mut save = true;

    let matched_rule = if let Ok(rules) = app_state.rules.lock() {
        rules
            .iter()
            .find(|cache_rule| cache_rule.matches(&request))
            .cloned()
    } else {
        error!("Rules mutex poisoned");
        None
    };

    if let Some(cache_rule) = matched_rule {
        request.rule_id = Some(cache_rule.rule.id);
        debug!("Selected rule: {:?}", cache_rule.rule);
        save = cache_rule.rule.store;
        allow = cache_rule.rule.allow;

        // ── Step 3: Rate limiter check ──
        if cache_rule.rule.rate_limit_enabled
            && let Some(ip_str) = &request.ip_address
            && let Ok(ip) = ip_str.parse::<IpAddr>()
        {
            let ignored = cache_rule
                .rule
                .ignoreip
                .iter()
                .filter_map(|ignored_ip| ignored_ip.parse::<IpAddr>().ok())
                .any(|ignored_ip| ignored_ip == ip);

            if ignored {
                debug!(
                    "Skipping rate limit for ignored IP {} on rule {}",
                    ip, cache_rule.rule.id
                );
            } else {
                let should_ban = if let Ok(mut rate_limiters) = app_state.rate_limiter.lock() {
                    let rl = rate_limiters.entry(cache_rule.rule.id).or_insert_with(|| {
                        RateLimiter::new(
                            cache_rule.rule.max_retry as u32,
                            cache_rule.rule.find_time_seconds,
                        )
                    });
                    rl.record(ip)
                } else {
                    error!("Rate limiter mutex poisoned");
                    false
                };

                if should_ban {
                    debug!(
                        "IP {} exceeded rate limit for rule {}, banning",
                        ip, cache_rule.rule.id
                    );
                    let settings = BanSettings {
                        ban_time_seconds: cache_rule.rule.ban_time_seconds,
                        bantime_increment: cache_rule.rule.bantime_increment,
                        bantime_multipliers: cache_rule
                            .rule
                            .bantime_multipliers
                            .iter()
                            .map(|value| (*value).max(1) as u32)
                            .collect(),
                        bantime_maxtime_seconds: cache_rule.rule.bantime_maxtime_seconds,
                        ban_count_decay_days: i64::from(cache_rule.rule.ban_count_decay_days),
                    };
                    let ban = if let Ok(mut ban_manager) = app_state.ban_manager.lock() {
                        ban_manager
                            .ban(
                                ip,
                                Some(cache_rule.rule.id),
                                format!(
                                    "Rate limit: {} requests in {}s",
                                    cache_rule.rule.max_retry, cache_rule.rule.find_time_seconds
                                ),
                                &settings,
                                None,
                            )
                            .clone()
                    } else {
                        error!("Ban manager mutex poisoned during rate limit ban");
                        return EmptyResponse::create(StatusCode::FORBIDDEN, "Ko");
                    };
                    if let Err(e) = Ban::create(
                        &app_state.pool,
                        NewBan {
                            ip_address: ip.to_string(),
                            rule_id: Some(cache_rule.rule.id),
                            jail_name: format!("rule-{}", cache_rule.rule.id),
                            banned_at: ban.banned_at,
                            ban_duration_seconds: ban.ban_duration_seconds as i32,
                            escalation_level: ban.escalation_level as i32,
                            reason: Some(ban.reason.clone()),
                        },
                    )
                    .await
                    {
                        error!("Failed to persist automatic ban: {}", e);
                    }
                    allow = false;
                }
            }
        }
    }

    if request.rule_id.is_none() {
        debug!("No matching rule found for request: {:?}", &request);
    }

    // ── Step 4: Persist the request if the rule says so ──
    if save {
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

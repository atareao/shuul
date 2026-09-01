//! # Endpoint de reporte de status codes
//!
//! Endpoint sin autenticación JWT donde el plugin de Traefik reporta
//! los status codes reales del backend. Esto permite al rate limiter
//! actuar incluso cuando el backend responde con errores internos.
//!
//! ## Flujo
//!
//! 1. Recibe `ReportPayload` (ip, status_code, path, method)
//! 2. Matchea contra las reglas activas (reusa `CacheRule::matches`)
//! 3. Si matchea una regla con `rate_limit_profile_id`:
//!    a. Carga el perfil de rate limiting
//!    b. Si `status_code` está en `profile.fail_codes`:
//!       - Obtiene/crea el `RateLimiter` y llama a `rl.record(ip)`
//!       - Si excede el umbral → banea la IP
//! 4. Devuelve 200 OK siempre (fire-and-forget)

use crate::models::{
    AppState, BanManager, EmptyResponse, NewRequest, RateLimitProfile, RateLimiter, ReportPayload,
};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, error, warn};

pub fn report_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::post(report_handler))
}

/// Handler de reporte de status codes desde el plugin de Traefik.
///
/// # Seguridad de concurrencia
///
/// Todos los `MutexGuard` se liberan antes de cualquier `.await` para
/// garantizar que el future sea `Send`.
async fn report_handler(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<ReportPayload>,
) -> impl IntoResponse {
    debug!("Report received: {:?}", payload);

    // ── Step 1: Build a minimal NewRequest for matching ──
    let request = NewRequest {
        ip_address: Some(payload.ip_address.clone()),
        protocol: None,
        fqdn: None,
        path: payload.path.clone(),
        query: None,
        city_name: None,
        country_name: None,
        country_code: None,
        user_agent: None,
        method: payload.method.clone(),
        referer: None,
        content_type: None,
        accept_language: None,
        x_request_id: None,
        rule_id: None,
        created_at: chrono::Utc::now(),
    };

    // ── Step 2: Match against cached rules (sync, releases lock before any await) ──
    let matched: Option<(i32, i32)> = {
        let rules = match app_state.rules.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Rules mutex poisoned: {e}");
                return EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Internal error");
            },
        };

        let mut result: Option<(i32, i32)> = None;
        for cache_rule in rules.iter() {
            if !cache_rule.matches(&request) {
                continue;
            }
            if cache_rule.rule.mode.as_str() == "off" {
                continue;
            }
            if let Some(profile_id) = cache_rule.rule.rate_limit_profile_id {
                result = Some((cache_rule.rule.id, profile_id));
            }
            break;
        }
        result
    };
    // rules lock is released here

    // ── Step 3: Apply rate limiting if matched and fail_code ──
    if let Some((rule_id, profile_id)) = matched {
        debug!(
            "Report: {} {} {} (status={}) matched rule #{}, profile #{}",
            payload.method.as_deref().unwrap_or("?"),
            payload.path.as_deref().unwrap_or("?"),
            payload.ip_address,
            payload.status_code,
            rule_id,
            profile_id,
        );

        // Load the profile from DB (async, no locks held)
        let profile = match RateLimitProfile::read(&app_state.pool, profile_id).await {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to load RateLimitProfile {profile_id}: {e}");
                return EmptyResponse::create(StatusCode::OK, "Ok");
            },
        };

        // Check if the reported status_code is in the profile's fail_codes
        let status_i32 = i32::from(payload.status_code);
        if profile.fail_codes.contains(&status_i32) {
            debug!(
                "Status code {} is in fail_codes {:?} for profile '{}'",
                status_i32, profile.fail_codes, profile.name
            );

            if let Ok(ip) = payload.ip_address.parse::<IpAddr>() {
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
                    let rl = rate_limiters.entry(rule_id).or_insert_with(|| {
                        RateLimiter::new(profile.max_retry as u32, profile.find_time_seconds)
                    });
                    rl.record(ip)
                };
                // rate_limiter lock released

                if should_ban {
                    debug!(
                        "Report: BANNED {} via {} (profile: {})",
                        ip,
                        payload.path.as_deref().unwrap_or("?"),
                        profile.name
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
                            None
                        } else {
                            Some(i64::from(profile.ban_time_seconds))
                        };

                        let reason = format!(
                            "Rate limit (report): {} requests in {}s (profile: {})",
                            profile.max_retry, profile.find_time_seconds, profile.name
                        );

                        let info = ban_manager
                            .ban(ip, Some(profile_id), reason.clone(), ban_duration)
                            .clone();
                        (reason, info)
                    };
                    // ban_manager lock released

                    // Persist to database (async, no locks held)
                    if let Err(e) = BanManager::persist_ban(
                        &app_state.pool,
                        ip,
                        Some(profile_id),
                        &ban_info.0,
                        ban_info.1.ban_duration_seconds,
                        ban_info.1.escalation_level,
                    )
                    .await
                    {
                        warn!("Failed to persist ban to DB: {e}");
                    }
                }
            }
        } else {
            debug!(
                "Status code {} NOT in fail_codes {:?}, skipping rate limit",
                status_i32, profile.fail_codes
            );
        }
    } else {
        debug!(
            "Report: no matching rule for {} {}",
            payload.ip_address,
            payload.path.as_deref().unwrap_or("?")
        );
    }

    // Always return 200 OK (fire-and-forget semantics)
    EmptyResponse::create(StatusCode::OK, "Ok")
}

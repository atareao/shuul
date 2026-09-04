//! # Endpoint de reporte de status codes
//!
//! Endpoint sin autenticación JWT donde el plugin de Traefik reporta
//! los status codes reales del backend. Esto permite al rate limiter
//! actuar incluso cuando el backend responde con errores internos.
//!
//! ## Flujo
//!
//! 1. Recibe `ReportPayload` (ip, status_code, path, method)
//! 2. Matchea contra TODAS las reglas activas (fail2ban-style: múltiples jails)
//! 3. Para cada regla que matchee con `rate_limit_profile_id`:
//!    a. Carga el perfil de rate limiting
//!    b. Si `status_code` está en `profile.fail_codes`:
//!       - Obtiene/crea el `RateLimiter` y llama a `rl.record(ip)`
//!       - Si excede el umbral → banea la IP
//! 4. Devuelve 200 OK siempre (fire-and-forget)

use crate::models::{
    AppState, BanManager, NewRequest, RateLimitProfile, RateLimiter, ReportPayload,
};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{error, trace, warn};

/// Determines if a log category should be logged based on the current mode.
fn should_log(mode: &str, category: &str) -> bool {
    match mode {
        "all" => true,
        "pass" => matches!(category, "pass"),
        "audit" => matches!(category, "banned" | "block" | "report_block" | "report_ban"),
        _ => false,
    }
}

/// Macro for structured audit logging with visible category tag.
macro_rules! audit_log {
    ($category:expr, $($arg:tt)*) => {
        tracing::info!(
            "[{}] {}",
            $category.to_uppercase(),
            serde_json::json!({
                "event": $category,
                "ts": chrono::Utc::now().to_rfc3339(),
                $($arg)*
            })
        )
    };
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
async fn report_handler(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<ReportPayload>,
) -> impl IntoResponse {
    let log_all_requests = app_state
        .settings
        .lock()
        .map(|g| g.log_all_requests.clone())
        .unwrap_or_else(|_| "all".to_string());

    // ── GeoIP lookup (needed for matching and all audit logs) ──
    let ip_data = app_state.geoip.lookup(&payload.ip_address);

    if should_log(&log_all_requests, "report_received") {
        audit_log!("report_received",
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

    // ── Step 1: Build NewRequest for matching ──
    let request = NewRequest {
        ip_address: Some(payload.ip_address.clone()),
        protocol: payload.protocol.clone(),
        fqdn: payload.fqdn.clone(),
        path: payload.path.clone(),
        query: payload.query.clone(),
        city_name: ip_data.city_name.as_ref().filter(|s| !s.is_empty()).cloned(),
        country_name: ip_data.country_name.as_ref().filter(|s| !s.is_empty()).cloned(),
        country_code: ip_data.country_code.as_ref().filter(|s| !s.is_empty()).cloned(),
        user_agent: payload.user_agent.clone(),
        method: payload.method.clone(),
        referer: payload.referer.clone(),
        content_type: payload.content_type.clone(),
        accept_language: payload.accept_language.clone(),
        x_request_id: payload.x_request_id.clone(),
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
        results
    };
    // rules lock is released here

    if matches.is_empty() {
        if should_log(&log_all_requests, "report_ok") {
            audit_log!("report_ok",
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
        if should_log(&log_all_requests, "report_match") {
            audit_log!("report_match",
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
                "status_code": payload.status_code,
                "profile_id": profile_id,
            );
        }

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
            if should_log(&log_all_requests, "report_warn") {
                audit_log!("report_warn",
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
        );
        if should_log(&log_all_requests, "report_block") {
            audit_log!("report_block",
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
                rl.record(ip)
            };
            // rate_limiter lock released

            if should_ban {
                if should_log(&log_all_requests, "report_ban") {
                    audit_log!("report_ban",
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

                    // Skip if IP is already serving a ban
                    if ban_manager.is_banned(&ip).is_some() {
                        continue;
                    }
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
                        .ban(ip, Some(*rule_id), reason.clone(), ban_duration)
                        .clone();
                    (reason, info)
                };
                // ban_manager lock released

                // Persist to database (async, no locks held)
                if let Err(e) = BanManager::persist_ban(
                    &app_state.pool,
                    ip,
                    Some(*rule_id),
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
    }

    // Always return 200 OK (fire-and-forget semantics)
    use crate::models::EmptyResponse;
    EmptyResponse::create(StatusCode::OK, "Ok")
}

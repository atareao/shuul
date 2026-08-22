//! # Endpoints de bans
//!
//! CRUD para bans activos: listar, banear manualmente, desbanear.

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Ban, BanSettings, Data, NewBan};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
pub fn ban_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(list_handler))
        .route("/", routing::post(ban_handler))
        .route("/", routing::delete(unban_handler))
        .route("/info", routing::get(info_handler))
}

#[derive(Debug, Serialize)]
pub struct BanResponse {
    pub id: i32,
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub jail_name: String,
    pub banned_at: String,
    pub reason: String,
    pub ban_duration_seconds: i64,
    pub escalation_level: u32,
    pub expired: bool,
    pub created_at: String,
    pub time_remaining_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub reason: Option<String>,
    pub ban_duration_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UnbanParams {
    pub id: Option<i32>,
    pub ip_address: Option<String>,
    pub rule_id: Option<i32>,
}

impl From<Ban> for BanResponse {
    fn from(ban: Ban) -> Self {
        let remaining = ban.banned_at
            + chrono::Duration::seconds(i64::from(ban.ban_duration_seconds))
            - chrono::Utc::now();
        Self {
            id: ban.id,
            ip_address: ban.ip_address,
            rule_id: ban.rule_id,
            jail_name: ban.jail_name,
            banned_at: ban.banned_at.to_rfc3339(),
            reason: ban.reason.unwrap_or_default(),
            ban_duration_seconds: i64::from(ban.ban_duration_seconds),
            escalation_level: ban.escalation_level.max(0) as u32,
            expired: ban.expired,
            created_at: ban.created_at.to_rfc3339(),
            time_remaining_seconds: remaining.num_seconds().max(0) as u64,
        }
    }
}

/// GET /api/v1/bans — List all active bans.
pub async fn list_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let bans = Ban::read_active(&app_state.pool)
        .await?
        .into_iter()
        .map(BanResponse::from)
        .collect::<Vec<_>>();
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Active bans",
        Data::Some(serde_json::to_value(bans)?),
    ))
}

/// POST /api/v1/bans — Manually ban an IP.
pub async fn ban_handler(
    State(app_state): State<Arc<AppState>>,
    Json(params): Json<BanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip: IpAddr = params
        .ip_address
        .parse()
        .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;

    let reason = params.reason.unwrap_or_else(|| "Manual ban".to_string());
    let duration = params.ban_duration_seconds.unwrap_or(3600);

    let ban_info = {
        let mut ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager
            .ban(
                ip,
                params.rule_id,
                reason.clone(),
                &BanSettings::default(),
                Some(duration),
            )
            .clone()
    };
    let saved_ban = Ban::create(
        &app_state.pool,
        NewBan {
            ip_address: ip.to_string(),
            rule_id: params.rule_id,
            jail_name: "manual".to_string(),
            banned_at: ban_info.banned_at,
            ban_duration_seconds: ban_info.ban_duration_seconds as i32,
            escalation_level: ban_info.escalation_level as i32,
            reason: Some(ban_info.reason.clone()),
        },
    )
    .await?;

    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "IP banned",
        Data::Some(serde_json::to_value(BanResponse::from(saved_ban))?),
    ))
}

/// DELETE /api/v1/bans — Unban an IP.
pub async fn unban_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<UnbanParams>,
) -> Result<impl IntoResponse, AppError> {
    let removed = if let Some(id) = params.id {
        let Some(ban) = Ban::expire_by_id(&app_state.pool, id).await? else {
            return Ok(ApiResponse::new(
                StatusCode::NOT_FOUND,
                "IP not found or not banned",
                Data::None,
            ));
        };
        let ip: IpAddr = ban
            .ip_address
            .parse()
            .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;
        {
            let mut ban_manager = app_state
                .ban_manager
                .lock()
                .map_err(|_| AppError::CachePoisoned)?;
            ban_manager.unban(&ip, ban.rule_id);
        }
        if let Some(rule_id) = ban.rule_id
            && let Ok(mut rate_limiters) = app_state.rate_limiter.lock()
            && let Some(rate_limiter) = rate_limiters.get_mut(&rule_id)
        {
            rate_limiter.remove_ip(&ip);
        }
        true
    } else {
        let ip_address = params.ip_address.ok_or_else(|| {
            AppError::InvalidInput("ip_address parameter is required".to_string())
        })?;
        let ip: IpAddr = ip_address
            .as_str()
            .parse()
            .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;

        let expired = Ban::expire_by_ip_rule(&app_state.pool, &ip_address, params.rule_id).await?;
        {
            let mut ban_manager = app_state
                .ban_manager
                .lock()
                .map_err(|_| AppError::CachePoisoned)?;
            ban_manager.unban(&ip, params.rule_id);
        }
        if let Some(rule_id) = params.rule_id
            && let Ok(mut rate_limiters) = app_state.rate_limiter.lock()
            && let Some(rate_limiter) = rate_limiters.get_mut(&rule_id)
        {
            rate_limiter.remove_ip(&ip);
        }
        !expired.is_empty()
    };

    if removed {
        Ok(ApiResponse::new(StatusCode::OK, "IP unbanned", Data::None))
    } else {
        Ok(ApiResponse::new(
            StatusCode::NOT_FOUND,
            "IP not found or not banned",
            Data::None,
        ))
    }
}

/// GET /api/v1/bans/info — Count of active bans.
pub async fn info_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let count = Ban::active_count(&app_state.pool).await?;
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Active bans count",
        Data::Some(serde_json::to_value(count)?),
    ))
}

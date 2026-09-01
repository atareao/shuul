//! # Endpoints de bans
//!
//! CRUD para bans activos: listar, banear manualmente, desbanear.

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, BanManager, Data, PagedResponse, Pagination};
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
use tracing::warn;
pub fn ban_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(list_handler))
        .route("/", routing::post(ban_handler))
        .route("/", routing::delete(unban_handler))
        .route("/info", routing::get(info_handler))
}

#[derive(Debug, Serialize)]
pub struct BanResponse {
    pub id: String,
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub reason: String,
    pub ban_duration_seconds: i64,
    pub escalation_level: u32,
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
    pub id: Option<String>,
    pub ip_address: Option<String>,
    pub rule_id: Option<i32>,
}

/// GET /api/v1/bans — List all active bans.
pub async fn list_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let bans = {
        let ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager
            .active_bans()
            .into_iter()
            .map(|(ip, ban)| BanResponse {
                id: ip.to_string(),
                ip_address: ip.to_string(),
                rule_id: ban.rule_id,
                reason: ban.reason.clone(),
                ban_duration_seconds: ban.ban_duration_seconds,
                escalation_level: ban.escalation_level,
                time_remaining_seconds: ban.time_remaining().as_secs(),
            })
            .collect::<Vec<_>>()
    };
    let count = bans.len() as i64;
    let pagination = Pagination {
        page: 1,
        limit: count.max(1) as u32,
        pages: 1,
        records: count,
        prev: None,
        next: None,
    };
    Ok(PagedResponse::new(
        StatusCode::OK,
        "Active bans",
        Data::Some(serde_json::to_value(bans)?),
        pagination,
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

    let ban_info = {
        let mut ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager
            .ban(ip, params.rule_id, reason, params.ban_duration_seconds)
            .clone()
    };

    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "IP banned",
        Data::Some(serde_json::to_value(BanResponse {
            id: ip.to_string(),
            ip_address: ip.to_string(),
            rule_id: params.rule_id,
            reason: ban_info.reason.clone(),
            ban_duration_seconds: ban_info.ban_duration_seconds,
            escalation_level: ban_info.escalation_level,
            time_remaining_seconds: ban_info.time_remaining().as_secs(),
        })?),
    ))
}

/// DELETE /api/v1/bans — Unban an IP.
pub async fn unban_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<UnbanParams>,
) -> Result<impl IntoResponse, AppError> {
    let ip_str = params.id.or(params.ip_address).ok_or_else(|| {
        AppError::InvalidInput("ip_address or id parameter is required".to_string())
    })?;
    let ip: IpAddr = ip_str
        .parse()
        .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;

    let removed = {
        let mut ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager.unban(&ip, params.rule_id)
    };

    if removed {
        // Persist the unban to the database
        if let Err(e) = BanManager::remove_from_db(&app_state.pool, &ip, params.rule_id).await {
            warn!("Failed to persist unban to DB: {e}");
        }
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
    let count = {
        let ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager.active_count()
    };
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Active bans count",
        Data::Some(serde_json::to_value(count)?),
    ))
}

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

/// GET /api/v1/bans — List active bans with pagination, filtering, and sorting.
#[derive(Debug, Deserialize)]
pub struct BanListParams {
    pub ip_address: Option<String>,
    pub reason: Option<String>,
    pub escalation_level: Option<u32>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}

pub async fn list_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<BanListParams>,
) -> Result<impl IntoResponse, AppError> {
    let mut bans = {
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

    // Apply filters
    if let Some(ref ip) = params.ip_address {
        let ip_lower = ip.to_lowercase();
        bans.retain(|b| b.ip_address.to_lowercase().contains(&ip_lower));
    }
    if let Some(ref reason) = params.reason {
        let reason_lower = reason.to_lowercase();
        bans.retain(|b| b.reason.to_lowercase().contains(&reason_lower));
    }
    if let Some(level) = params.escalation_level {
        bans.retain(|b| b.escalation_level == level);
    }

    let records = bans.len() as i64;

    // Sort
    let sort_by = params.sort_by.as_deref().unwrap_or("ip_address");
    let asc = params.asc.unwrap_or(true);
    if [
        "ip_address",
        "reason",
        "ban_duration_seconds",
        "escalation_level",
        "time_remaining_seconds",
    ]
    .contains(&sort_by)
    {
        match sort_by {
            "ip_address" => {
                if asc {
                    bans.sort_by(|a, b| a.ip_address.cmp(&b.ip_address));
                } else {
                    bans.sort_by(|a, b| b.ip_address.cmp(&a.ip_address));
                }
            },
            "reason" => {
                if asc {
                    bans.sort_by(|a, b| a.reason.cmp(&b.reason));
                } else {
                    bans.sort_by(|a, b| b.reason.cmp(&a.reason));
                }
            },
            "ban_duration_seconds" => {
                if asc {
                    bans.sort_by(|a, b| a.ban_duration_seconds.cmp(&b.ban_duration_seconds));
                } else {
                    bans.sort_by(|a, b| b.ban_duration_seconds.cmp(&a.ban_duration_seconds));
                }
            },
            "escalation_level" => {
                if asc {
                    bans.sort_by(|a, b| a.escalation_level.cmp(&b.escalation_level));
                } else {
                    bans.sort_by(|a, b| b.escalation_level.cmp(&a.escalation_level));
                }
            },
            "time_remaining_seconds" => {
                if asc {
                    bans.sort_by(|a, b| a.time_remaining_seconds.cmp(&b.time_remaining_seconds));
                } else {
                    bans.sort_by(|a, b| b.time_remaining_seconds.cmp(&a.time_remaining_seconds));
                }
            },
            _ => {},
        }
    }

    // Pagination
    let page = params.page.unwrap_or(1).max(1);
    let limit = params
        .limit
        .unwrap_or(crate::constants::DEFAULT_LIMIT)
        .min(100);
    let offset = ((page - 1) as usize) * (limit as usize);
    let total_pages = if records == 0 {
        0u32
    } else {
        ((records as f64) / (limit as f64)).ceil() as u32
    };
    let paged_bans: Vec<_> = bans.into_iter().skip(offset).take(limit as usize).collect();

    let pagination = Pagination {
        page,
        limit,
        pages: total_pages,
        records,
        prev: if page > 1 {
            Some((page - 1).to_string())
        } else {
            None
        },
        next: if page < total_pages {
            Some((page + 1).to_string())
        } else {
            None
        },
    };
    Ok(PagedResponse::new(
        StatusCode::OK,
        "Active bans",
        Data::Some(serde_json::to_value(paged_bans)?),
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
            .ban(
                ip,
                params.rule_id,
                reason.clone(),
                params.ban_duration_seconds,
            )
            .clone()
    };

    // Persist to database (async, no locks held)
    if let Err(e) = BanManager::persist_ban(
        &app_state.pool,
        ip,
        params.rule_id,
        &reason,
        ban_info.ban_duration_seconds,
        ban_info.escalation_level,
    )
    .await
    {
        warn!("Failed to persist ban to DB: {e}");
    }

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

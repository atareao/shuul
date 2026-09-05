//! # Log viewer endpoint
//!
//! Provides access to the in-memory ring buffer for the frontend log viewer.
//! No persistence — all data is lost on restart.
//!
//! ## Endpoints
//!
//! - `GET /api/v1/logs` — Returns all log entries (client-side pagination).
//!   Optional query param `?event=block,report_ban` to filter by event type.
//! - `PUT /api/v1/logs/capacity` — Changes ring buffer capacity at runtime.
//!   Body: `{ "capacity": 5000 }`

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{
    AppState, EmptyResponse,
    log_collector::LOG_COLLECTOR,
};

pub fn log_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(list_logs))
        .route("/capacity", routing::put(set_capacity))
}

#[derive(Deserialize)]
struct LogFilter {
    event: Option<String>,
}

/// GET /api/v1/logs — Get all buffered log entries.
async fn list_logs(
    State(_app_state): State<Arc<AppState>>,
    Query(filter): Query<LogFilter>,
) -> Response {
    match LOG_COLLECTOR.lock() {
        Ok(collector) => {
            let capacity = collector.capacity();
            let all = collector.all();
            let entries = if let Some(ref event_filter) = filter.event {
                let events: Vec<&str> = event_filter.split(',').map(|s| s.trim()).collect();
                all.into_iter()
                    .filter(|e| events.contains(&e.event.as_str()))
                    .collect::<Vec<_>>()
            } else {
                all
            };
            let total = entries.len();
            Json(serde_json::json!({
                "status": 200,
                "data": {
                    "entries": entries,
                    "total": total,
                    "capacity": capacity,
                }
            }))
            .into_response()
        },
        Err(_) => {
            EmptyResponse::create(StatusCode::INTERNAL_SERVER_ERROR, "Log collector poisoned")
                .into_response()
        },
    }
}

#[derive(Deserialize)]
struct CapacityRequest {
    capacity: usize,
}

/// PUT /api/v1/logs/capacity — Update ring buffer capacity.
async fn set_capacity(
    State(_app_state): State<Arc<AppState>>,
    Json(req): Json<CapacityRequest>,
) -> Response {
    let new_cap = match req.capacity {
        1000 | 5000 | 10000 | 20000 => req.capacity,
        other => {
            return Json(serde_json::json!({
                "status": 400,
                "message": format!(
                    "Invalid capacity: {}. Valid values: 1000, 5000, 10000, 20000",
                    other
                ),
            }))
            .into_response();
        },
    };

    match LOG_COLLECTOR.lock() {
        Ok(mut collector) => {
            collector.set_capacity(new_cap);
            Json(serde_json::json!({
                "status": 200,
                "data": {
                    "capacity": new_cap,
                    "entries": collector.len(),
                }
            }))
            .into_response()
        },
        Err(_) => Json(serde_json::json!({
            "status": 500,
            "message": "Log collector poisoned",
        }))
        .into_response(),
    }
}
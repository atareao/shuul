//! # Endpoints de estadísticas
//!
//! Consulta de estadísticas agregadas desde `StatsCollector` (en memoria):
//! evolución temporal, top reglas, top países e información general.

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Data};
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;

pub fn stats_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(read_info_handler))
        .route("/top_countries", routing::get(read_top_countries))
        .route("/top_rules", routing::get(read_top_rules))
        .route("/top_methods", routing::get(read_top_methods))
        .route("/top_paths", routing::get(read_top_paths))
        .route("/evolution", routing::get(read_evolution))
        .route(
            "/evolution_by_method",
            routing::get(read_evolution_by_method),
        )
}

#[derive(Debug, Deserialize)]
pub struct EvolutionParams {
    pub unit: Option<String>,
    #[allow(dead_code)]
    pub last: Option<i32>,
}

/// Returns time‑series evolution data for requests.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, caches, etc.).
///   - `params`: Query parameters containing `unit` (`day|hour|minute`) and `last` (how many periods).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the evolution data or an error message.
pub async fn read_evolution(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EvolutionParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Evolution params: {:?}", params);
    let unit = params.unit.as_deref().unwrap_or("day").to_string();
    let evolution = app_state.stats.get_evolution(&unit);

    // Convert buckets into frontend-friendly series format:
    //   [{"id": "blocked", "data": [{"x": "2024-01-01T00:00:00Z", "y": 5}, ...]},
    //    {"id": "allowed", "data": [{"x": "2024-01-01T00:00:00Z", "y": 10}, ...]}]
    let blocked_series: Vec<serde_json::Value> = evolution
        .iter()
        .map(|bucket| {
            let chrono_dt =
                chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
            serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.blocked})
        })
        .collect();

    let allowed_series: Vec<serde_json::Value> = evolution
        .iter()
        .map(|bucket| {
            let chrono_dt =
                chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
            serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.allowed})
        })
        .collect();

    let result = serde_json::json!([
        {"id": "blocked", "data": blocked_series},
        {"id": "allowed", "data": allowed_series},
    ]);

    debug!("Request evolution: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Request evolution",
        Data::Some(result),
    ))
}

/// Returns the top rules based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top rules or an error.
pub async fn read_top_rules(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_rules = app_state.stats.get_top_rules();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_rules
        .into_iter()
        .map(|(rule_id, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (rule_id.to_string(), count as i32, percentage)
        })
        .collect();
    debug!("Top rules: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top rules",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top countries based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top countries or an error.
pub async fn read_top_countries(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_countries = app_state.stats.get_top_countries();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_countries
        .into_iter()
        .map(|(country, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (country, count as i32, percentage)
        })
        .collect();
    debug!("Top countries: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top countries",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top HTTP methods based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top methods or an error.
pub async fn read_top_methods(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_methods = app_state.stats.get_top_methods();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_methods
        .into_iter()
        .map(|(method, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (method, count as i32, percentage)
        })
        .collect();
    debug!("Top methods: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top methods",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top paths based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top paths or an error.
pub async fn read_top_paths(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_paths = app_state.stats.get_top_paths();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_paths
        .into_iter()
        .map(|(path, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (path, count as i32, percentage)
        })
        .collect();
    debug!("Top paths: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top paths",
        Data::Some(serde_json::to_value(result)?),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ReadInfoParams {
    pub option: Option<String>,
}

/// Retrieves aggregated information about requests (e.g., total count, per‑rule stats).
///
/// * **Parameters**
///   - `app_state`: Shared state with DB pool.
///   - `params`: Query parameters containing an optional `option` (e.g., "total").
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the requested info or an error.
pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" && opt != "filtered" {
                return Ok(ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'filtered'",
                    Data::None,
                )
                .into_response());
            }
            let info = if opt == "total" {
                app_state.stats.get_total_allowed() + app_state.stats.get_total_blocked()
            } else {
                app_state.stats.get_total_blocked()
            };
            debug!("Request info: {:?}", info);
            Ok(ApiResponse::new(
                StatusCode::OK,
                "Request info",
                Data::Some(serde_json::to_value(info)?),
            )
            .into_response())
        },
        None => Ok(ApiResponse::new(
            StatusCode::BAD_REQUEST,
            "Option parameter is required",
            Data::None,
        )
        .into_response()),
    }
}

/// Returns time-series evolution data per HTTP method.
pub async fn read_evolution_by_method(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EvolutionParams>,
) -> Result<impl IntoResponse, AppError> {
    let unit = params.unit.as_deref().unwrap_or("day").to_string();
    let method_evolution = app_state.stats.get_method_evolution(&unit);

    let result: Vec<serde_json::Value> = method_evolution
        .into_iter()
        .map(|(method, series)| {
            let data: Vec<serde_json::Value> = series
                .iter()
                .map(|bucket| {
                    let chrono_dt =
                        chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
                    serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.count})
                })
                .collect();
            serde_json::json!({"id": method, "data": data})
        })
        .collect();

    debug!("Evolution by method: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Evolution by method",
        Data::Some(serde_json::to_value(result)?),
    ))
}

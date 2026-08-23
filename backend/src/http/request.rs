//! # Endpoints de peticiones
//!
//! Consulta de peticiones HTTP capturadas: listado con paginación,
//! estadísticas (top países, top reglas, evolución temporal) y eliminación.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::error::AppError;
use crate::models::{
    ApiResponse, AppState, Data, NewRequest, PagedResponse, Pagination, ReadRequestParams, Request,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;

pub fn request_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/top_countries", routing::get(read_top_countries))
        .route("/top_rules", routing::get(read_top_rules))
        .route("/evolution", routing::get(read_evolution))
        .route("/", routing::delete(delete_handler))
}

#[derive(Debug, Deserialize)]
pub struct EvolutionParams {
    pub unit: Option<String>,
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
    let unit = params.unit.clone().unwrap_or_else(|| "day".to_string());
    let last = params.last.unwrap_or(7);
    let evolution = Request::evolution(&app_state.pool, &unit, last).await?;
    debug!("Request evolution: {:?}", evolution);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Request evolution",
        Data::Some(serde_json::to_value(evolution)?),
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
    let countries = Request::top_rules(&app_state.pool).await?;
    debug!("Top rules: {:?}", countries);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top rules",
        Data::Some(serde_json::to_value(countries)?),
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
    let countries = Request::top_countries(&app_state.pool).await?;
    debug!("Top countries: {:?}", countries);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top countries",
        Data::Some(serde_json::to_value(countries)?),
    ))
}

/// Creates a new request record.
///
/// * **Parameters**
///   - `app_state`: Shared state containing DB pool and caches.
///   - `json`: JSON payload representing the request to be stored.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the created request or an error.
pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<NewRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Request: {:?}", request);
    let request = Request::create(&app_state.pool, request).await?;
    debug!("Request created: {:?}", request);
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "Request created",
        Data::Some(serde_json::to_value(request)?),
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
            let info = Request::read_info(&app_state.pool, opt).await?;
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

/// Retrieves one or many requests depending on query parameters.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, caches).
///   - `params`: Query parameters for filtering, pagination, etc.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the request(s) or an error.
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRequestParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        let request = Request::read(&app_state.pool, id).await?;
        debug!("Request: {:?}", request);
        Ok(ApiResponse::new(
            StatusCode::OK,
            "Request",
            Data::Some(serde_json::to_value(request)?),
        )
        .into_response())
    } else {
        let requests = Request::read_paged(&app_state.pool, &params).await?;
        let count = Request::count_paged(&app_state.pool, &params).await?;
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = params.page.unwrap_or(DEFAULT_PAGE).saturating_sub(1);
        let total_pages = (count as f64 / f64::from(limit)).ceil() as u32;
        let pagination = Pagination {
            page: offset + 1,
            limit,
            pages: total_pages,
            records: count,
            prev: if offset > 0 {
                Some(format!("/requests?page={offset}&limit={limit}"))
            } else {
                None
            },
            next: if (offset + 1) < total_pages {
                Some(format!("/requests?page={}&limit={}", offset + 2, limit))
            } else {
                None
            },
        };
        Ok(PagedResponse::new(
            StatusCode::OK,
            "Requests",
            Data::Some(serde_json::to_value(requests)?),
            pagination,
        )
        .into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    days: Option<i32>,
}

/// Deletes requests older than a given number of days.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool).
///   - `params`: Query parameter with `days`.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON confirmation or an error.
pub async fn delete_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    let days = params
        .days
        .ok_or_else(|| AppError::InvalidInput("Days parameter is required".to_string()))?;
    let deleted = Request::delete_before(&app_state.pool, days).await?;
    debug!("Request deleted: {:?}", deleted);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Requests deleted",
        Data::Some(serde_json::to_value(deleted)?),
    ))
}

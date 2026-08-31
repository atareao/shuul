//! # Endpoints de perfiles de rate limiting
//!
//! CRUD completo para los perfiles de rate limiting:
//! crear, leer (con paginación), actualizar, eliminar y consultar info agregada.
//!
//! Estos perfiles se referencian desde las reglas mediante `rate_limit_profile_id`.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::error::AppError;
use crate::models::{
    ApiResponse, AppState, Data, NewRateLimitProfile, PagedResponse, Pagination, RateLimitProfile,
    ReadRateLimitProfileParams, UpdateRateLimitProfile,
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

pub fn rate_limit_profile_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/", routing::patch(update_handler))
        .route("/", routing::delete(delete_handler))
}

/// Creates a new rate limit profile in the database.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool).
///   - `profile`: The rate limit profile payload received from the client.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON response with the created profile or an error.
pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(profile): Json<NewRateLimitProfile>,
) -> Result<impl IntoResponse, AppError> {
    debug!("RateLimitProfile: {:?}", profile);
    let profile = RateLimitProfile::create(&app_state.pool, profile).await?;
    debug!("RateLimitProfile created: {:?}", &profile);
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "Rate limit profile created",
        Data::Some(serde_json::to_value(profile)?),
    ))
}

/// Retrieves one or many rate limit profiles depending on query parameters.
///
/// * **Parameters**
///   - `app_state`: Shared state containing DB pool.
///   - `params`: Query parameters for filtering, pagination, etc.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the profile(s) or an error message.
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRateLimitProfileParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        let profile = RateLimitProfile::read(&app_state.pool, id).await?;
        debug!("RateLimitProfile: {:?}", profile);
        Ok(ApiResponse::new(
            StatusCode::OK,
            "Rate limit profile",
            Data::Some(serde_json::to_value(profile)?),
        )
        .into_response())
    } else {
        let records = RateLimitProfile::read_paged(&app_state.pool, &params).await?;
        let count = RateLimitProfile::count_paged(&app_state.pool, &params).await?;
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = params.page.unwrap_or(DEFAULT_PAGE).saturating_sub(1);
        let total_pages = (count as f64 / f64::from(limit)).ceil() as u32;
        let pagination = Pagination {
            page: offset + 1,
            limit,
            pages: total_pages,
            records: count,
            prev: if offset > 0 {
                Some(format!("/rate-limit-profiles?page={offset}&limit={limit}"))
            } else {
                None
            },
            next: if (offset + 1) < total_pages {
                Some(format!(
                    "/rate-limit-profiles?page={}&limit={}",
                    offset + 2,
                    limit
                ))
            } else {
                None
            },
        };
        Ok(PagedResponse::new(
            StatusCode::OK,
            "Rate limit profiles",
            Data::Some(serde_json::to_value(records)?),
            pagination,
        )
        .into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadInfoParams {
    pub option: Option<String>,
}

/// Retrieves aggregated information about rate limit profiles (total count).
///
/// * **Parameters**
///   - `app_state`: Shared application state.
///   - `params`: Query parameter with an optional `option` field (`"total"`).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the requested info or an error.
pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" {
                return Ok(ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total'",
                    Data::None,
                )
                .into_response());
            }
            let info = RateLimitProfile::read_info(&app_state.pool, opt).await?;
            debug!("Rate limit profile info: {:?}", info);
            Ok(ApiResponse::new(
                StatusCode::OK,
                "Rate limit profile info",
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

/// Updates an existing rate limit profile in the database.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool).
///   - `profile`: The update payload.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the updated profile or an error.
pub async fn update_handler(
    State(app_state): State<Arc<AppState>>,
    Json(profile): Json<UpdateRateLimitProfile>,
) -> Result<impl IntoResponse, AppError> {
    debug!("RateLimitProfile update: {:?}", profile);
    let profile = RateLimitProfile::update(&app_state.pool, profile).await?;
    debug!("RateLimitProfile updated: {:?}", &profile);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rate limit profile updated",
        Data::Some(serde_json::to_value(profile)?),
    ))
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    id: Option<i32>,
}

/// Deletes a rate limit profile by ID.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool).
///   - `params`: Query parameter with the profile `id`.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON confirmation or an error.
pub async fn delete_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    let id = params
        .id
        .ok_or_else(|| AppError::InvalidInput("id parameter is required".to_string()))?;
    let profile = RateLimitProfile::delete(&app_state.pool, id).await?;
    debug!("RateLimitProfile deleted: {:?}", &profile);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rate limit profile deleted",
        Data::Some(serde_json::to_value(profile)?),
    ))
}

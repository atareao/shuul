//! # Endpoints de reglas
//!
//! CRUD completo para las reglas de filtrado HTTP:
//! crear, leer (con paginación), actualizar, eliminar y consultar info agregada.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::error::AppError;
use crate::models::{
    ApiResponse, AppState, Data, NewRule, PagedResponse, Pagination, ReadRuleParams, Rule,
    UpdateRule,
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

pub fn rule_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/", routing::patch(update_handler))
        .route("/", routing::delete(delete_handler))
}

/// Creates a new rule in the database and updates the in‑memory cache.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool, cache, etc.).
///   - `rule`: The rule payload received from the client.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON response with the created rule or an error.
pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(rule): Json<NewRule>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Rule: {:?}", rule);
    let rule = Rule::create(&app_state.pool, rule).await?;
    debug!("Rule created: {:?}", &rule);
    {
        // Lock the cache; map poisoning to AppError
        let mut rules_guard = app_state
            .rules
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        rules_guard.push(rule.clone().into());
        rules_guard.sort_by_key(|item| item.rule.weight);
        debug!("Rule updated: {:?}", rules_guard);
    }
    // Propagar error de serialización con `?`
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "Rule created",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

/// Retrieves one or many rules depending on query parameters.
///
/// * **Parameters**
///   - `app_state`: Shared state containing DB pool and cached rules.
///   - `params`: Query parameters for filtering, pagination, etc.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the rule(s) or an error message.
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRuleParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        let rule = Rule::read(&app_state.pool, id).await?;
        debug!("Rule: {:?}", rule);
        Ok(ApiResponse::new(
            StatusCode::OK,
            "Rule",
            Data::Some(serde_json::to_value(rule)?),
        )
        .into_response())
    } else {
        let records = Rule::read_paged(&app_state.pool, &params).await?;
        let count = Rule::count_paged(&app_state.pool, &params).await?;
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = params.page.unwrap_or(DEFAULT_PAGE).saturating_sub(1);
        let total_pages = (count as f64 / f64::from(limit)).ceil() as u32;
        let pagination = Pagination {
            page: offset + 1,
            limit,
            pages: total_pages,
            records: count,
            prev: if offset > 0 {
                Some(format!("/records?page={offset}&limit={limit}"))
            } else {
                None
            },
            next: if (offset + 1) < total_pages {
                Some(format!("/records?page={}&limit={}", offset + 2, limit))
            } else {
                None
            },
        };
        Ok(PagedResponse::new(
            StatusCode::OK,
            "Records",
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

/// Retrieves aggregated information about rules (e.g., total count or active count).
///
/// * **Parameters**
///   - `app_state`: Shared application state.
///   - `params`: Query parameter with an optional `option` field (`"total"` or `"active"`).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the requested info or an error.
pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" && opt != "active" {
                return Ok(ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'active'",
                    Data::None,
                )
                .into_response());
            }
            let info = Rule::read_info(&app_state.pool, opt).await?;
            debug!("Record info: {:?}", info);
            Ok(ApiResponse::new(
                StatusCode::OK,
                "Record info",
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

/// Updates an existing rule in the database and refreshes the in‑memory cache.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
///   - `rule`: The update payload.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the updated rule or an error.
pub async fn update_handler(
    State(app_state): State<Arc<AppState>>,
    Json(rule): Json<UpdateRule>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Rule: {:?}", rule);
    let rule = Rule::update(&app_state.pool, rule).await?;
    {
        let mut rules_guard = app_state
            .rules
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        rules_guard.retain(|r| r.rule.id != rule.id);
        rules_guard.push(rule.clone().into());
        rules_guard.sort_by_key(|r| r.rule.weight);
        debug!("Rule updated: {:?}", rules_guard);
    }
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule updated",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    id: Option<i32>,
}

/// Deletes a rule by ID.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache).
///   - `params`: Query parameter with the rule `id`.
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
    let rule = Rule::delete(&app_state.pool, id).await?;
    debug!("Rule deleted: {:?}", rule);
    {
        let mut rules_guard = app_state
            .rules
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        rules_guard.retain(|cached_rule| cached_rule.rule.id != id);
    }
    if let Ok(mut rate_limiters) = app_state.rate_limiter.lock() {
        rate_limiters.remove(&id);
    }
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rules deleted",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

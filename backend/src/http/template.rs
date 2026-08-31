//! # Endpoints de plantillas
//!
//! Devuelve el catálogo de plantillas preconfiguradas para reglas
//! y perfiles de rate limiting. Endpoints separados para cada tipo.

use crate::models::{ApiResponse, AppState, Data};
use crate::templates::{all_rate_limit_profile_templates, all_rule_templates};
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::sync::Arc;

pub fn template_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/rules", routing::get(list_rule_templates))
        .route(
            "/rate-limit-profiles",
            routing::get(list_rate_limit_profile_templates),
        )
}

/// GET /api/v1/templates/rules — List all rule templates.
pub async fn list_rule_templates(
    State(_app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, crate::models::error::AppError> {
    let templates = all_rule_templates();
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule templates",
        Data::Some(serde_json::to_value(templates)?),
    ))
}

/// GET /api/v1/templates/rate-limit-profiles — List all rate limit profile templates.
pub async fn list_rate_limit_profile_templates(
    State(_app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, crate::models::error::AppError> {
    let templates = all_rate_limit_profile_templates();
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rate limit profile templates",
        Data::Some(serde_json::to_value(templates)?),
    ))
}

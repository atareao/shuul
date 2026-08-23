//! # Endpoint de plantillas
//!
//! Devuelve el catálogo de reglas preconfiguradas.

use crate::models::{ApiResponse, AppState, Data};
use crate::templates::all_templates;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::sync::Arc;

pub fn template_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::get(list_templates))
}

/// GET /api/v1/templates — List all rule templates.
pub async fn list_templates(
    State(_app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, crate::models::error::AppError> {
    let templates = all_templates();
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule templates",
        Data::Some(serde_json::to_value(templates)?),
    ))
}
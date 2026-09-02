use crate::models::{ApiResponse, Data};
use crate::templates::{all_rate_limit_profile_templates, all_rule_templates};
use axum::{Router, extract::State, response::IntoResponse, routing};
use std::sync::Arc;

use crate::models::AppState;

pub fn template_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::get(list_templates))
}

/// GET /api/v1/templates — List all rule templates and rate limit profiles.
pub async fn list_templates(State(_app_state): State<Arc<AppState>>) -> impl IntoResponse {
    let rules = all_rule_templates();
    let profiles = all_rate_limit_profile_templates();

    let mut waf = Vec::new();
    let mut jail = Vec::new();

    for template in rules {
        match template.pipeline.as_str() {
            "jail" => jail.push(template),
            _ => waf.push(template),
        }
    }

    ApiResponse::create(
        axum::http::StatusCode::OK,
        "Templates retrieved",
        Data::Some(serde_json::json!({
            "waf": waf,
            "jail": jail,
            "profiles": profiles,
        })),
    )
}

//! # Endpoint de plantillas
//!
//! Devuelve el catálogo completo de plantillas de reglas y perfiles
//! de rate limiting, leídos desde la base de datos.

use crate::models::error::AppError;
use crate::models::{AppState, RateLimitProfile, ReadRateLimitProfileParams, Rule};
use axum::{Router, extract::State, response::IntoResponse, routing, Json};
use serde::Serialize;
use std::sync::Arc;

pub fn template_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::get(list_templates))
}

/// Plantilla ligera de regla para el frontend.
#[derive(Debug, Serialize, Clone)]
pub struct RuleTemplate {
    pub name: String,
    pub description: String,
    pub category: String,
    pub severity: String,
    pub path: Option<String>,
    pub query: Option<String>,
    pub country_code: Option<String>,
    pub allow: bool,
    pub store: bool,
    pub pipeline: String,
    pub rate_limit_profile_id: Option<i32>,
    pub rate_limit_profile_name: Option<String>,
    pub requires_fqdn: bool,
}

/// Respuesta unificada del endpoint de plantillas.
#[derive(Debug, Serialize)]
pub struct TemplatesResponse {
    pub waf: Vec<RuleTemplate>,
    pub jail: Vec<RuleTemplate>,
    pub profiles: Vec<RateLimitProfile>,
}

/// GET /api/v1/templates — List all rule templates and rate limit profiles.
pub async fn list_templates(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let rules = Rule::read_all(&app_state.pool).await?;
    let profiles = RateLimitProfile::read_paged(
        &app_state.pool,
        &ReadRateLimitProfileParams {
            id: None,
            name: None,
            page: Some(1),
            limit: Some(9999),
            sort_by: None,
            asc: None,
        },
    )
    .await?;

    let mut waf = Vec::new();
    let mut jail = Vec::new();

    for rule in rules {
        let template = RuleTemplate {
            name: rule.name.clone(),
            description: rule.description.clone(),
            category: extract_category(&rule.name),
            severity: "🟡 Medio".to_string(),
            path: rule.path.clone(),
            query: rule.query.clone(),
            country_code: rule.country_code.clone(),
            allow: rule.allow,
            store: rule.store,
            pipeline: rule.pipeline.clone(),
            rate_limit_profile_id: rule.rate_limit_profile_id,
            rate_limit_profile_name: rule.rate_limit_profile_name.clone(),
            requires_fqdn: rule.path.is_some(),
        };
        match rule.pipeline.as_str() {
            "jail" => jail.push(template),
            _ => waf.push(template),
        }
    }

    Ok(Json(TemplatesResponse { waf, jail, profiles }))
}

/// Extrae la categoría del nombre de la regla.
/// Toma la primera palabra antes de " - " y la convierte a minúsculas.
/// Si no hay separador, usa "general".
fn extract_category(name: &str) -> String {
    name.split(" - ")
        .next()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "general".to_string())
}
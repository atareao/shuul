//! # Endpoints de configuración global
//!
//! Permite leer y actualizar toda la configuración global de la aplicación
//! (`safe_paths`, `trusted_ips`, `trusted_user_agents`, `default_rule_mode`, `log_retention_days`).
//!
//! Los datos se cargan y persisten mediante las funciones del modelo [`Settings`].

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Data, Settings};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// DTO for reading settings (serialized to JSON).
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub safe_paths: Vec<String>,
    pub trusted_ips: Vec<String>,
    pub trusted_user_agents: Vec<String>,
    pub default_rule_mode: String,
    pub log_retention_days: i32,
}

/// DTO for updating settings (all fields optional).
#[derive(Debug, Deserialize)]
pub struct UpdateSettingsPayload {
    pub safe_paths: Option<Vec<String>>,
    pub trusted_ips: Option<Vec<String>>,
    pub trusted_user_agents: Option<Vec<String>>,
    pub default_rule_mode: Option<String>,
    pub log_retention_days: Option<i32>,
}

pub fn settings_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(get_settings))
        .route("/", routing::put(update_settings))
}

/// GET /api/v1/settings — Returns all current settings.
pub async fn get_settings(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let settings = app_state
        .settings
        .lock()
        .map_err(|_| AppError::CachePoisoned)?
        .clone();

    let response = SettingsResponse {
        safe_paths: settings.safe_paths,
        trusted_ips: settings
            .trusted_ips
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        trusted_user_agents: settings.trusted_user_agents,
        default_rule_mode: settings.default_rule_mode,
        log_retention_days: settings.log_retention_days,
    };

    Ok(ApiResponse::new(
        StatusCode::OK,
        "Settings",
        Data::Some(serde_json::to_value(response).map_err(AppError::from)?),
    ))
}

/// PUT /api/v1/settings — Updates settings and persists to DB.
pub async fn update_settings(
    State(app_state): State<Arc<AppState>>,
    Json(update): Json<UpdateSettingsPayload>,
) -> Result<impl IntoResponse, AppError> {
    // Validate log_retention_days
    if let Some(days) = update.log_retention_days
        && (!(1..=365).contains(&days))
    {
        return Err(AppError::InvalidInput(
            "log_retention_days must be between 1 and 365".to_string(),
        ));
    }

    // Validate default_rule_mode
    if let Some(ref mode) = update.default_rule_mode {
        match mode.as_str() {
            "enforce" | "log_only" | "off" => {},
            _ => {
                return Err(AppError::InvalidInput(
                    "default_rule_mode must be 'enforce', 'log_only', or 'off'".to_string(),
                ));
            },
        }
    }

    // Build new settings from existing + overrides
    let mut settings = app_state
        .settings
        .lock()
        .map_err(|_| AppError::CachePoisoned)?
        .clone();

    if let Some(paths) = update.safe_paths {
        settings.safe_paths = paths;
    }
    if let Some(ips) = update.trusted_ips {
        // Parse CIDR strings into IpNet
        settings.trusted_ips = ips
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: AppError| e)?;
    }
    if let Some(agents) = update.trusted_user_agents {
        settings.trusted_user_agents = agents;
    }
    if let Some(mode) = update.default_rule_mode {
        settings.default_rule_mode = mode;
    }
    if let Some(days) = update.log_retention_days {
        settings.log_retention_days = days;
    }

    // Recompile regex patterns before persisting
    settings.recompile();

    // Persist to database
    Settings::save(&app_state.pool, &settings).await?;

    // Update in-memory settings
    {
        let mut guard = app_state
            .settings
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        *guard = settings;
    }

    // Return current settings
    get_settings(State(app_state)).await
}

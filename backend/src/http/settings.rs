//! # Endpoints de configuración global
//!
//! Permite leer y actualizar la configuración global de la aplicación
//! (`default_rule_mode`, `log_retention_days`, `log_all_requests`).
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
    pub default_rule_mode: String,
    pub log_retention_days: i32,
    pub log_all_requests: String,
}

/// DTO for updating settings (all fields optional).
#[derive(Debug, Deserialize)]
pub struct UpdateSettingsPayload {
    pub default_rule_mode: Option<String>,
    pub log_retention_days: Option<i32>,
    pub log_all_requests: Option<String>,
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
        default_rule_mode: settings.default_rule_mode,
        log_retention_days: settings.log_retention_days,
        log_all_requests: settings.log_all_requests,
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

    // Validate log_all_requests
    if let Some(ref val) = update.log_all_requests {
        match val.as_str() {
            "all" | "pass" | "audit" => {},
            _ => {
                return Err(AppError::InvalidInput(
                    "log_all_requests must be 'all', 'pass', or 'audit'".to_string(),
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

    if let Some(mode) = update.default_rule_mode {
        settings.default_rule_mode = mode;
    }
    if let Some(days) = update.log_retention_days {
        settings.log_retention_days = days;
    }
    if let Some(val) = update.log_all_requests {
        settings.log_all_requests = val;
    }

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

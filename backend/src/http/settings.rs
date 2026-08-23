//! # Endpoints de configuración
//!
//! Permite leer y actualizar la configuración de retención de datos.
//! La configuración se almacena en la tabla `settings` de PostgreSQL.

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

use crate::models::{ApiResponse, AppState, Data, error::AppError};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub log_retention_days: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettings {
    pub log_retention_days: Option<i32>,
}

pub fn settings_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(get_settings))
        .route("/", routing::put(update_settings))
}

/// GET /api/v1/settings — Returns current settings
pub async fn get_settings(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = 'log_retention_days'")
        .fetch_optional(&app_state.pool)
        .await
        .map_err(AppError::from)?;

    let days: i32 = row
        .and_then(|r| r.get::<Option<String>, _>("value"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let settings = Settings {
        log_retention_days: days,
    };
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Settings",
        Data::Some(serde_json::to_value(settings).map_err(AppError::from)?),
    ))
}

/// PUT /api/v1/settings — Updates settings
pub async fn update_settings(
    State(app_state): State<Arc<AppState>>,
    Json(update): Json<UpdateSettings>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(days) = update.log_retention_days {
        if days < 1 || days > 365 {
            return Err(AppError::InvalidInput(
                "log_retention_days must be between 1 and 365".to_string(),
            ));
        }
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES ('log_retention_days', $1) 
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(days.to_string())
        .execute(&app_state.pool)
        .await
        .map_err(AppError::from)?;
    }

    let settings = get_settings(State(app_state)).await?;
    Ok(settings)
}

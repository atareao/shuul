//! # Endpoints de usuarios (solo lectura)
//!
//! Listado de usuarios y comprobación de existencia.
//! El login/registro se delega al SSO.

use std::sync::Arc;

use crate::models::error::AppError;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use tracing::debug;

use crate::models::{ApiResponse, AppState, Data, User};

/// Returns a router that exposes read‑only user endpoints.
///
/// * **Routes**
///   - `GET /` – List all users.
///   - `GET /any` – Check if at least one user exists.
pub fn api_user_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(read))
        .route("/any", routing::get(any_user_exists))
}

/// Checks whether at least one user exists in the database.
pub async fn any_user_exists(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let exists = User::any_user_exists(&app_state.pool).await?;
    debug!("Any user exists: {:?}", exists);
    let value = serde_json::json!({ "any_user_exists": exists });
    Ok(ApiResponse::new(StatusCode::OK, "Ok", Data::Some(value)))
}

/// Retrieves a list of all registered users.
pub async fn read(State(app_state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let values = User::read_all(&app_state.pool).await?;
    debug!("Users: {:?}", values);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Users",
        Data::Some(serde_json::to_value(values)?),
    ))
}

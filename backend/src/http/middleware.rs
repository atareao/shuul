//! # Middleware de autenticación
//!
//! Proporciona [`require_auth`], un middleware de Axum que valida el token JWT
//! en el header `Authorization` para todas las rutas protegidas.
//!
//! Las rutas públicas (health, auth, shuul, util, templates) se omiten.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::models::AppState;

/// Middleware que requiere un token JWT válido para acceder a rutas protegidas.
///
/// Rutas públicas (sin autenticación):
/// - `/api/v1/health`
/// - `/api/v1/auth`
/// - `/api/v1/shuul`
/// - `/api/v1/util`
/// - `/api/v1/templates`
pub async fn require_auth(
    State(app_state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for public paths
    let path = req.uri().path().to_string();
    if path.starts_with("/api/v1/health")
        || path.starts_with("/api/v1/auth")
        || path.starts_with("/api/v1/shuul")
        || path.starts_with("/api/v1/util")
        || path.starts_with("/api/v1/templates")
    {
        return Ok(next.run(req).await);
    }

    // Check Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token with JwtValidator
    app_state
        .jwt_validator
        .validate(auth_header)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(req).await)
}
//! # Middleware de autenticación
//!
//! Proporciona [`require_auth`], un middleware de Axum que valida el token JWT
//! en el header `Authorization` para todas las rutas protegidas.
//!
//! El token es emitido por shuul durante el callback SSO (HS256 con app_state.secret).
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
/// Valida JWTs emitidos por shuul (HS256 firmados con app_state.secret).
///
/// Rutas públicas (sin autenticación):
/// - `/health`
/// - `/auth`
/// - `/shuul`
/// - `/util`
/// - `/templates`
///
/// Nota: este middleware se monta en `api_routes`, que está anidado bajo `/api/v1`
/// en el router principal. Axum ya ha eliminado el prefijo `/api/v1` para cuando
/// este middleware se ejecuta, por lo que las rutas se comprueban sin ese prefijo.
pub async fn require_auth(
    State(app_state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for public paths
    // Paths are checked without the `/api/v1` prefix because Axum strips it
    // before reaching this middleware (it's applied on the nested `api_routes`).
    let path = req.uri().path().to_string();
    if path.starts_with("/health")
        || path.starts_with("/auth")
        || path.starts_with("/shuul")
        || path.starts_with("/util")
        || path.starts_with("/templates")
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

    // Validate as shuul's own JWT (HS256 with app_state.secret)
    use jsonwebtoken::{Algorithm, DecodingKey, Validation};
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    // Shuul's JWTs don't have iss/aud claims, so skip those checks
    validation.set_issuer(&[] as &[&str]);
    validation.set_audience(&[] as &[&str]);

    let decoding_key = DecodingKey::from_secret(app_state.secret.as_bytes());
    jsonwebtoken::decode::<serde_json::Value>(auth_header, &decoding_key, &validation)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(req).await)
}

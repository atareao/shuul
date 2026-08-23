//! # Endpoints de autenticación SSO (Single Sign-On)
//!
//! Proporciona rutas para iniciar sesión mediante un proveedor OIDC (PocketID):
//!
//! - `GET /sso` — Redirige al usuario al proveedor OIDC para autorización.
//! - `GET /callback` — Maneja el callback OIDC, intercambia el código por un token
//!   y devuelve una página HTML que almacena el JWT en `sessionStorage`.
//! - `GET /sso-status` — Indica si SSO está configurado.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing,
};
use rand::Rng;
use serde::Deserialize;
use tracing::{debug, error};

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Data, TokenClaims};

/// Router for SSO authentication endpoints.
pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sso", routing::get(sso_redirect))
        .route("/callback", routing::get(callback_handler))
        .route("/sso-status", routing::get(sso_status))
}

/// Query parameters for the OIDC callback.
#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// GET /api/v1/auth/sso — Redirect to PocketID authorize URL.
///
/// Generates a random state for CSRF protection, stores it in `oidc_states`,
/// and redirects the user to the OIDC provider's authorization endpoint.
pub async fn sso_redirect(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let metadata = app_state
        .oidc_metadata
        .as_ref()
        .ok_or_else(|| AppError::Other("SSO not configured".to_string()))?;

    let client_id = app_state
        .oidc_client_id
        .as_ref()
        .ok_or_else(|| AppError::Other("OIDC client ID not configured".to_string()))?;

    let redirect_url = app_state
        .oidc_redirect_url
        .as_deref()
        .unwrap_or("http://localhost:3000/api/v1/auth/callback");

    // Generate random state for CSRF protection
    let state: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Store state with timestamp for expiration (5 min)
    {
        let mut states = app_state.oidc_states.lock().await;
        states.insert(state.clone(), (String::new(), std::time::Instant::now()));
    }

    // Build authorization URL
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
        metadata.authorization_endpoint,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_url),
        urlencoding::encode(&state),
    );

    debug!("Redirecting to OIDC authorize URL: {}", auth_url);
    Ok(Redirect::to(&auth_url))
}

/// GET /api/v1/auth/callback — Handle OIDC callback.
///
/// Validates the state parameter, exchanges the authorization code for tokens,
/// fetches user info, creates a JWT, and returns an HTML page that stores the
/// token in `sessionStorage` and redirects to the admin panel.
pub async fn callback_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    // Check for error from provider
    if let Some(err) = &params.error {
        error!("OIDC provider returned error: {}", err);
        return Err(AppError::Other(format!("OIDC error: {err}")));
    }

    let code = params
        .code
        .as_ref()
        .ok_or_else(|| AppError::InvalidInput("Missing authorization code".to_string()))?;

    let state = params
        .state
        .as_ref()
        .ok_or_else(|| AppError::InvalidInput("Missing state parameter".to_string()))?;

    // Validate state (CSRF protection)
    {
        let mut states = app_state.oidc_states.lock().await;
        if states.remove(state).is_none() {
            return Err(AppError::InvalidInput("Invalid or expired state".to_string()));
        }
    }

    let metadata = app_state
        .oidc_metadata
        .as_ref()
        .ok_or_else(|| AppError::Other("SSO not configured".to_string()))?;

    let client_id = app_state
        .oidc_client_id
        .as_ref()
        .ok_or_else(|| AppError::Other("OIDC client ID not configured".to_string()))?;

    let client_secret = std::env::var("OIDC_CLIENT_SECRET")
        .map_err(|_| AppError::Other("OIDC_CLIENT_SECRET not set".to_string()))?;

    let redirect_url = app_state
        .oidc_redirect_url
        .as_deref()
        .unwrap_or("http://localhost:3000/api/v1/auth/callback");

    // Exchange authorization code for token
    let token_response = exchange_code_for_token(
        &metadata.token_endpoint,
        client_id,
        &client_secret,
        code,
        redirect_url,
    )
    .await?;

    // Fetch user info
    let userinfo = fetch_userinfo(&metadata.userinfo_endpoint, &token_response.access_token).await?;

    // Extract user info claims
    let email = userinfo
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@unknown.com");
    let sub = userinfo
        .get("sub")
        .and_then(|v| v.as_str())
        .unwrap_or(email);
    let role = userinfo
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");

    // Create JWT token (same format as existing login)
    let now = chrono::Utc::now();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let iat = now.timestamp() as usize;
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let exp = (now + chrono::Duration::minutes(60)).timestamp() as usize;

    let claims = TokenClaims {
        sub: sub.to_string(),
        role: role.to_string(),
        iat,
        exp,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(app_state.secret.as_bytes()),
    )
    .map_err(AppError::from)?;

    // Return HTML page that stores token in sessionStorage and redirects to admin
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Redirecting...</title>
</head>
<body>
    <script>
        sessionStorage.setItem("sso_token", "{}");
        window.location.href = "/admin/";
    </script>
    <p>Redirecting to admin panel...</p>
</body>
</html>"#,
        token
    );

    Ok(Html(html))
}

/// GET /api/v1/auth/sso-status — Returns whether SSO is configured.
pub async fn sso_status(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let sso_configured = app_state.oidc_metadata.is_some();
    let issuer_url = app_state
        .oidc_metadata
        .as_ref()
        .map(|m| m.issuer.clone())
        .unwrap_or_default();

    let value = serde_json::json!({
        "sso_configured": sso_configured,
        "issuer_url": issuer_url,
    });

    Ok(ApiResponse::new(StatusCode::OK, "SSO status", Data::Some(value)))
}

/// Response from the OIDC token endpoint.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Exchange an authorization code for an access token.
async fn exchange_code_for_token(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_url: &str,
) -> Result<TokenResponse, AppError> {
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_url),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let resp = client
        .post(token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Token exchange failed: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!("Token exchange error: {body}")));
    }

    let token_response: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Failed to parse token response: {e}")))?;

    Ok(token_response)
}

/// Fetch user info from the OIDC userinfo endpoint.
async fn fetch_userinfo(
    userinfo_endpoint: &str,
    access_token: &str,
) -> Result<serde_json::Value, AppError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(userinfo_endpoint)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Userinfo request failed: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!("Userinfo error: {body}")));
    }

    let userinfo: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Failed to parse userinfo: {e}")))?;

    Ok(userinfo)
}
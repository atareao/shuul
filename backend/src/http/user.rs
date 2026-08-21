//! # Endpoints de autenticación y usuarios
//!
//! Gestión de usuarios: login (JWT), logout, registro y listado.
//! Las contraseñas se hashean con bcrypt.

use std::sync::Arc;

use crate::models::error::AppError;
use axum::{
    Json, Router, body,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing,
};
use tracing::{debug, error};

use axum_extra::extract::cookie::{Cookie, SameSite};
use jsonwebtoken::{EncodingKey, Header, encode};

use crate::models::{ApiResponse, AppState, Data, TokenClaims, User, UserRegister, UserSchema};

/// Returns a router that exposes the authentication endpoints (login, logout, register).
///
/// * **Routes**
///   - `POST /login` – Authenticate a user and return a JWT.
///   - `GET  /logout` – Invalidate the session cookie.
///   - `POST /register` – Create a new user.
pub fn user_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", routing::post(login))
        .route("/logout", routing::get(logout))
        .route("/register", routing::post(register))
}

/// Returns a router that exposes read‑only user endpoints.
///
/// * **Routes**
///   - `GET /` – List all users.
///   - `GET /any` – Check if at least one user exists (useful for initial setup).
pub fn api_user_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(read))
        .route("/any", routing::get(any_user_exists))
}

/// Checks whether at least one user exists in the database.
///
/// * **Parameters**
///   - `app_state`: Shared application state (database pool).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with a boolean flag `any_user_exists`.
pub async fn any_user_exists(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let exists = User::any_user_exists(&app_state.pool).await?;
    debug!("Any user exists: {:?}", exists);
    let value = serde_json::json!({ "any_user_exists": exists });
    Ok(ApiResponse::new(StatusCode::OK, "Ok", Data::Some(value)))
}

/// Authenticates a user using email/password and returns a JWT token.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, secret, etc.).
///   - `user_schema`: JSON payload containing `email` and `password`.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with a JWT on success, or an error.
pub async fn login(
    State(app_state): State<Arc<AppState>>,
    Json(user_schema): Json<UserSchema>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("init login");
    tracing::info!("User schema: {:?}", user_schema);
    let user = User::get_by_email(&app_state.pool, &user_schema.email)
        .await
        .map_err(|e| {
            error!("Login error: {}", e);
            e
        })?;

    let valid =
        bcrypt::verify(&user_schema.password, &user.hashed_password).map_err(AppError::from)?;

    if !user.active || !valid {
        let message = "Invalid name or password. Please <a href='/login'>log in</a>";
        return Err(AppError::InvalidInput(message.to_string()));
    }

    let now = chrono::Utc::now();
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let iat = now.timestamp() as usize;
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let exp = (now + chrono::Duration::minutes(60)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: user.email.clone(),
        role: user.role,
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(app_state.secret.as_bytes()),
    )?;

    let value = serde_json::json!({"token": token});
    Ok(ApiResponse::new(StatusCode::OK, "Ok", Data::Some(value)))
}

/// Registers a new user in the system.
///
/// Validates that the email contains an '@' symbol and that registration
/// is disabled when SSO is configured.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool, secret, etc.).
///   - `user_data`: JSON payload containing `username`, `email`, `password`, `role`.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the created user or an error.
pub async fn register(
    State(app_state): State<Arc<AppState>>,
    Json(user_data): Json<UserRegister>,
) -> Result<impl IntoResponse, AppError> {
    // Disable registration if SSO is configured
    if app_state.oidc_metadata.is_some() {
        return Err(AppError::InvalidInput(
            "Registration is disabled. Use SSO to sign in.".to_string(),
        ));
    }

    // Validate email format (must contain @)
    if !user_data.email.contains('@') {
        return Err(AppError::InvalidInput(
            format!("Invalid email: '{}' must contain '@'", user_data.email),
        ));
    }

    debug!("User data: {:?}", user_data);
    let user = User::create(
        &app_state.pool,
        &user_data.username,
        &user_data.email,
        &user_data.password,
        &user_data.role,
    )
    .await?;
    debug!("User created: {:?}", user);
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "User created",
        Data::Some(serde_json::to_value(user)?),
    ))
}

/// Logs the user out by expiring the authentication cookie.
///
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – HTTP 303 redirect to `/` with a cleared `token` cookie.
pub async fn logout() -> Result<impl IntoResponse, AppError> {
    debug!("Logout");
    let cookie = Cookie::build(("token", ""))
        .path("/")
        .max_age(cookie::time::Duration::hours(-1))
        .same_site(SameSite::Lax)
        .http_only(true)
        .build();

    tracing::info!("The cookie: {}", cookie.to_string());

    let response = Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/")
        .header(header::SET_COOKIE, cookie.to_string())
        .body(body::Body::empty())
        .map_err(|e| AppError::Other(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Retrieves a list of all registered users.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON array of users or an error.
pub async fn read(State(app_state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let values = User::read_all(&app_state.pool).await?;
    debug!("Users: {:?}", values);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Users",
        Data::Some(serde_json::to_value(values)?),
    ))
}

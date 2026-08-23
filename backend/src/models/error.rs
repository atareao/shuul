//! # Tipos de error centralizados
//!
//! Define [`AppError`], el enum de error principal del backend que agrupa
//! todos los errores posibles (base de datos, bcrypt, JWT, I/O, serialización, etc.).
//!
//! Implementa [`IntoResponse`] para convertir cada variante en una respuesta
//! HTTP con el código de estado apropiado.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::env::VarError;
use thiserror::Error;

/// Central error type for the backend.
///
/// Cada variante envuelve un error concreto del ecosistema.
/// El atributo `#[from]` genera automáticamente la conversión `From<T>`
/// para que el operador `?` convierta el error subyacente en `AppError`.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Error de base de datos: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Error JWT: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("Error de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Error de Serialización JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Entrada inválida: {0}")]
    InvalidInput(String),

    #[error("Variable de entorno faltante: {0}")]
    EnvVar(#[from] VarError),

    #[error("Mutex lock poisoned")]
    CachePoisoned,

    #[error("Otro error: {0}")]
    Other(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Jwt(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::SerdeJson(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::EnvVar(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::CachePoisoned => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Other(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = serde_json::json!({
            "status": status.as_u16(),
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}

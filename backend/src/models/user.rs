//! # Modelo de usuarios
//!
//! Define [`TokenClaims`] para la validación de JWT.
//! El login/registro se delega al SSO vía OIDC (PocketID).

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub role: String,
    pub iat: usize,
    pub exp: usize,
}

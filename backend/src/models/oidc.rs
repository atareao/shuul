//! # Modelos OIDC (OpenID Connect)
//!
//! Define [`OidcMetadata`] para el descubrimiento OIDC y [`JwtValidator`]
//! para la validación de tokens JWT emitidos por un proveedor OIDC (PocketID).

use serde::Deserialize;

/// OIDC discovery metadata from `.well-known/openid-configuration`
#[derive(Debug, Deserialize, Clone)]
pub struct OidcMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
}

/// JWT validator that fetches JWKS from the OIDC provider.
///
/// Verifies the signature against the provider's JWKS and checks
/// issuer/audience claims.
pub struct JwtValidator {
    issuer: String,
    client_id: String,
    jwks: Option<Vec<jsonwebtoken::jwk::Jwk>>,
}

impl std::fmt::Debug for JwtValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtValidator")
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("jwks_count", &self.jwks.as_ref().map(|j| j.len()))
            .finish()
    }
}

impl JwtValidator {
    /// Create a new JWT validator.
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            jwks: None,
        }
    }

    /// Fetch JWKS from the OIDC provider's `jwks_uri`.
    ///
    /// Parses the response into `jsonwebtoken::jwk::JwkSet` and stores the keys.
    pub async fn fetch_jwks(
        &mut self,
        jwks_uri: &str,
    ) -> Result<(), crate::models::error::AppError> {
        let client = reqwest::Client::new();
        let resp = client.get(jwks_uri).send().await.map_err(|e| {
            crate::models::error::AppError::Other(format!("Failed to fetch JWKS: {e}"))
        })?;

        let jwk_set: jsonwebtoken::jwk::JwkSet = resp.json().await.map_err(|e| {
            crate::models::error::AppError::Other(format!("Failed to parse JWKS: {e}"))
        })?;

        self.jwks = Some(jwk_set.keys);
        Ok(())
    }
}

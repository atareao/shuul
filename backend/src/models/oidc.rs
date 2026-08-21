//! # Modelos OIDC (OpenID Connect)
//!
//! Define [`OidcMetadata`] para el descubrimiento OIDC y [`JwtValidator`]
//! para la validación de tokens JWT emitidos por un proveedor OIDC (PocketID).

use serde::{Deserialize, Serialize};

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
/// In dev mode (`is_dev = true`), any token is accepted (no signature verification).
/// In production, it verifies the signature against the provider's JWKS and checks
/// issuer/audience claims.
pub struct JwtValidator {
    issuer: String,
    client_id: String,
    jwks: Option<Vec<jsonwebtoken::jwk::Jwk>>,
    is_dev: bool,
}

impl std::fmt::Debug for JwtValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtValidator")
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("jwks_count", &self.jwks.as_ref().map(|j| j.len()))
            .field("is_dev", &self.is_dev)
            .finish()
    }
}

impl JwtValidator {
    /// Create a dev-mode validator (accepts any token without verification).
    pub fn dev() -> Self {
        Self {
            issuer: String::new(),
            client_id: String::new(),
            jwks: None,
            is_dev: true,
        }
    }

    /// Create a production validator.
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            jwks: None,
            is_dev: false,
        }
    }

    /// Fetch JWKS from the OIDC provider's `jwks_uri`.
    ///
    /// Parses the response into `jsonwebtoken::jwk::JwkSet` and stores the keys.
    pub async fn fetch_jwks(&mut self, jwks_uri: &str) -> Result<(), crate::models::error::AppError> {
        let client = reqwest::Client::new();
        let resp = client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| crate::models::error::AppError::Other(format!("Failed to fetch JWKS: {e}")))?;

        let jwk_set: jsonwebtoken::jwk::JwkSet = resp
            .json()
            .await
            .map_err(|e| crate::models::error::AppError::Other(format!("Failed to parse JWKS: {e}")))?;

        self.jwks = Some(jwk_set.keys);
        Ok(())
    }

    /// Validate a JWT token.
    ///
    /// In dev mode, just decode the token and return the claims as a JSON value.
    /// In production, find the matching JWK by `kid`, verify the signature, and
    /// validate the `iss` and `aud` claims.
    pub fn validate(&self, token: &str) -> Result<serde_json::Value, crate::models::error::AppError> {
        // Decode without verification first to get header and claims
        let header = jsonwebtoken::decode_header(token)
            .map_err(|_| crate::models::error::AppError::InvalidInput("Invalid JWT header".to_string()))?;

        if self.is_dev {
            // Dev mode: just decode and return claims
            let mut validation = jsonwebtoken::Validation::default();
            validation.insecure_disable_signature_validation();
            let token_data = jsonwebtoken::decode::<serde_json::Value>(
                token,
                &jsonwebtoken::DecodingKey::from_secret(&[]),
                &validation,
            )
            .map_err(|e| crate::models::error::AppError::Jwt(e))?;
            return Ok(token_data.claims);
        }

        // Production mode: verify signature against JWKS
        let jwks = self
            .jwks
            .as_ref()
            .ok_or_else(|| crate::models::error::AppError::Other("JWKS not loaded".to_string()))?;

        // Find the key by `kid` from the header
        let kid = header.kid.as_deref().unwrap_or("");
        let jwk = jwks
            .iter()
            .find(|k| k.common.key_id.as_deref() == Some(kid))
            .ok_or_else(|| crate::models::error::AppError::Other(format!("No JWK found for kid: {kid}")))?;

        // Convert JWK to decoding key
        let decoding_key = jsonwebtoken::DecodingKey::from_jwk(jwk)
            .map_err(|e| crate::models::error::AppError::Other(format!("Failed to create decoding key: {e}")))?;

        // Validate with issuer and audience checks
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.client_id]);
        validation.validate_exp = true;

        let token_data = jsonwebtoken::decode::<serde_json::Value>(token, &decoding_key, &validation)
            .map_err(|e| crate::models::error::AppError::Jwt(e))?;

        Ok(token_data.claims)
    }
}
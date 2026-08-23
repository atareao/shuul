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

    /// Validate an `id_token` from the OIDC provider.
    ///
    /// Verifies:
    /// - Signature against the provider's JWKS (matched by `kid`)
    /// - `iss` claim matches the expected issuer
    /// - `aud` claim matches the expected client_id
    /// - `exp` claim is not expired
    ///
    /// Returns the decoded claims on success.
    pub fn validate_id_token(
        &self,
        id_token: &str,
    ) -> Result<serde_json::Value, crate::models::error::AppError> {
        let jwks = self.jwks.as_ref().ok_or_else(|| {
            crate::models::error::AppError::Other("JWKS not initialized".to_string())
        })?;

        // Decode header to find the key ID (kid)
        let header = jsonwebtoken::decode_header(id_token).map_err(|e| {
            crate::models::error::AppError::Other(format!("Failed to decode id_token header: {e}"))
        })?;

        // Find matching JWK by kid, or fall back to the first key
        let jwk = if let Some(kid) = &header.kid {
            jwks.iter()
                .find(|j| j.common.key_id == Some(kid.clone()))
                .ok_or_else(|| {
                    crate::models::error::AppError::Other(format!(
                        "No matching JWK found for kid: {kid}"
                    ))
                })?
        } else {
            jwks.first().ok_or_else(|| {
                crate::models::error::AppError::Other("No JWKs available".to_string())
            })?
        };

        // Create decoding key from the JWK
        let key = jsonwebtoken::DecodingKey::from_jwk(jwk).map_err(|e| {
            crate::models::error::AppError::Other(format!(
                "Failed to create DecodingKey from JWK: {e}"
            ))
        })?;

        // Validate with expected issuer and audience
        let mut validation = jsonwebtoken::Validation::new(header.alg);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.client_id]);
        validation.validate_exp = true;

        let token_data = jsonwebtoken::decode::<serde_json::Value>(id_token, &key, &validation)
            .map_err(|e| {
                crate::models::error::AppError::Other(format!("id_token validation failed: {e}"))
            })?;

        Ok(token_data.claims)
    }
}

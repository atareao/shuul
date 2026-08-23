//! # Modelos de datos
//!
//! Define las estructuras principales del dominio: `User`, `Rule`, `Request`,
//! y los tipos de respuesta de la API (`ApiResponse`, `PagedResponse`, etc.).
//!
//! También contiene el tipo de error central [`AppError`] y el estado
//! compartido de la aplicación ([`AppState`]).

mod ban_manager;
mod data;
pub mod error;
mod ipdata;
mod oidc;
mod rate_limiter;
mod request;
mod response;
mod rule;
mod user;

pub use ban_manager::BanManager;
pub use data::Data;
pub use error::AppError as Error;
pub use ipdata::IPData;
pub use oidc::{JwtValidator, OidcMetadata};
#[allow(unused_imports)]
pub use rate_limiter::CircularTimestamps;
pub use rate_limiter::RateLimiter;
pub use request::{NewRequest, ReadRequestParams, Request};
pub use response::{ApiResponse, EmptyResponse, PagedResponse, Pagination};
pub use rule::{CacheRule, NewRule, ReadRuleParams, Rule, UpdateRule};
pub use user::TokenClaims;

use maxminddb::Reader;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tracing::debug;

pub struct AppState {
    pub pool: PgPool,
    pub secret: String,
    pub maxmind_db: Reader<Vec<u8>>,
    pub rules: Mutex<Vec<CacheRule>>,
    pub cache: Mutex<Vec<NewRequest>>,
    pub cache_enabled: bool,
    pub cache_size: usize,
    pub ban_manager: Mutex<BanManager>,
    pub rate_limiter: Mutex<HashMap<i32, RateLimiter>>, // rule_id → RateLimiter
    // SSO / OIDC fields
    pub oidc_metadata: tokio::sync::RwLock<Option<OidcMetadata>>,
    pub jwt_validator: tokio::sync::RwLock<Option<JwtValidator>>,
    pub oidc_states: tokio::sync::Mutex<HashMap<String, (String, Instant)>>,
    pub oidc_client_id: Option<String>,
    pub oidc_redirect_url: Option<String>,
}

impl AppState {
    /// Recarga la caché de reglas desde la base de datos.
    ///
    /// Solo se cargan reglas activas, ordenadas por peso ascendente (`ORDER BY weight ASC`).
    /// Es seguro contra poison del `Mutex`.
    pub async fn reload_rules(&self) -> Result<(), Error> {
        let rules = CacheRule::read_all_active(&self.pool).await?;
        let mut guard = self.rules.lock().map_err(|_| Error::CachePoisoned)?;
        *guard = rules;
        debug!("Rules cache reloaded: {} rules active", guard.len());
        Ok(())
    }
}

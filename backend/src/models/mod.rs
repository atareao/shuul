//! # Modelos de datos
//!
//! Define las estructuras principales del dominio: `User`, `Rule`, `NewRequest`,
//! y los tipos de respuesta de la API (`ApiResponse`, `PagedResponse`, etc.).
//!
//! También contiene el tipo de error central [`AppError`] y el estado
//! compartido de la aplicación ([`AppState`]).

mod ban_manager;
mod data;
pub mod error;
mod ipdata;
mod new_request;
mod oidc;
mod rate_limit_profile;
mod rate_limiter;
mod report;
mod response;
mod rule;
mod settings;
mod stats;
mod user;

pub use ban_manager::BanManager;
pub use data::Data;
pub use error::AppError as Error;
pub use ipdata::IPData;
pub use new_request::NewRequest;
pub use oidc::{JwtValidator, OidcMetadata};
pub use rate_limit_profile::{
    NewRateLimitProfile, RateLimitProfile, ReadRateLimitProfileParams, UpdateRateLimitProfile,
};
#[allow(unused_imports)]
pub use rate_limiter::CircularTimestamps;
pub use rate_limiter::RateLimiter;
pub use report::ReportPayload;
pub use response::{ApiResponse, EmptyResponse, PagedResponse, Pagination};
#[allow(unused_imports)]
pub use rule::{CacheRule, NewRule, ReadRuleParams, Rule, UpdateRule};
pub use settings::Settings;
pub use stats::StatsCollector;
pub use user::TokenClaims;

use maxminddb::Reader;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct AppState {
    pub pool: SqlitePool,
    pub secret: String,
    pub maxmind_db: Reader<Vec<u8>>,
    pub rules: Mutex<Vec<CacheRule>>,
    pub stats: StatsCollector,
    #[allow(dead_code)]
    pub static_dir: String,
    pub ban_manager: Mutex<BanManager>,
    pub rate_limiter: Mutex<HashMap<i32, RateLimiter>>, // rule_id → RateLimiter
    pub settings: Mutex<Settings>,
    // SSO / OIDC fields
    pub oidc_metadata: tokio::sync::RwLock<Option<OidcMetadata>>,
    pub jwt_validator: tokio::sync::RwLock<Option<JwtValidator>>,
    pub oidc_states: tokio::sync::Mutex<HashMap<String, (String, Instant)>>,
    pub oidc_client_id: Option<String>,
    pub oidc_redirect_url: Option<String>,
}

impl AppState {
    /// Reloads the in-memory rules cache from the database.
    pub async fn reload_rules(&self) -> Result<(), Error> {
        let rules = CacheRule::read_all_active(&self.pool).await.map_err(|e| {
            tracing::error!("Failed to reload rules: {e}");
            Error::Other(format!("Failed to reload rules: {e}"))
        })?;
        if let Ok(mut guard) = self.rules.lock() {
            *guard = rules;
        }
        Ok(())
    }
}

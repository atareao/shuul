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
mod rate_limiter;
mod request;
mod response;
mod rule;
mod user;

pub use ban_manager::{BanInfo, BanManager};
pub use data::Data;
pub use error::AppError as Error;
pub use ipdata::IPData;
pub use rate_limiter::{CircularTimestamps, RateLimiter};
pub use request::{NewRequest, ReadRequestParams, Request};
pub use response::{ApiResponse, EmptyResponse, PagedResponse, Pagination};
pub use rule::{CacheRule, NewRule, ReadRuleParams, Rule, UpdateRule};
pub use user::{TokenClaims, User, UserRegister, UserSchema};

use maxminddb::Reader;
use sqlx::postgres::PgPool;
use std::sync::Mutex;

pub struct AppState {
    pub pool: PgPool,
    pub secret: String,
    pub maxmind_db: Reader<Vec<u8>>,
    pub rules: Mutex<Vec<CacheRule>>,
    pub cache: Mutex<Vec<NewRequest>>,
    pub cache_enabled: bool,
    pub cache_size: usize,
    pub static_dir: String,
}

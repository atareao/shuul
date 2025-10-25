mod user;
mod rule;
mod request;
mod response;
mod data;
mod ipdata;

pub use data::Data;
pub use ipdata::IPData;
pub use rule::{Rule, NewRule, UpdateRule, ReadRuleParams};
pub use request::{Request, NewRequest, ReadRequestParams};
pub use response::{ApiResponse, EmptyResponse, PagedResponse, Pagination};
pub use user::{User, TokenClaims, UserSchema, UserRegister};
pub type Error = Box<dyn std::error::Error>;

use sqlx::postgres::PgPool;
use maxminddb::Reader;
use std::sync::Mutex;

pub struct AppState {
    pub pool: PgPool,
    pub secret: String,
    pub maxmind_db: Reader<Vec<u8>>,
    pub rules: Mutex<Vec<Rule>>,
    pub cache: Mutex<Vec<NewRequest>>,
    pub cache_enabled: bool,
    pub cache_size: usize,
    pub static_dir: String,
}

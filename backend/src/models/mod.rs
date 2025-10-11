mod user;
mod rule;
mod record;
mod api_response;
mod data;
mod ipdata;

pub use data::Data;
pub use ipdata::IPData;
pub use rule::{Rule, NewRule, UpdateRule};
pub use record::{Record, NewRecord};
pub use api_response::{ApiResponse, EmptyResponse};
pub use user::{User, TokenClaims, UserSchema, UserRegister};
pub type Error = Box<dyn std::error::Error>;

use sqlx::postgres::PgPool;
use maxminddb::Reader;

pub struct AppState {
    pub pool: PgPool,
    pub secret: String,
    pub maxmind_db: Reader<Vec<u8>>,
    pub static_dir: String,
}

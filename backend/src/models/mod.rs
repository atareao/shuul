mod user;
mod rule;
mod record;
mod response;
mod data;
mod ipdata;
mod ignored;

pub use data::Data;
pub use ipdata::IPData;
pub use rule::{Rule, NewRule, UpdateRule, ReadRuleParams};
pub use ignored::{Ignored, NewIgnored, UpdateIgnored, ReadIgnoredParams};
pub use record::{Record, NewRecord, ReadRecordParams};
pub use response::{ApiResponse, EmptyResponse, PagedResponse, Pagination};
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

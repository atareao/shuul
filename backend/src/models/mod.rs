mod user;
mod rule;
mod record;
mod api_response;
mod data;

pub use data::Data;
pub use api_response::{ApiResponse, CustomResponse};
pub use user::{User, TokenClaims, UserSchema, UserRegister};
pub type Error = Box<dyn std::error::Error>;

use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub secret: String,
    pub static_dir: String,
}

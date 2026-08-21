//! # Handlers HTTP
//!
//! Módulos que definen los endpoints de la API REST:
//!
//! - [`health`] — Health check
//! - [`user`] — Autenticación (login, logout, registro) y gestión de usuarios
//! - [`rule`] — CRUD de reglas de filtrado
//! - [`request`] — Consulta de peticiones HTTP capturadas
//! - [`shuul`] — Endpoint principal de captura y filtrado
//! - [`util`] — Utilidades (geolocalización, etc.)

mod ban;
mod health;
mod request;
mod rule;
mod shuul;
mod user;
mod util;

pub use ban::ban_router;
pub use health::health_router;
pub use request::request_router;
pub use rule::rule_router;
pub use shuul::shuul_router;
pub use user::{api_user_router, user_router};
pub use util::util_router;

//! # Handlers HTTP
//!
//! Módulos que definen los endpoints de la API REST:
//!
//! - [`health`] — Health check
//! - [`user`] — Autenticación (login, logout, registro) y gestión de usuarios
//! - [`auth`] — SSO / OIDC (Single Sign-On)
//! - [`rule`] — CRUD de reglas de filtrado
//! - [`request`] — Consulta de peticiones HTTP capturadas
//! - [`shuul`] — Endpoint principal de captura y filtrado
//! - [`util`] — Utilidades (geolocalización, etc.)

mod auth;
mod ban;
mod health;
mod middleware;
mod request;
mod rule;
mod settings;
mod shuul;
mod template;
mod util;

pub use auth::auth_router;
pub use ban::ban_router;
pub use health::health_router;
pub use middleware::require_auth;
pub use request::request_router;
pub use rule::rule_router;
pub use settings::settings_router;
pub use shuul::shuul_router;
pub use template::template_router;
pub use util::util_router;

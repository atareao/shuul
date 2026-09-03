//! # Handlers HTTP
//!
//! Módulos que definen los endpoints de la API REST:
//!
//! - [`health`] — Health check
//! - [`auth`] — SSO / OIDC (Single Sign-On)
//! - [`rule`] — CRUD de reglas de filtrado
//! - [`rate_limit_profile`] — CRUD de perfiles de rate limiting
//! - [`request`] — Consulta de peticiones HTTP capturadas
//! - [`stats`] — Estadísticas agregadas (StatsCollector, en memoria)
//! - [`shuul`] — Endpoint principal de captura y filtrado
//! - [`settings`] — Configuración global
//! - [`template`] — Plantillas de reglas y perfiles
//! - [`util`] — Utilidades (geolocalización, etc.)

mod auth;
mod ban;
mod health;
mod middleware;
mod rate_limit_profile;
mod report;
mod rule;
mod settings;
mod shuul;
mod stats;
mod template;
mod util;

pub use auth::auth_router;
pub use ban::ban_router;
pub use health::health_router;
pub use middleware::require_auth;
pub use rate_limit_profile::rate_limit_profile_router;
pub use report::report_router;
pub use rule::rule_router;
pub use settings::settings_router;
pub use shuul::shuul_router;
pub use stats::stats_router;
pub use template::template_router;
pub use util::util_router;

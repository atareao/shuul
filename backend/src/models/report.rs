//! # Modelo de reportes
//!
//! Define [`ReportPayload`], la estructura que utiliza el plugin de Traefik
//! para reportar status codes reales del backend.

use serde::Deserialize;

/// Payload enviado por el plugin de Traefik para reportar
/// el resultado de una petición HTTP real al backend.
#[derive(Debug, Deserialize)]
pub struct ReportPayload {
    pub ip_address: String,
    pub status_code: u16,
    pub path: Option<String>,
    pub method: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub fqdn: Option<String>,
    pub query: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub protocol: Option<String>,
}

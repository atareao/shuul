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
}

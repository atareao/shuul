//! # Modelo de nueva petición HTTP
//!
//! Define [`NewRequest`], la estructura utilizada para el matcheo
//! de reglas (WAF y Jail). No representa almacenamiento en BD.

use chrono::{DateTime, Utc};
use http::Uri;
use serde::{Deserialize, Serialize};

use super::GeoIpService;
use super::TorService;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRequest {
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub is_tor: Option<bool>,
    pub rule_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl NewRequest {
    /// Construye un `NewRequest` a partir de los encabezados HTTP y la DB `GeoIP`.
    ///
    /// Los encabezados `x-forwarded-*` son la fuente principal de datos.
    /// Para campos de encabezado HTTP estándar, se usa primero el prefijo
    /// `x-forwarded-*` y como fallback el encabezado original.
    /// Build a `NewRequest` from HTTP headers and optional `GeoIP` data.
    #[allow(clippy::too_many_lines, clippy::option_if_let_else)]
    pub fn from_request(
        headers: &http::HeaderMap,
        geoip: Option<&GeoIpService>,
        tor_service: Option<&TorService>,
    ) -> Self {
        let protocol = headers
            .get("x-forwarded-proto")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .unwrap_or("");
        let host = headers
            .get("x-forwarded-host")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .unwrap_or("");
        let uri = headers
            .get("x-forwarded-uri")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .unwrap_or("")
            .parse::<Uri>()
            .unwrap_or_default();
        let ip = headers
            .get("x-forwarded-for")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .unwrap_or("");
        let ip_address = if ip.is_empty() {
            None
        } else {
            Some(ip.to_string())
        };
        let protocol = if protocol.is_empty() {
            None
        } else {
            Some(protocol.to_string())
        };
        let fqdn = if host.is_empty() {
            None
        } else {
            Some(host.to_string())
        };
        let path = if uri.path().is_empty() {
            None
        } else {
            Some(uri.path().to_string())
        };
        let query = uri.query().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });

        // GeoIP lookup: usa el cache de GeoIpService si está disponible
        let (city_name, country_name, country_code) = if let Some(geoip) = geoip {
            let ip_data = geoip.lookup(ip);
            (
                ip_data.city_name.filter(|s| !s.is_empty()),
                ip_data.country_name.filter(|s| !s.is_empty()),
                ip_data.country_code.filter(|s| !s.is_empty()),
            )
        } else {
            (None, None, None)
        };

        // Tor lookup: usa TorService si está disponible
        let is_tor = tor_service.and_then(|ts| {
            if ip.is_empty() {
                None
            } else {
                ts.is_exit_node(ip)
            }
        });

        // Extraer encabezados HTTP adicionales con prefijo x-forwarded-* + fallback
        let user_agent = headers
            .get("x-forwarded-user-agent")
            .or_else(|| headers.get("user-agent"))
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let method = headers
            .get("x-forwarded-method")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let referer = headers
            .get("x-forwarded-referer")
            .or_else(|| headers.get("referer"))
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let content_type = headers
            .get("x-forwarded-content-type")
            .or_else(|| headers.get("content-type"))
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let accept_language = headers
            .get("x-forwarded-accept-language")
            .or_else(|| headers.get("accept-language"))
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        let x_request_id = headers
            .get("x-forwarded-x-request-id")
            .or_else(|| headers.get("x-request-id"))
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);

        Self {
            ip_address,
            protocol,
            fqdn,
            path,
            query,
            city_name,
            country_name,
            country_code,
            user_agent,
            method,
            referer,
            content_type,
            accept_language,
            x_request_id,
            is_tor,
            rule_id: None,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build HTTP headers for testing.
    fn build_headers(ip: &str, host: &str, uri: &str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-for", ip.parse().unwrap());
        headers.insert("x-forwarded-host", host.parse().unwrap());
        headers.insert("x-forwarded-uri", uri.parse().unwrap());
        headers
    }

    /// Escenario: TorService cargado con IP Tor conocida.
    /// `is_tor` debe ser `Some(true)`.
    #[test]
    fn test_from_request_with_tor_service_returns_some_true() {
        let headers = build_headers("185.220.101.1", "example.com", "/test");
        let tor_service = TorService::new();
        tor_service.load_from_str("185.220.101.1\n185.220.101.2\n");

        let request = NewRequest::from_request(&headers, None, Some(&tor_service));

        assert_eq!(request.is_tor, Some(true));
        assert_eq!(request.ip_address.as_deref(), Some("185.220.101.1"));
    }

    /// Escenario: TorService es `None`.
    /// `is_tor` debe ser `None`.
    #[test]
    fn test_from_request_without_tor_service_returns_none() {
        let headers = build_headers("185.220.101.1", "example.com", "/test");

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.is_tor, None);
        assert_eq!(request.ip_address.as_deref(), Some("185.220.101.1"));
    }

    /// Escenario: TorService disponible pero sin IP en headers.
    /// `is_tor` debe ser `None` (no se puede consultar sin IP).
    #[test]
    fn test_from_request_without_ip_returns_none() {
        let headers = http::HeaderMap::new(); // Sin x-forwarded-for
        let tor_service = TorService::new();
        tor_service.load_from_str("185.220.101.1\n");

        let request = NewRequest::from_request(&headers, None, Some(&tor_service));

        assert_eq!(request.is_tor, None);
        assert_eq!(request.ip_address, None);
    }
}

//! # Modelo de nueva petición HTTP
//!
//! Define [`NewRequest`], la estructura utilizada para el matcheo
//! de reglas (WAF y Jail). No representa almacenamiento en BD.

use chrono::{DateTime, Utc};
use http::Uri;
use serde::{Deserialize, Serialize};
use tracing::debug;

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
    ///
    /// Los cinco campos derivados de cabecera —`user_agent`, `referer`,
    /// `content_type`, `accept_language` y `x_request_id`— se resuelven a `None`
    /// cuando la cabecera falta o está presente pero vacía: el filtro
    /// `.filter(|s| !s.is_empty())` colapsa ambos casos a `None`. A efectos del
    /// matcheo, una cabecera ausente y una presente pero vacía son indistinguibles.
    ///
    /// **Todos** los consumidores de la dirección del cliente parten de la
    /// dirección resuelta (`ip_address`: el último elemento de `x-forwarded-for`,
    /// ya validado como `IpAddr`), nunca de la cabecera cruda. Tanto la consulta
    /// `GeoIP` como la de Tor se hacen contra `ip_address`, y ninguna se intenta
    /// cuando ésta es `None`: sin dirección válida no hay nada que consultar. El
    /// único diagnóstico que puede emitirse en ese camino es el del resolvedor, y
    /// solo cuando **había un valor no vacío que no resolvió**: una cabecera
    /// ausente o vacía colapsa a `None` sin producir diagnóstico alguno.
    #[allow(clippy::too_many_lines)]
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
        let ip_address = match ip.trim() {
            "" => None,
            raw => {
                // `X-Forwarded-For` es una cadena de saltos: se toma el ÚLTIMO elemento, que es el
                // que observó el salto más próximo — el mismo del que el pipeline Jail deriva la
                // dirección al leer `ClientHost`/`ClientAddr` del access log. Si no coincidieran, el
                // WAF comprobaría los baneos contra una dirección distinta de la que el Jail usó para
                // crearlos, y no se aplicarían nunca. El primer elemento lo puede anteponer el
                // cliente, así que decidiría la dirección quien no debe.
                let candidate = raw.rsplit(',').next().unwrap_or("").trim();
                if candidate.parse::<std::net::IpAddr>().is_ok() {
                    Some(candidate.to_string())
                } else {
                    debug!(
                        field = "ip_address",
                        value = %candidate,
                        header = %raw,
                        "dirección del cliente descartada: no es una dirección IP"
                    );
                    None
                }
            },
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

        // GeoIP lookup: usa el cache de GeoIpService si está disponible y hay
        // una dirección resuelta contra la que consultar
        let (city_name, country_name, country_code) = match (geoip, ip_address.as_deref()) {
            (Some(geoip), Some(resolved_ip)) => {
                let ip_data = geoip.lookup(resolved_ip);
                (
                    ip_data.city_name.filter(|s| !s.is_empty()),
                    ip_data.country_name.filter(|s| !s.is_empty()),
                    ip_data.country_code.filter(|s| !s.is_empty()),
                )
            },
            _ => (None, None, None),
        };

        // Tor lookup: usa TorService si está disponible y hay una dirección
        // resuelta contra la que consultar
        let is_tor = match (tor_service, ip_address.as_deref()) {
            (Some(ts), Some(resolved_ip)) => ts.is_exit_node(resolved_ip),
            _ => None,
        };

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
    use std::sync::{Arc, Mutex};

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

    // ── captura de logs (copia privada del patrón de `rule.rs`) ──

    #[derive(Clone, Default)]
    struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Ejecuta `f` con un subscriber a nivel DEBUG que graba cada evento y
    /// devuelve lo escrito. Es el único modo de observar el diagnóstico del
    /// resolvedor de la dirección del cliente.
    fn capture_debug<F: FnOnce()>(f: F) -> String {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let writer = CapturedLog(Arc::clone(&sink));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || CapturedLog(Arc::clone(&writer.0)))
            .finish();

        tracing::subscriber::with_default(subscriber, f);

        let bytes = sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        String::from_utf8_lossy(&bytes).to_string()
    }

    /// Cabeceras con `x-forwarded-for` en el valor indicado. No se inserta la
    /// cabecera cuando el valor es `None`, para poder probar el caso ausente.
    fn headers_with_forwarded_for(value: Option<&str>) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        if let Some(v) = value {
            headers.insert("x-forwarded-for", v.parse().unwrap());
        }
        headers
    }

    /// Carga el servicio `GeoIP` desde el fichero mmdb, probando varias rutas.
    ///
    /// El fichero `GeoLite2-City.mmdb` es un **requisito declarado de la suite**:
    /// los tests que consultan `GeoIP` no pueden ejercer sus aserciones sin él, así
    /// que su ausencia es un fallo explícito, nunca un salto silencioso.
    /// `.github/workflows/ci.yml` lo descarga en `geo/` cuando el secreto
    /// `MAXMIND_LICENSE_KEY` está definido (y omite el paso de tests cuando no lo
    /// está).
    ///
    /// # Panics
    ///
    /// Entra en pánico si no encuentra el fichero en ninguna de las rutas
    /// probadas —`geo/`, `/app/geo/` y `../geo/`, relativas a `backend/`—, con un
    /// mensaje accionable que nombra las rutas y el mecanismo de descarga.
    fn require_geoip_service() -> GeoIpService {
        let paths = [
            "geo/GeoLite2-City.mmdb",
            "/app/geo/GeoLite2-City.mmdb",
            "../geo/GeoLite2-City.mmdb",
        ];
        for path in &paths {
            if let Ok(reader) = maxminddb::Reader::open_readfile(path) {
                return GeoIpService::new(reader);
            }
        }
        panic!(
            "falta la base GeoLite2-City.mmdb, que esta suite requiere: se ha buscado en \
             `geo/GeoLite2-City.mmdb`, `/app/geo/GeoLite2-City.mmdb` y \
             `../geo/GeoLite2-City.mmdb` (relativas a `backend/`). `.github/workflows/ci.yml` \
             la descarga en `geo/` cuando el secreto `MAXMIND_LICENSE_KEY` está definido, y \
             omite el paso de tests cuando no lo está. Define el secreto o coloca el fichero \
             en `backend/geo/GeoLite2-City.mmdb` para ejecutar la suite completa."
        )
    }

    /// Escenario: Un solo valor.
    /// `X-Forwarded-For: 203.0.113.7` → `ip_address == Some("203.0.113.7")`.
    #[test]
    fn test_forwarded_for_single_value_yields_that_address() {
        let headers = headers_with_forwarded_for(Some("203.0.113.7"));

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.ip_address.as_deref(), Some("203.0.113.7"));
    }

    /// Escenario: Una cadena de saltos.
    /// `X-Forwarded-For: 192.0.2.1, 203.0.113.7` → la dirección es el último
    /// elemento, nunca la cadena entera.
    #[test]
    fn test_forwarded_for_chain_yields_the_last_element() {
        let headers = headers_with_forwarded_for(Some("192.0.2.1, 203.0.113.7"));

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.ip_address.as_deref(), Some("203.0.113.7"));
        assert_ne!(
            request.ip_address.as_deref(),
            Some("192.0.2.1, 203.0.113.7"),
            "la cadena entera no es una dirección válida"
        );
    }

    /// Escenario: Una cadena con espacios alrededor de los elementos.
    /// Los espacios alrededor del elemento elegido se ignoran.
    #[test]
    fn test_forwarded_for_chain_with_spaces_is_trimmed() {
        let headers = headers_with_forwarded_for(Some("192.0.2.1 ,  203.0.113.7"));

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.ip_address.as_deref(), Some("203.0.113.7"));
    }

    /// Escenario: Una dirección IPv6.
    /// El valor IPv6 simple ya pasa hoy (guardián); el último elemento de una
    /// cadena IPv6 debe sobrevivir a la partición por comas y hoy falla.
    #[test]
    fn test_forwarded_for_ipv6_survives_the_split() {
        let single = headers_with_forwarded_for(Some("2001:db8::1"));
        let request = NewRequest::from_request(&single, None, None);
        assert_eq!(request.ip_address.as_deref(), Some("2001:db8::1"));

        let chain = headers_with_forwarded_for(Some("2001:db8::1, 2001:db8::2"));
        let request = NewRequest::from_request(&chain, None, None);
        assert_eq!(request.ip_address.as_deref(), Some("2001:db8::2"));
    }

    /// Escenario: Un valor que no es una dirección.
    /// `ip_address == None` y el diagnóstico nombra el valor descartado.
    #[test]
    fn test_forwarded_for_invalid_value_yields_none_and_diagnoses() {
        let headers = headers_with_forwarded_for(Some("no-soy-una-direccion"));

        let mut ip_address: Option<String> = None;
        let log = capture_debug(|| {
            ip_address = NewRequest::from_request(&headers, None, None).ip_address;
        });

        assert_eq!(ip_address, None);
        assert!(
            log.contains("no-soy-una-direccion"),
            "el diagnóstico debe nombrar el valor descartado; log capturado: {log:?}"
        );
    }

    /// Escenario: Una cadena cuya cola no es una dirección.
    /// `ip_address == None` y el diagnóstico nombra el valor descartado.
    #[test]
    fn test_forwarded_for_chain_with_invalid_tail_yields_none_and_diagnoses() {
        let headers = headers_with_forwarded_for(Some("192.0.2.1, no-soy-una-direccion"));

        let mut ip_address: Option<String> = None;
        let log = capture_debug(|| {
            ip_address = NewRequest::from_request(&headers, None, None).ip_address;
        });

        assert_eq!(ip_address, None);
        assert!(
            log.contains("no-soy-una-direccion"),
            "el diagnóstico debe nombrar el valor descartado; log capturado: {log:?}"
        );
    }

    /// Escenario: una cola vacía tras la última coma.
    /// `X-Forwarded-For: 192.0.2.1,` → el último elemento está vacío → `None`.
    #[test]
    fn test_forwarded_for_trailing_comma_yields_none() {
        let headers = headers_with_forwarded_for(Some("192.0.2.1,"));

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.ip_address, None);
    }

    /// Escenario: Cabecera ausente.
    /// `None` y sin diagnóstico. Guardián: hoy pasa.
    #[test]
    fn test_forwarded_for_absent_yields_none_without_diagnostic() {
        let headers = headers_with_forwarded_for(None);

        let mut ip_address: Option<String> = None;
        let log = capture_debug(|| {
            ip_address = NewRequest::from_request(&headers, None, None).ip_address;
        });

        assert_eq!(ip_address, None);
        assert!(
            log.is_empty(),
            "una cabecera ausente NO debe producir diagnóstico; log capturado: {log:?}"
        );
    }

    /// Escenario: Cabecera vacía.
    /// `None` y sin diagnóstico. Guardián: hoy pasa.
    #[test]
    fn test_forwarded_for_empty_yields_none_without_diagnostic() {
        let headers = headers_with_forwarded_for(Some(""));

        let mut ip_address: Option<String> = None;
        let log = capture_debug(|| {
            ip_address = NewRequest::from_request(&headers, None, None).ip_address;
        });

        assert_eq!(ip_address, None);
        assert!(
            log.is_empty(),
            "una cabecera vacía NO debe producir diagnóstico; log capturado: {log:?}"
        );
    }

    /// Escenario: valor con sólo espacios (borde del escenario de cabecera vacía).
    /// `X-Forwarded-For: "   "` → no hay dirección → `None`.
    #[test]
    fn test_forwarded_for_whitespace_only_yields_none() {
        let headers = headers_with_forwarded_for(Some("   "));

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(request.ip_address, None);
    }

    /// Escenario: La dirección coincide con la que usa el pipeline Jail.
    /// Invariante: si `ip_address` es `Some`, tiene que parsear como `IpAddr`.
    /// Un valor que no parsea hace que el WAF se salte la comprobación de
    /// baneos (`shuul.rs`: `request.ip_address...and_then(|s| s.parse().ok())`),
    /// así que los baneos que el Jail registró a partir del access log nunca se
    /// aplicarían. La dirección con la que el WAF comprueba tiene que coincidir
    /// con la que el Jail derivó.
    #[test]
    fn test_ip_address_when_present_is_always_a_valid_ip() {
        let cases: [(&str, Option<&str>); 7] = [
            ("203.0.113.7", Some("203.0.113.7")),
            ("192.0.2.1, 203.0.113.7", Some("203.0.113.7")),
            ("2001:db8::1", Some("2001:db8::1")),
            ("no-soy-una-direccion", None),
            ("192.0.2.1, no-soy-una-direccion", None),
            ("", None),
            ("   ", None),
        ];

        for (raw, expected) in cases {
            let headers = headers_with_forwarded_for(Some(raw));
            let request = NewRequest::from_request(&headers, None, None);
            assert_eq!(request.ip_address.as_deref(), expected, "entrada {raw:?}");
            if let Some(ip) = request.ip_address.as_deref() {
                assert!(
                    ip.parse::<std::net::IpAddr>().is_ok(),
                    "`ip_address` debe parsear como IpAddr para que el WAF \
                     compruebe baneos; el valor {ip:?} (de {raw:?}) no parsea y \
                     dejaría el baneo sin evaluar"
                );
            }
        }

        // Caso ausente: sin cabecera.
        let headers = headers_with_forwarded_for(None);
        let request = NewRequest::from_request(&headers, None, None);
        assert_eq!(request.ip_address, None, "cabecera ausente");
        if let Some(ip) = request.ip_address.as_deref() {
            assert!(
                ip.parse::<std::net::IpAddr>().is_ok(),
                "`ip_address` debe parsear como IpAddr; el valor {ip:?} no parsea"
            );
        }
    }

    // ── la dirección resuelta es la que consumen GeoIP y Tor ──

    /// Escenario: una cadena de saltos con `GeoIP` disponible.
    /// La consulta `GeoIP` se hace contra la dirección resuelta —el último
    /// elemento de la cadena—, así que `country_code` se rellena y el `GeoIP` no
    /// emite el diagnóstico de sintaxis que emite al recibir la cadena entera.
    #[test]
    fn test_chain_with_geoip_consults_the_resolved_address() {
        let geoip = require_geoip_service();
        let headers = headers_with_forwarded_for(Some("192.0.2.1, 8.8.8.8"));

        let mut ip_address: Option<String> = None;
        let mut country_code: Option<String> = None;
        let log = capture_debug(|| {
            let request = NewRequest::from_request(&headers, Some(&geoip), None);
            ip_address = request.ip_address;
            country_code = request.country_code;
        });

        assert_eq!(ip_address.as_deref(), Some("8.8.8.8"));
        assert!(
            country_code.is_some(),
            "con GeoIP disponible la consulta debe hacerse contra la dirección \
             resuelta (8.8.8.8 → US), no contra la cadena; log capturado: {log:?}"
        );
        assert!(
            !log.contains("invalid IP address syntax"),
            "el GeoIP no debe recibir la cadena cruda; log capturado: {log:?}"
        );
    }

    /// Escenario: una cadena de saltos con el filtro de Tor disponible.
    /// La consulta de Tor se hace contra la dirección resuelta, así que
    /// `is_tor` es `Some(true)` y no queda en `None` por culpa de la cadena.
    #[test]
    fn test_chain_with_tor_consults_the_resolved_address() {
        let tor = TorService::new();
        tor.load_from_str("185.220.101.1\n185.220.101.2\n");
        let headers = headers_with_forwarded_for(Some("192.0.2.1, 185.220.101.1"));

        let request = NewRequest::from_request(&headers, None, Some(&tor));

        assert_eq!(request.ip_address.as_deref(), Some("185.220.101.1"));
        assert_eq!(
            request.is_tor,
            Some(true),
            "el filtro de Tor debe consultarse con la dirección resuelta, no con \
             la cadena (que no parsea como IpAddr y deja `is_tor` en `None`)"
        );
    }

    /// Escenario: una dirección irresoluble no se consulta.
    /// Los cuatro campos derivados de los consumidores de la dirección —
    /// `country_code`, `country_name`, `city_name` e `is_tor`— quedan en `None`,
    /// como exige el spec `request-extraction` para el caso de dirección no
    /// resuelta; el único diagnóstico es el del resolvedor, que nombra el valor
    /// descartado, y ningún consumidor emite un diagnóstico de sintaxis.
    #[test]
    fn test_unresolvable_address_does_not_consult_geoip_nor_tor() {
        let geoip = require_geoip_service();
        let tor = TorService::new();
        tor.load_from_str("185.220.101.1\n");
        let headers = headers_with_forwarded_for(Some("no-soy-una-direccion"));

        let mut request: Option<NewRequest> = None;
        let log = capture_debug(|| {
            request = Some(NewRequest::from_request(&headers, Some(&geoip), Some(&tor)));
        });
        let request = request.expect("`from_request` debe haberse ejecutado dentro del closure");

        assert_eq!(request.ip_address, None);
        assert_eq!(request.is_tor, None);
        assert_eq!(
            request.country_code, None,
            "sin dirección resuelta el GeoIP no se consulta y `country_code` \
             queda en `None`"
        );
        assert_eq!(
            request.country_name, None,
            "sin dirección resuelta el GeoIP no se consulta y `country_name` \
             queda en `None`; el spec `request-extraction` exige los cuatro \
             campos derivados de los consumidores de la dirección en `None`"
        );
        assert_eq!(
            request.city_name, None,
            "sin dirección resuelta el GeoIP no se consulta y `city_name` queda \
             en `None`; el spec `request-extraction` exige los cuatro campos \
             derivados de los consumidores de la dirección en `None`"
        );
        assert!(
            log.contains("no-soy-una-direccion"),
            "el diagnóstico del resolvedor debe nombrar el valor descartado; \
             log capturado: {log:?}"
        );
        assert!(
            !log.contains("invalid IP address syntax"),
            "sin dirección válida no debe consultarse el GeoIP; log capturado: {log:?}"
        );
    }

    /// Escenario: un solo valor sigue resolviéndose igual (guardián: hoy pasa).
    /// Con una sola dirección `GeoIP` y Tor ya se consultaban bien; el arreglo no
    /// puede romper ese camino.
    #[test]
    fn test_single_value_still_consults_geoip_and_tor() {
        let geoip = require_geoip_service();
        let tor = TorService::new();
        tor.load_from_str("185.220.101.1\n");
        let headers = headers_with_forwarded_for(Some("8.8.8.8"));

        let request = NewRequest::from_request(&headers, Some(&geoip), Some(&tor));

        assert_eq!(request.ip_address.as_deref(), Some("8.8.8.8"));
        assert_eq!(request.is_tor, Some(false));
        assert!(
            request.country_code.is_some(),
            "con GeoIP disponible `country_code` debe rellenarse para 8.8.8.8; \
             obtenido: {:?}",
            request.country_code
        );
    }
}

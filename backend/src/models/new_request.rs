//! # Modelo de nueva petición HTTP
//!
//! Define [`NewRequest`], la estructura utilizada para el matcheo
//! de reglas (WAF y Jail). No representa almacenamiento en BD.

use chrono::{DateTime, Utc};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Cookies de la petición, con el nombre como clave. Vacío en el pipeline
    /// Jail, cuyo payload no las trae.
    #[serde(default)]
    pub cookies: BTreeMap<String, String>,
    /// Cabeceras de la petición con el nombre en minúsculas como clave. Vacío en
    /// el pipeline Jail, cuyo payload no trae el mapa completo de cabeceras.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Argumentos del query string, uno a uno y con el valor **crudo** (sin
    /// decodificar), igual que el campo `query`.
    #[serde(default)]
    pub query_args: BTreeMap<String, String>,
    /// Último segmento no vacío del path, si existe.
    pub basename: Option<String>,
    /// Subcadena tras el último punto del basename, si ese punto no es el primer
    /// carácter.
    pub extension: Option<String>,
    pub is_tor: Option<bool>,
    pub rule_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl NewRequest {
    /// Deriva las colecciones dependientes del path y el query.
    ///
    /// Recalcula `query_args`, `basename` y `extension` a partir de `self.query`
    /// y `self.path`. Es público para que el pipeline Jail pueda invocarlo sobre
    /// el `NewRequest` que construye desde su payload, sin duplicar la lógica de
    /// extracción que usa [`NewRequest::from_request`].
    pub fn derive_path_collections(&mut self) {
        // Argumentos del query string, crudos (sin decodificar).
        self.query_args = self
            .query
            .as_deref()
            .map(|query| {
                let mut args = BTreeMap::new();
                for part in query.split('&') {
                    if let Some((name, value)) = part.split_once('=') {
                        let name = name.trim();
                        let value = value.trim();
                        if !name.is_empty() && !value.is_empty() {
                            args.insert(name.to_string(), value.to_string());
                        }
                    }
                }
                args
            })
            .unwrap_or_default();

        // Último segmento no vacío del path.
        self.basename = self
            .path
            .as_deref()
            .and_then(|path| path.rsplit('/').find(|s| !s.is_empty()))
            .map(str::to_string);

        // Subcadena tras el último punto del basename, siempre que ese punto no
        // sea el primer carácter y la subcadena no esté vacía.
        self.extension = self.basename.as_deref().and_then(|basename| {
            basename
                .rfind('.')
                .filter(|&index| index > 0)
                .map(|index| &basename[index + 1..])
                .filter(|ext| !ext.is_empty())
                .map(str::to_string)
        });
    }

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

        // Extracción de las colecciones nuevas. Las cookies salen del encabezado
        // `cookie` (repetido si lo hay); las cabeceras se guardan con el nombre en
        // minúsculas y el último valor gana; el resto de colecciones derivadas del
        // path/query se calculan en `derive_path_collections`, para no duplicar la
        // lógica con el Jail.
        let mut cookies = BTreeMap::new();
        for value in headers.get_all("cookie") {
            if let Ok(raw) = value.to_str() {
                for part in raw.split(';') {
                    if let Some((name, value)) = part.trim().split_once('=') {
                        let name = name.trim();
                        let value = value.trim();
                        if !name.is_empty() && !value.is_empty() {
                            cookies.insert(name.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        let mut headers_map = BTreeMap::new();
        for (name, value) in headers {
            if let Ok(value) = value.to_str()
                && !value.is_empty()
            {
                headers_map.insert(name.as_str().to_lowercase(), value.to_string());
            }
        }

        let mut request = Self {
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
            cookies,
            headers: headers_map,
            query_args: BTreeMap::new(),
            basename: None,
            extension: None,
            is_tor,
            rule_id: None,
            created_at: Utc::now(),
        };
        request.derive_path_collections();
        request
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

    // ── waf-transformations 1.13: ¿`path` y `query` llegan codificados? ──

    /// MEDICIÓN (tarea 1.13): mide de dónde salen `path` y `query` y si llegan
    /// codificados o decodificados.
    ///
    /// Con `x-forwarded-uri: /a/%2e%2e/b?q=%2e%2e` el valor observado es
    /// `path = "/a/%2e%2e/b"` y `query = "q=%2e%2e"`: los dos llegan
    /// **codificados**, tal cual los escribió el cliente. `Uri::path()` y
    /// `Uri::query()` devuelven los componentes **crudos** del *request-target*:
    /// `http::Uri` no decodifica los escapes porcentuales ni normaliza los
    /// segmentos `..`.
    ///
    /// Implicación para la plantilla de path traversal: como la petición llega
    /// codificada, la plantilla **necesita** `urlDecodeUni` sobre `path` —si no,
    /// un patrón `\.\./` nunca vería el `../`— y sobre `query` **si además filtra
    /// por `query`**, porque ahí los escapes tampoco vienen resueltos. Un
    /// `normalizePath` suelto tampoco bastaría: sin decodificar antes no hay
    /// ningún segmento `..` que normalizar.
    #[test]
    fn test_forwarded_uri_reaches_path_and_query_percent_encoded() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-uri", "/a/%2e%2e/b?q=%2e%2e".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        // MEDICIÓN (tarea 1.13): valores observados, no supuestos.
        assert_eq!(
            request.path.as_deref(),
            Some("/a/%2e%2e/b"),
            "el `path` llega codificado: `%2e%2e` no se decodifica a `..`"
        );
        assert_eq!(
            request.query.as_deref(),
            Some("q=%2e%2e"),
            "la `query` llega codificada: `%2e%2e` no se decodifica a `..`"
        );
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

// ── F2b-2a: extracción de las colecciones nuevas ──
//
// Cubre la extracción real de cookies, cabeceras, argumentos del query,
// basename y extensión que implementa `from_request` y `derive_path_collections`.
#[cfg(test)]
mod collection_extraction_tests {
    use super::*;

    /// Escenario: Las cookies se extraen del encabezado `Cookie`.
    #[test]
    fn test_cookies_are_extracted_from_cookie_header() {
        let mut headers = http::HeaderMap::new();
        headers.insert("cookie", "a=1; b=2".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.cookies.get("a").map(String::as_str),
            Some("1"),
            "la cookie `a` debe valer `1`"
        );
        assert_eq!(
            request.cookies.get("b").map(String::as_str),
            Some("2"),
            "la cookie `b` debe valer `2`"
        );
    }

    /// Escenario: Las cabeceras se extraen con el nombre en minúsculas.
    #[test]
    fn test_headers_are_extracted_with_lowercase_name() {
        let mut headers = http::HeaderMap::new();
        headers.insert("authorization", "Bearer x".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer x"),
            "la cabecera `authorization` debe estar en minúsculas"
        );
    }

    /// Escenario: Los argumentos del query se extraen uno a uno y crudos.
    #[test]
    fn test_query_args_are_extracted_raw() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-uri", "/?id=1%20union&x=2".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.query_args.get("id").map(String::as_str),
            Some("1%20union"),
            "el valor de `id` debe conservar el `%20` sin decodificar"
        );
        assert_eq!(
            request.query_args.get("x").map(String::as_str),
            Some("2"),
            "el argumento `x` debe valer `2`"
        );
    }

    /// Escenario: El basename y la extensión se derivan del path.
    #[test]
    fn test_basename_and_extension_are_derived_from_path() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-uri", "/a/b/index.php".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.basename.as_deref(),
            Some("index.php"),
            "el basename debe ser el último segmento no vacío"
        );
        assert_eq!(
            request.extension.as_deref(),
            Some("php"),
            "la extensión debe ser la subcadena tras el último punto"
        );
    }

    /// Escenario: No hay basename ni extensión sin último segmento.
    #[test]
    fn test_no_basename_or_extension_without_last_segment() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-uri", "/".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert!(request.basename.is_none(), "el path `/` no tiene basename");
        assert!(
            request.extension.is_none(),
            "el path `/` no tiene extensión"
        );
    }

    /// Escenario: La extracción del Jail conserva lo derivable de su payload.
    ///
    /// El payload del Jail trae `path` y `query`; se derivan argumentos del
    /// query, basename y extensión. Las cookies y las cabeceras quedan ausentes.
    #[test]
    fn test_jail_extraction_derives_what_it_can_from_payload() {
        // Simula la construcción del `NewRequest` en el pipeline Jail.
        let mut request = NewRequest::from_request(&http::HeaderMap::new(), None, None);
        request.path = Some("/a/b/index.php".to_string());
        request.query = Some("id=1%20union&x=2".to_string());
        // La derivación es explícita: el Jail la invoca tras construir el literal.
        request.derive_path_collections();
        assert_eq!(
            request.query_args.get("id").map(String::as_str),
            Some("1%20union"),
            "el Jail debe derivar los argumentos del query de su payload"
        );
        assert_eq!(
            request.basename.as_deref(),
            Some("index.php"),
            "el Jail debe derivar el basename de su payload"
        );
        assert_eq!(
            request.extension.as_deref(),
            Some("php"),
            "el Jail debe derivar la extensión de su payload"
        );
        assert!(
            request.cookies.is_empty(),
            "el Jail no trae cookies y deben quedar ausentes"
        );
        assert!(
            request.headers.is_empty(),
            "el Jail no trae el mapa completo de cabeceras"
        );
    }

    /// Semántica fijada: con argumentos duplicados en el query, el último valor
    /// gana.
    ///
    /// // NOTA: en un WAF, la ambigüedad de parámetros duplicados (HTTP
    /// parameter pollution) puede permitir evadir reglas si el backend elige un
    /// valor distinto del que inspecciona el WAF. Hoy se fija el comportamiento
    /// actual —el último gana—; revisar en otra sub-entrega.
    #[test]
    fn test_query_args_duplicate_last_wins() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-uri", "/?id=1&id=2".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.query_args.get("id").map(String::as_str),
            Some("2"),
            "con argumentos duplicados en el query, el último valor debe ganar"
        );
    }

    /// Semántica fijada: dos cabeceras `cookie` mezclan sus cookies y el último
    /// valor de una misma clave gana.
    ///
    /// // NOTA: en un WAF, la ambigüedad de cabeceras repetidas puede permitir
    /// evadir reglas si el backend elige un valor distinto del que inspecciona el
    /// WAF. Hoy se fija el comportamiento actual —mezcla y último valor gana—;
    /// revisar en otra sub-entrega.
    #[test]
    fn test_duplicate_cookie_headers_mix_and_last_wins() {
        let mut headers = http::HeaderMap::new();
        headers.append("cookie", "a=1; b=2".parse().unwrap());
        headers.append("cookie", "c=3; a=4".parse().unwrap());

        let request = NewRequest::from_request(&headers, None, None);

        assert_eq!(
            request.cookies.get("a").map(String::as_str),
            Some("4"),
            "el último valor de una misma cookie debe ganar"
        );
        assert_eq!(
            request.cookies.get("b").map(String::as_str),
            Some("2"),
            "las cookies de la primera cabecera deben conservarse"
        );
        assert_eq!(
            request.cookies.get("c").map(String::as_str),
            Some("3"),
            "las cookies de la segunda cabecera deben añadirse"
        );
    }
}

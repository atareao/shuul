//! # Tor Exit Node Detection Service
//!
//! [`TorService`] descarga y cachea la lista de Tor exit nodes
//! desde `https://check.torproject.org/torbulkexitlist`.
//!
//! La inicialización es lazy: el conjunto arranca vacío y se puebla
//! en el primer `refresh()` exitoso.
//!
//! En caso de fallo de red, el conjunto existente se preserva
//! (degradación graceful con datos stale).

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::RwLock;
use std::time::Duration;
use tracing::{error, info};

/// URL de la lista bulk de Tor exit nodes.
#[allow(dead_code)]
const TOR_EXIT_LIST_URL: &str = "https://check.torproject.org/torbulkexitlist";
/// Timeout para las peticiones HTTP.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// Servicio de detección de Tor exit nodes.
///
/// Mantiene un conjunto de IPs de Tor exit nodes, refrescado
/// periódicamente desde la lista bulk del Tor Project.
///
/// # Lazy initialization
///
/// El servicio arranca con `None` (no inicializado). La primera
/// llamada a `refresh()` que tenga éxito puebla el conjunto.
/// Mientras no se haya inicializado, `is_exit_node()` devuelve
/// `None` (desconocido).
///
/// # Panics
///
/// - `new()` panics si `reqwest::Client::builder().build()` falla
///   (solo ocurre si la configuración TLS es inválida).
/// - `refresh()`, `load_from_str()`, `is_exit_node()` pueden panic
///   si el lock de `RwLock` está envenenado (otro thread panicó
///   mientras mantenía el lock). Esto es aceptable porque un lock
///   envenenado indica un estado inconsistente irrecuperable.
#[allow(dead_code)]
pub struct TorService {
    exit_nodes: RwLock<Option<HashSet<IpAddr>>>,
    client: reqwest::Client,
}

impl TorService {
    /// Crea un nuevo `TorService` con conjunto vacío (lazy init).
    ///
    /// # Panics
    ///
    /// Panics if the reqwest client cannot be built (e.g., TLS
    /// configuration is invalid).
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("Failed to create reqwest client — TLS config may be invalid");
        Self {
            exit_nodes: RwLock::new(None),
            client,
        }
    }

    /// Descarga la lista de Tor exit nodes desde la URL por defecto
    /// y reemplaza el conjunto en memoria.
    ///
    /// Si la petición HTTP falla, el conjunto existente se preserva
    /// (degradación graceful con datos stale).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[allow(dead_code)]
    pub async fn refresh(&self) {
        self.refresh_from_url(TOR_EXIT_LIST_URL).await;
    }

    /// Descarga la lista de Tor exit nodes desde una URL específica.
    ///
    /// Expuesta como `pub` para testing con servidores mock.
    /// Si la petición HTTP falla o devuelve un status no-2xx,
    /// el conjunto existente se preserva (degradación graceful).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[allow(dead_code)]
    pub async fn refresh_from_url(&self, url: &str) {
        let response = self.client.get(url).send().await;
        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    error!(
                        "Tor exit list returned HTTP {}: {}. Preserving stale data.",
                        status, body
                    );
                    return;
                }
                match resp.text().await {
                    Ok(body) => {
                        let new_set = parse_exit_list(&body);
                        info!("Loaded {} Tor exit nodes", new_set.len());
                        // Safety: lock is held briefly, no concurrent poisoning expected
                        *self.exit_nodes.write().expect("RwLock poisoned") = Some(new_set);
                    },
                    Err(e) => {
                        error!("Failed to read Tor exit list response body: {e}");
                    },
                }
            },
            Err(e) => {
                error!("Failed to fetch Tor exit list: {e}");
            },
        }
    }

    /// Carga exit nodes desde un string (una IP por línea).
    ///
    /// Útil para testing y para bootstrapping desde una fuente alternativa.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[allow(dead_code)]
    pub fn load_from_str(&self, data: &str) {
        let new_set = parse_exit_list(data);
        // Safety: lock is held briefly, no concurrent poisoning expected
        *self.exit_nodes.write().expect("RwLock poisoned") = Some(new_set);
    }

    /// Comprueba si una IP es un Tor exit node.
    ///
    /// Devuelve `None` si el conjunto aún no se ha cargado (lazy).
    /// Devuelve `Some(true)` si la IP está en el conjunto.
    /// Devuelve `Some(false)` si la IP no está en el conjunto.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn is_exit_node(&self, ip: &str) -> Option<bool> {
        // Safety: read lock is held briefly, no concurrent poisoning expected
        let guard = self.exit_nodes.read().expect("RwLock poisoned");
        match guard.as_ref() {
            None => None,
            Some(set) => {
                let ip = ip.parse::<IpAddr>().ok()?;
                Some(set.contains(&ip))
            },
        }
    }
}

impl Default for TorService {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsea la lista bulk de Tor exit nodes (una IP por línea).
///
/// Ignora líneas vacías y líneas con IPs inválidas.
#[allow(dead_code)]
fn parse_exit_list(data: &str) -> HashSet<IpAddr> {
    let mut set = HashSet::new();
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line.parse::<IpAddr>() {
            Ok(ip) => {
                set.insert(ip);
            },
            Err(e) => {
                error!("Failed to parse Tor exit node IP '{line}': {e}");
            },
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Escenario: TorService se inicializa con conjunto vacío.
    /// `is_exit_node()` devuelve `None` antes del primer refresh.
    #[test]
    fn test_is_exit_node_returns_none_before_refresh() {
        let service = TorService::new();
        assert_eq!(service.is_exit_node("185.220.101.1"), None);
        assert_eq!(service.is_exit_node("8.8.8.8"), None);
    }

    /// Escenario: Tras cargar datos, `is_exit_node()` devuelve `Some(true)`
    /// para una IP Tor conocida.
    #[test]
    fn test_is_exit_node_returns_some_true_for_known_tor_ip() {
        let service = TorService::new();
        service.load_from_str("185.220.101.1\n185.220.101.2\n");
        assert_eq!(service.is_exit_node("185.220.101.1"), Some(true));
    }

    /// Escenario: Tras cargar datos, `is_exit_node()` devuelve `Some(false)`
    /// para una IP no Tor.
    #[test]
    fn test_is_exit_node_returns_some_false_for_non_tor_ip() {
        let service = TorService::new();
        service.load_from_str("185.220.101.1\n185.220.101.2\n");
        assert_eq!(service.is_exit_node("8.8.8.8"), Some(false));
    }

    /// Escenario: Un refresh que falla preserva el conjunto stale.
    ///
    /// 1. Cargamos datos mock via `load_from_str()`
    /// 2. Verificamos que los datos están presentes
    /// 3. Llamamos a `refresh()` (fallará en entorno de test)
    /// 4. Verificamos que los datos stale se conservan
    #[tokio::test]
    async fn test_refresh_failure_keeps_stale_data() {
        let service = TorService::new();
        service.load_from_str("185.220.101.1\n185.220.101.2\n");

        // Verificar datos cargados
        assert_eq!(service.is_exit_node("185.220.101.1"), Some(true));
        assert_eq!(service.is_exit_node("8.8.8.8"), Some(false));

        // Llamar a refresh() — fallará por red (no hay conexión a Tor)
        service.refresh().await;

        // Los datos stale deben conservarse
        assert_eq!(
            service.is_exit_node("185.220.101.1"),
            Some(true),
            "Stale data should persist after failed refresh"
        );
        assert_eq!(
            service.is_exit_node("8.8.8.8"),
            Some(false),
            "Stale data should persist after failed refresh"
        );
    }

    /// Escenario: parse_exit_list ignora líneas inválidas.
    #[test]
    fn test_parse_exit_list_skips_invalid_lines() {
        let data = "185.220.101.1\ninvalid\n\n185.220.101.2\n";
        let set = parse_exit_list(data);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&"185.220.101.1".parse::<IpAddr>().unwrap()));
        assert!(set.contains(&"185.220.101.2".parse::<IpAddr>().unwrap()));
    }

    /// Escenario: TorService implementa Default.
    #[test]
    fn test_tor_service_default() {
        let service = TorService::default();
        assert_eq!(service.is_exit_node("185.220.101.1"), None);
    }

    /// Escenario: Refresh con respuesta HTTP no-2xx preserva datos stale.
    ///
    /// 1. Cargamos datos mock via `load_from_str()`
    /// 2. Verificamos que los datos están presentes
    /// 3. Llamamos a `refresh_from_url()` apuntando a un mock que devuelve 429
    /// 4. Verificamos que los datos stale se conservan
    #[tokio::test]
    async fn test_refresh_non_2xx_preserves_stale_data() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/torbulkexitlist"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Too Many Requests"))
            .mount(&mock_server)
            .await;

        let service = TorService::new();
        service.load_from_str("185.220.101.1\n185.220.101.2\n");

        // Verificar datos cargados
        assert_eq!(service.is_exit_node("185.220.101.1"), Some(true));
        assert_eq!(service.is_exit_node("8.8.8.8"), Some(false));

        // Llamar a refresh_from_url con la URL del mock (devuelve 429)
        let url = format!("{}/torbulkexitlist", mock_server.uri());
        service.refresh_from_url(&url).await;

        // Los datos stale deben conservarse
        assert_eq!(
            service.is_exit_node("185.220.101.1"),
            Some(true),
            "Stale data should persist after non-2xx HTTP response"
        );
        assert_eq!(
            service.is_exit_node("8.8.8.8"),
            Some(false),
            "Stale data should persist after non-2xx HTTP response"
        );
    }
}

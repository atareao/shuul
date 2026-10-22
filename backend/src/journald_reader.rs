//! # Journald Reader
//!
//! Lee en streaming los access logs de Traefik, los parsea a `ReportPayload` y
//! los inyecta en el pipeline Jail. Un mismo cliente HTTP/1.1 (`hyper`, genérico
//! sobre `AsyncRead + AsyncWrite`) sirve **dos backends** que sólo se diferencian
//! en la **petición** y en cómo se trocea el cuerpo; el transporte, el bucle de
//! streaming y la reconexión son comunes.
//!
//! ## Backends y transportes
//!
//! Cada backend se sirve por socket Unix (`AF_UNIX`) o por TCP, y el backend y el
//! transporte se eligen por variable de entorno; no hay dos implementaciones de la
//! petición ni del parseo.
//!
//! | Backend | Socket Unix | TCP |
//! |---|---|---|
//! | **API de Docker** | `SHUUL_DOCKER_SOCKET` | `SHUUL_DOCKER_URL` (proxy) |
//! | **`systemd-journal-gatewayd`** | `SHUUL_JOURNALD_SOCKET` | `SHUUL_JOURNALD_URL` |
//!
//! La precedencia es: `SHUUL_DOCKER_SOCKET` → `SHUUL_DOCKER_URL` →
//! `SHUUL_JOURNALD_SOCKET` → `SHUUL_JOURNALD_URL` → `http://localhost:19531`. Un
//! valor ausente o en blanco cuenta como no definido. Dentro de un backend el
//! socket gana a su URL, se avisa de la URL ignorada y se sigue; y el backend de
//! más prioridad gana **sin** que las variables del otro lleguen a evaluarse como
//! transporte. El contenedor sigue `SHUUL_DOCKER_CONTAINER` →
//! `SHUUL_JOURNALD_CONTAINER` → `traefik`, de modo que quien ya apuntaba al
//! contenedor correcto con la variable de journald no cambia al pasar de backend.
//!
//! El socket de `/run/systemd/journal/socket` NO se usa: es write-only. Lo que
//! sirve gatewayd es su API HTTP.
//!
//! ## Petición
//!
//! - **API de Docker**:
//!   `GET /v1.44/containers/{container}/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={cutoff}`
//!   en **forma origen**, con `Accept: application/json`, `Host: docker` por socket
//!   (la autoridad del endpoint por TCP) y **sin** `Upgrade` ni
//!   `Connection: Upgrade` —con ellas el motor contesta `101 UPGRADED` en vez de
//!   servir el stream—. `timestamps=1` antepone a cada carga su instante
//!   `RFC3339Nano`, que alimenta el corte por intento.
//! - **gatewayd**: `GET /entries?follow&CONTAINER_NAME={container}`, también en
//!   forma origen, más un `Range` de reanudación que es sólo una **pista**.
//!
//! El cuerpo de la API de Docker va en **frames** multiplexados (8 bytes de
//! cabecera: byte de stream, 3 de relleno y longitud big-endian) cuando el
//! contenedor **no** lleva TTY. `1` (stdout) va al pipeline, `2` (stderr) se
//! descarta, y un primer byte fuera de `{0,1,2}` —un JSON crudo empieza por
//! `{`— descarta la conexión con un warning que nombra la causa probable. Con
//! `tty: true` el stream va crudo y sin cabeceras: es el error más común.
//!
//! El camino de Docker va **directo** a [`parse_traefik_access_log`]: **no** pasa
//! por `extract_report_payload`/`is_traefik_entry`, que exigen el campo
//! `CONTAINER_NAME` —que el stream no trae— y descartarían el 100 % de las líneas
//! en silencio.
//!
//! ## Corte por intento de conexión (no-replay)
//!
//! Garantía **común a los dos backends**: cada intento fija su corte en el
//! instante en que empieza (µs) y sólo entrega al pipeline lo **estrictamente
//! posterior**. Lo escrito mientras el reader estaba caído —o entre dos
//! intentos— no cuenta para el rate limiter, así que reproducir el histórico al
//! arrancar no puede banear IPs por escaneos antiguos. Es un fail-open deliberado;
//! el WAF sigue protegiendo porque es síncrono y no depende de este transporte.
//!
//! ## Frontera de descarte, por backend (asimetría deliberada)
//!
//! Cada backend descarta con la evidencia de la que dispone, y las dos fronteras
//! son distintas a propósito, no por descuido:
//!
//! - **API de Docker**: comparación exacta del instante de la carga con el corte
//!   del intento en curso (`payload_is_after_cutoff`), estrictamente posterior.
//!   El `since` de la petición es el filtro grueso del servidor —granularidad de
//!   segundo— y una mera pista; la corrección la da la comparación exacta.
//! - **gatewayd**: descarte por `__REALTIME_TIMESTAMP` anterior al corte y, tras
//!   reconectar, dedupe por `__CURSOR` —el stream puede reanudarse en la entrada
//!   que el cursor señala—. El `Range` es sólo una pista: gatewayd ignora
//!   `realtime` con `follow` y no aplica el `skip` de `entries=<cursor>:1` sin
//!   `num_entries`.
//!
//! ## Limitación de reloj
//!
//! La comparación con el corte asume que el emisor y Shuul comparten reloj. Con el
//! daemon en la misma máquina se cumple (es el caso de los dos modos de Docker);
//! apuntar `SHUUL_DOCKER_URL` a un daemon de **otra** máquina queda **fuera del
//! contrato**. Y falla en las dos direcciones: un emisor **atrasado** descarta
//! tráfico legítimo en silencio, y uno **adelantado** deja pasar el backlog
//! (replay proporcional al desfase). Es la misma limitación que ya tenía journald
//! por TCP, ahora aplicable también a la API de Docker remota.
//!
//! ## Fail-open
//!
//! Si el backend no está disponible se emite un `warn!` que **nombra el backend y
//! el endpoint** —`docker api not available: {endpoint}` o `journald gateway not
//! available`— y se reintenta cada [`DEFAULT_RETRY_DELAY`]. El texto de gatewayd
//! se conserva literal: es el guardián de que el camino heredado no cambió.
//!
//! ## Concurrencia
//!
//! El reader corre como background task en main.rs. No comparte estado mutable
//! más allá del `AppState` que recibe para inyectar payloads en el Jail pipeline.

use crate::http::report::run_jail_pipeline;
use crate::models::{AppState, ReportPayload};
use http::{Method, Request, StatusCode, header};
use http_body_util::{BodyExt, Empty};
use hyper::body::{Bytes, Incoming};
use hyper::client::conn::http1;
use hyper::{Response, Uri};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UnixStream};
use tracing::{debug, info, warn};

/// Errors that can occur in the journald reader.
///
/// El primer campo es el contexto ya formateado que puebla `Display`; el
/// segundo lleva la causa real como `#[source]` cuando existe, de modo que la
/// cadena se puede recorrer con `std::error::Error::source()` y la causa se
/// puede inspeccionar por tipo (`io::Error::kind()`,
/// `serde_json::Error::line()`, `hyper::Error::is_incomplete_message()`, …).
#[derive(Debug, Error)]
pub enum JournalError {
    /// Error de E/S al conectar o leer del gateway
    #[error("Socket error: {0}")]
    Socket(
        String,
        #[source] Option<Box<dyn std::error::Error + Send + Sync>>,
    ),

    /// Error al parsear el formato nativo del journal
    #[error("Journal entry parse error: {0}")]
    Parse(
        String,
        #[source] Option<Box<dyn std::error::Error + Send + Sync>>,
    ),

    /// Error al parsear el JSON del access log de Traefik
    #[error("Traefik access log parse error: {0}")]
    TraefikParse(
        String,
        #[source] Option<Box<dyn std::error::Error + Send + Sync>>,
    ),

    /// Error de conexión (gateway no disponible)
    #[error("Connection error: {0}")]
    Connection(
        String,
        #[source] Option<Box<dyn std::error::Error + Send + Sync>>,
    ),

    /// El stream de la API de Docker no lleva cabeceras de frame: el
    /// contenedor tiene TTY.
    ///
    /// La API sólo multiplexa el stream de un contenedor **sin** TTY; con él, el
    /// cuerpo llega crudo y el primer byte que el decodificador lee como
    /// identificador de stream es en realidad el primero del mensaje (un JSON
    /// empieza por `{`, `0x7B`). El campo es ese byte, para que la diagnosis lo
    /// nombre.
    #[error(
        "raw Docker log stream: byte 0x{0:02X} is not a stream id (0, 1 or 2); the container \
         probably has TTY enabled, which sends the log stream raw, without the 8-byte frame headers"
    )]
    TtyStream(u8),

    /// La longitud declarada en la cabecera de un frame de la API de Docker
    /// excede la cota superior aceptada ([`MAX_FRAME_LEN`]).
    ///
    /// La cabecera lleva la longitud de la carga como un `u32`, así que el par
    /// —el daemon o, en el modo B, un proxy de terceros— puede declarar hasta
    /// ~4 GiB y hacer crecer el buffer del decodificador hasta esa cifra con
    /// bytes reales: es un vector de agotamiento de memoria. La cota es
    /// holgadísima para una línea de access log de Traefik (~1 KiB). Descartar
    /// la conexión es el mismo remedio que [`JournalError::TtyStream`]: el
    /// bucle reconecta y el corte del intento nuevo impide cualquier replay.
    #[error(
        "Docker log stream frame declares {declared} bytes of payload, over the {limit}-byte \
         limit; discarding the connection"
    )]
    FrameTooLarge {
        /// Longitud de la carga que declaraba la cabecera, en bytes.
        declared: u32,
        /// Cota superior aceptada, en bytes ([`MAX_FRAME_LEN`]).
        limit: usize,
    },
}

impl JournalError {
    /// `Socket` sin causa subyacente (fallo de contexto puro).
    ///
    /// Todo sitio de producción de `Socket` tiene hoy una causa, así que este
    /// constructor sólo lo usan los tests; existe para que la pareja de cada
    /// variante quede completa (spec `journald-reader`).
    #[allow(dead_code)]
    pub fn socket(ctx: impl Into<String>) -> Self {
        Self::Socket(ctx.into(), None)
    }

    /// `Socket` con la causa subyacente adjunta como `#[source]`.
    pub fn socket_from(
        ctx: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Socket(ctx.into(), Some(Box::new(e)))
    }

    /// `Parse` sin causa subyacente (fallo de contexto puro).
    pub fn parse(ctx: impl Into<String>) -> Self {
        Self::Parse(ctx.into(), None)
    }

    /// `Parse` con la causa subyacente adjunta como `#[source]`.
    pub fn parse_from(
        ctx: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Parse(ctx.into(), Some(Box::new(e)))
    }

    /// `TraefikParse` sin causa subyacente (fallo de contexto puro).
    ///
    /// Todo sitio de producción de `TraefikParse` tiene hoy una causa, así que
    /// este constructor sólo lo usan los tests; existe para que la pareja de
    /// cada variante quede completa (spec `journald-reader`).
    #[allow(dead_code)]
    pub fn traefik_parse(ctx: impl Into<String>) -> Self {
        Self::TraefikParse(ctx.into(), None)
    }

    /// `TraefikParse` con la causa subyacente adjunta como `#[source]`.
    pub fn traefik_parse_from(
        ctx: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::TraefikParse(ctx.into(), Some(Box::new(e)))
    }

    /// `Connection` sin causa subyacente (fallo de contexto puro).
    pub fn connection(ctx: impl Into<String>) -> Self {
        Self::Connection(ctx.into(), None)
    }

    /// `Connection` con la causa subyacente adjunta como `#[source]`.
    pub fn connection_from(
        ctx: impl Into<String>,
        e: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Connection(ctx.into(), Some(Box::new(e)))
    }
}

/// Cualquier flujo bidireccional de bytes sirve para `hyper`: `TcpStream` y
/// `UnixStream` implementan ambos estos traits.
trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// Stream borrado de tipo: el transporte se decide en tiempo de ejecución.
type BoxedIo = Box<dyn Io>;

/// Tarea que hace avanzar la conexión HTTP/1.1 de `hyper`.
///
/// Vive mientras dure `stream_once`: al soltarse aborta la `Connection`, de
/// modo que ni un timeout ni un retorno temprano dejan el socket abierto.
struct ConnectionTask(Option<tokio::task::JoinHandle<()>>);

impl ConnectionTask {
    fn spawn<F>(connection: F) -> Self
    where
        F: std::future::Future<Output = Result<(), hyper::Error>> + Send + 'static,
    {
        Self(Some(tokio::spawn(async move {
            if let Err(e) = connection.await {
                debug!("journald reader: gateway connection finished: {e}");
            }
        })))
    }
}

impl Drop for ConnectionTask {
    /// Aborta la conexión al salir de `stream_once`.
    ///
    /// Cada conexión sirve una sola petición, así que no se reutiliza: abortar
    /// aquí cierra el socket también en los retornos tempranos (status de
    /// error, shutdown a mitad de stream). Si sólo se soltara `Incoming`,
    /// `hyper` mantendría viva la conexión contra un gateway que no cierra y
    /// se acumularía un descriptor por reintento.
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

/// Transporte por el que se alcanza `systemd-journal-gatewayd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournaldTransport {
    /// HTTP sobre TCP contra una URL base (`http://host:puerto`).
    Tcp {
        /// URL base del endpoint HTTP de gatewayd, sin path de la petición.
        base_url: String,
    },
    /// HTTP sobre un socket `AF_UNIX` del sistema de ficheros.
    UnixSocket {
        /// Path del socket que expone gatewayd.
        path: PathBuf,
    },
    // Variantes aditivas del backend de la **API de Docker** (decisión D2 del
    // `design.md`): las construye `from_values` al resolver la precedencia.
    /// API de logs de Docker sobre TCP contra una URL base (proxy).
    DockerTcp {
        /// URL base del endpoint HTTP de la API de Docker.
        base_url: String,
    },
    /// API de logs del motor de Docker sobre su socket `AF_UNIX`.
    DockerUnixSocket {
        /// Path del socket del motor de Docker.
        path: PathBuf,
    },
}

impl JournaldTransport {
    /// `true` si el transporte habla con la **API de Docker** en vez de con
    /// `systemd-journal-gatewayd`.
    ///
    /// Es lo único que decide el texto del aviso de backend no disponible: los
    /// dos backends comparten bucle, petición y conexión, así que el log es el
    /// único punto donde hay que distinguirlos, y se distingue con un predicado
    /// para no abrir una segunda bifurcación del camino de streaming.
    const fn is_docker(&self) -> bool {
        matches!(self, Self::DockerTcp { .. } | Self::DockerUnixSocket { .. })
    }
}

/// Lector de access logs desde `systemd-journal-gatewayd`.
///
/// Hace streaming NDJSON contra el endpoint `/entries?follow` del gateway,
/// filtra las entradas por contenedor y las convierte en `ReportPayload`
/// para el pipeline Jail.
pub struct JournaldReader {
    /// Transporte hacia gatewayd (TCP o socket Unix).
    pub transport: JournaldTransport,
    /// Nombre del contenedor Docker a filtrar (default: "traefik")
    pub container_name: String,
    /// Espera entre reintentos cuando el gateway no está disponible
    /// (default: [`DEFAULT_RETRY_DELAY`])
    pub retry_delay: Duration,
}

impl JournaldReader {
    /// Crea un reader que habla HTTP sobre TCP contra `base_url`.
    pub fn new(base_url: &str, container_name: &str) -> Self {
        Self {
            transport: JournaldTransport::Tcp {
                base_url: base_url.to_string(),
            },
            container_name: container_name.to_string(),
            retry_delay: DEFAULT_RETRY_DELAY,
        }
    }

    /// Crea un reader que habla HTTP sobre el socket Unix `path`.
    pub fn unix(path: impl Into<PathBuf>, container_name: &str) -> Self {
        Self {
            transport: JournaldTransport::UnixSocket { path: path.into() },
            container_name: container_name.to_string(),
            retry_delay: DEFAULT_RETRY_DELAY,
        }
    }

    /// Sobrescribe la espera entre reintentos (útil fuera de producción).
    ///
    /// Sólo lo usan los tests y los embedders; en producción se mantiene el
    /// default de [`DEFAULT_RETRY_DELAY`].
    #[allow(dead_code)]
    #[must_use]
    pub const fn with_retry_delay(mut self, retry_delay: Duration) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    /// Construye un reader a partir de valores ya resueltos, aplicando esta
    /// precedencia: socket de Docker → URL de Docker → socket de journald →
    /// URL de journald → default de gatewayd.
    ///
    /// Los valores ausentes o en blanco se consideran no definidos. Cuando dos
    /// variables del **mismo** backend llegan a la vez, gana el socket y se avisa
    /// una sola vez de la URL que se ignora. El backend de más prioridad gana sin
    /// que las variables del otro lleguen a evaluarse como transporte.
    ///
    /// Está separado de [`JournaldReader::from_env`] para poder probarlo sin
    /// tocar las variables de entorno del proceso.
    pub fn from_values(
        docker_socket: Option<String>,
        docker_url: Option<String>,
        socket: Option<String>,
        url: Option<String>,
        container_name: Option<String>,
    ) -> Self {
        let docker_socket = blank_to_none(docker_socket);
        let docker_url = blank_to_none(docker_url);
        let socket = blank_to_none(socket);
        let url = blank_to_none(url);
        let container_name =
            blank_to_none(container_name).unwrap_or_else(|| DEFAULT_CONTAINER_NAME.to_string());

        if let Some(path) = docker_socket {
            if let Some(url) = docker_url {
                warn!(
                    "journald reader: ignoring SHUUL_DOCKER_URL={url}: \
                     SHUUL_DOCKER_SOCKET={path} takes precedence"
                );
            }
            return Self {
                transport: JournaldTransport::DockerUnixSocket {
                    path: PathBuf::from(path),
                },
                container_name,
                retry_delay: DEFAULT_RETRY_DELAY,
            };
        }

        if let Some(url) = docker_url {
            // Un path base en `SHUUL_DOCKER_URL` se ignora en silencio en
            // `build_request` —la petición va en forma origen—, así que hay que
            // avisarlo aquí igual que se avisa en `SHUUL_JOURNALD_URL`: de otro
            // modo el prefijo desaparece sin que nadie se entere.
            warn_ignored_base_path(&url, "SHUUL_DOCKER_URL", &docker_logs_path(&container_name));
            return Self {
                transport: JournaldTransport::DockerTcp { base_url: url },
                container_name,
                retry_delay: DEFAULT_RETRY_DELAY,
            };
        }

        match (socket, url) {
            (Some(path), Some(url)) => {
                warn!(
                    "journald reader: ignoring SHUUL_JOURNALD_URL={url}: \
                     SHUUL_JOURNALD_SOCKET={path} takes precedence"
                );
                Self::unix(path, &container_name)
            },
            (Some(path), None) => Self::unix(path, &container_name),
            (None, Some(url)) => {
                warn_ignored_base_path(&url, "SHUUL_JOURNALD_URL", "/entries");
                Self::new(&url, &container_name)
            },
            (None, None) => Self::new(DEFAULT_GATEWAY_URL, &container_name),
        }
    }

    /// Construye un reader desde las variables de entorno documentadas:
    /// `SHUUL_DOCKER_SOCKET`, `SHUUL_DOCKER_URL`, `SHUUL_JOURNALD_SOCKET`,
    /// `SHUUL_JOURNALD_URL` y el contenedor, con los defaults documentados.
    ///
    /// El contenedor sigue la decisión D1 del `design.md`:
    /// `SHUUL_DOCKER_CONTAINER` → `SHUUL_JOURNALD_CONTAINER` → `traefik`. Así
    /// quien ya tenía definida la de journald sigue apuntando al mismo
    /// contenedor al cambiar de backend, y quien quiera distinguirlos define la
    /// de Docker.
    pub fn from_env() -> Self {
        let container = blank_to_none(std::env::var("SHUUL_DOCKER_CONTAINER").ok())
            .or_else(|| std::env::var("SHUUL_JOURNALD_CONTAINER").ok());

        Self::from_values(
            std::env::var("SHUUL_DOCKER_SOCKET").ok(),
            std::env::var("SHUUL_DOCKER_URL").ok(),
            std::env::var("SHUUL_JOURNALD_SOCKET").ok(),
            std::env::var("SHUUL_JOURNALD_URL").ok(),
            container,
        )
    }

    /// Target de la petición en forma origen (`path?query`), idéntico en ambos
    /// transportes. El nombre del contenedor va URL-encoded.
    pub fn request_target(&self) -> String {
        format!(
            "/entries?follow&CONTAINER_NAME={}",
            urlencoding::encode(&self.container_name)
        )
    }

    /// Representación del endpoint para los logs: `unix:/ruta` o
    /// `scheme://host:puerto`.
    ///
    /// Para TCP se muestra el endpoint efectivo, no la URL configurada: si
    /// `SHUUL_JOURNALD_URL` trae un path base, éste se ignora al construir la
    /// petición y no debe aparecer en los logs.
    pub fn endpoint_display(&self) -> String {
        match &self.transport {
            JournaldTransport::Tcp { base_url } => tcp_endpoint(base_url).map_or_else(
                |_| base_url.trim_end_matches('/').to_string(),
                |endpoint| format!("http://{}", endpoint.authority()),
            ),
            JournaldTransport::UnixSocket { path } => format!("unix:{}", path.display()),
            // Mismo cálculo que el brazo de journald (`tcp_endpoint` +
            // `map_or_else`) para que el endpoint efectivo no se pueda desviar
            // entre los dos backends: si `SHUUL_DOCKER_URL` trae un path base,
            // éste se ignora al construir la petición y no debe aparecer en los
            // logs. El fallback es la URL recortada, con el prefijo del backend.
            JournaldTransport::DockerTcp { base_url } => tcp_endpoint(base_url).map_or_else(
                |_| {
                    format!(
                        "docker:{} ({})",
                        base_url.trim_end_matches('/'),
                        self.container_name
                    )
                },
                |endpoint| {
                    format!(
                        "docker:http://{} ({})",
                        endpoint.authority(),
                        self.container_name
                    )
                },
            ),
            JournaldTransport::DockerUnixSocket { path } => {
                format!("docker:unix:{} ({})", path.display(), self.container_name)
            },
        }
    }
}

impl Default for JournaldReader {
    /// Reader con los valores por defecto: gatewayd en local y contenedor
    /// `traefik`.
    fn default() -> Self {
        Self::new(DEFAULT_GATEWAY_URL, DEFAULT_CONTAINER_NAME)
    }
}

/// Motivo por el que terminó una conexión de streaming.
#[derive(Debug, PartialEq, Eq)]
enum StreamOutcome {
    /// La señal de shutdown llegó mientras se leía del gateway.
    Shutdown,
    /// El gateway cerró el stream y hay que reconectar.
    Disconnected,
}

/// Espera a que el canal de shutdown valga `true`.
///
/// `changed()` devuelve `Err` cuando se sueltan todos los senders. Eso no es
/// una orden de parada: si se tratara como tal, el reader terminaría en
/// silencio. Por eso, con el canal cerrado, esta función deja de resolver.
async fn wait_for_shutdown(shutdown: &mut tokio::sync::watch::Receiver<bool>) {
    if shutdown.wait_for(|stop| *stop).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// Instante actual en microsegundos desde `UNIX_EPOCH`, la misma unidad que
/// usa `__REALTIME_TIMESTAMP` en el journal.
fn now_micros() -> u64 {
    cutoff_micros_at(SystemTime::now())
}

/// Corte en microsegundos desde `UNIX_EPOCH` para el instante `at`.
///
/// **Fail-closed:** si `at` es anterior a la época el corte vale `u64::MAX` y
/// descarta todas las entradas. Es deliberado — mejor perder una entrada que
/// banear una IP por evidencia vieja — y es lo contrario de devolver `0`, que
/// aceptaría el journal entero.
fn cutoff_micros_at(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH).map_or(u64::MAX, |elapsed| {
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
mod tests_cutoff {
    use super::cutoff_micros_at;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_cutoff_is_the_instant_in_microseconds() {
        let at = UNIX_EPOCH + Duration::from_micros(1_723_897_600_123_456);
        assert_eq!(cutoff_micros_at(at), 1_723_897_600_123_456);
    }

    #[test]
    fn test_cutoff_before_the_epoch_is_fail_closed() {
        let before = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(
            cutoff_micros_at(before),
            u64::MAX,
            "a clock before UNIX_EPOCH must discard everything, not accept the whole journal"
        );
    }

    #[test]
    fn test_cutoff_of_the_epoch_itself_is_zero() {
        assert_eq!(
            cutoff_micros_at(UNIX_EPOCH),
            0,
            "the epoch is a known instant: only entries older than it are discarded"
        );
    }
}

/// Espera por defecto entre reintentos cuando el gateway no está disponible.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(30);

/// URL por defecto del endpoint de `systemd-journal-gatewayd`.
pub const DEFAULT_GATEWAY_URL: &str = "http://localhost:19531";

/// Contenedor por defecto cuyos access logs se leen.
pub const DEFAULT_CONTAINER_NAME: &str = "traefik";

/// Tiempo máximo para conectar, completar el handshake HTTP/1.1 y obtener la
/// cabecera de respuesta del gateway.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Microsegundos que tiene un segundo: el corte por intento de conexión se mide
/// en microsegundos y el `since` de la API de Docker, que tiene granularidad de
/// segundo, en segundos desde `UNIX_EPOCH`.
const MICROS_PER_SECOND: u64 = 1_000_000;

/// Versión de la API del motor de Docker que se consulta.
pub const DOCKER_API_VERSION: &str = "v1.44";

/// Un valor de configuración ausente o en blanco equivale a no definido.
fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl JournaldReader {
    /// Bucle de streaming: conecta con gatewayd, recibe NDJSON y entrega cada
    /// access log de Traefik al pipeline Jail.
    ///
    /// Si el gateway no está disponible se registra un warning y se reintenta
    /// tras [`JournaldReader::retry_delay`]. Termina limpiamente cuando
    /// `shutdown` pasa a `true`.
    ///
    /// # Errors
    ///
    /// En la práctica no devuelve error: cualquier fallo del gateway —incluida
    /// una URL no soportada como `https`— es recuperable y se reintenta, para
    /// que el WAF siga funcionando. El `Result` se mantiene en la firma por si
    /// en el futuro aparece un fallo irrecuperable.
    pub async fn run(
        &self,
        app_state: Arc<AppState>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), JournalError> {
        // Last cursor seen; survives reconnects so no entry is missed.
        let mut last_cursor: Option<String> = None;

        loop {
            if *shutdown.borrow() {
                info!("journald reader: shutdown requested");
                return Ok(());
            }

            match self
                .stream_once(&app_state, &mut shutdown, &mut last_cursor)
                .await
            {
                Ok(StreamOutcome::Shutdown) => return Ok(()),
                Ok(StreamOutcome::Disconnected) => {
                    debug!("journald reader: stream closed, reconnecting");
                },
                Err(e) => self.warn_backend_unavailable(&e),
            }

            // Backoff that stays responsive to the shutdown signal.
            tokio::select! {
                () = tokio::time::sleep(self.retry_delay) => {},
                () = wait_for_shutdown(&mut shutdown) => return Ok(()),
            }
        }
    }

    /// Abre una conexión de streaming y procesa las líneas que van llegando.
    async fn stream_once(
        &self,
        app_state: &Arc<AppState>,
        shutdown: &mut tokio::sync::watch::Receiver<bool>,
        last_cursor: &mut Option<String>,
    ) -> Result<StreamOutcome, JournalError> {
        // Every attempt fixes its own cutoff *before* asking the gateway for
        // anything: whatever the journal accumulated while this reader was
        // down (or between two attempts) must not reach the Jail pipeline.
        // Replaying it would feed `RateLimiter::record()` with failures that
        // happened long ago and ban IPs on startup.
        let cutoff_micros = now_micros();

        // Bound with a name on purpose: `_` would drop the ConnectionTask
        // immediately and stop the body from ever being driven.
        let (_connection, response) = tokio::select! {
            biased;

            // A shutdown must not wait for CONNECT_TIMEOUT to elapse.
            () = wait_for_shutdown(shutdown) => return Ok(StreamOutcome::Shutdown),

            response = self.send_request(last_cursor.as_deref(), cutoff_micros) => response?,
        };

        let status = response.status();
        if !status.is_success() {
            // A stale cursor (e.g. the journal was reset) must not wedge the
            // reader: drop it so the next attempt starts from scratch.
            if last_cursor.is_some() {
                warn!(
                    "journald reader: gateway rejected cursor with HTTP {status}; restarting without it"
                );
                *last_cursor = None;
            }
            return Err(JournalError::connection(self.error_status_cause(status)));
        }

        info!(
            "journald reader: streaming access logs from {}",
            self.endpoint_display()
        );

        let mut body = response.into_body();
        // El backend en curso decide cómo se trocea el cuerpo —NDJSON de gatewayd
        // o frames multiplexados de la API de Docker— en un único punto de
        // decisión. El bucle, la petición y la reconexión son comunes a los dos
        // (tarea 5.3): no hay un segundo `send_request` ni un segundo `loop`.
        let mut framing = StreamFraming::for_transport(&self.transport);
        loop {
            let frame = tokio::select! {
                biased;

                () = wait_for_shutdown(shutdown) => return Ok(StreamOutcome::Shutdown),

                frame = body.frame() => frame,
            };

            let Some(frame) = frame else {
                // El stream se cerró: un frame a medias en el backend de Docker
                // se descarta aquí y no se arrastra a la siguiente conexión.
                let discarded = framing.finish();
                if discarded > 0 {
                    debug!(
                        "journald reader: discarding {discarded} bytes of an unfinished Docker \
                         frame at stream close"
                    );
                }
                return Ok(StreamOutcome::Disconnected);
            };
            let frame = frame.map_err(|e| JournalError::socket_from(e.to_string(), e))?;

            // Trailer frames carry no payload, so only data frames are appended.
            let Some(data) = frame.data_ref() else {
                continue;
            };

            match &mut framing {
                // NDJSON de gatewayd: comportamiento sin cambios respecto al
                // camino heredado.
                StreamFraming::Journald { buffer } => {
                    buffer.extend_from_slice(data);
                    for line in drain_ndjson_lines(buffer) {
                        let entry = match parse_journal_entry(&line) {
                            Ok(entry) => entry,
                            Err(e) => {
                                debug!("journald reader: skipping unsuitable journal line: {e}");
                                continue;
                            },
                        };

                        let cursor = entry.get("__CURSOR");

                        // The cursor is only a hint: `?follow` can hand back the entry
                        // the cursor points at after a reconnect. `last_cursor` is
                        // `None` on startup, so this never fires for the backlog — at
                        // most once per reconnect.
                        if cursor.is_some() && cursor == last_cursor.as_ref() {
                            debug!(
                                "journald reader: discarding the entry already processed at the cursor"
                            );
                            continue;
                        }

                        // An entry that cannot be placed after the start of this
                        // attempt is not ours to count. A missing or malformed
                        // timestamp is treated the same way: better to lose an entry
                        // than to ban an IP on stale evidence. Silence is deliberate:
                        // on startup this runs for every entry of the backlog, so an
                        // `INFO` line (or worse) here would flood the log.
                        let Some(timestamp) = entry
                            .get("__REALTIME_TIMESTAMP")
                            .and_then(|value| value.parse::<u64>().ok())
                        else {
                            debug!(
                                "journald reader: discarding entry without a usable __REALTIME_TIMESTAMP"
                            );
                            continue;
                        };
                        if timestamp < cutoff_micros {
                            debug!(
                                "journald reader: discarding entry from before this connection attempt \
                                 ({timestamp} < {cutoff_micros})"
                            );
                            continue;
                        }

                        if let Some(cursor) = cursor {
                            *last_cursor = Some(cursor.clone());
                        }

                        self.process_entry(&entry, app_state).await;
                    }
                },
                // Frames multiplexados de la API de Docker: cada frame cerrado
                // se entrega al pipeline por el camino directo de Docker.
                StreamFraming::Docker { decoder } => {
                    for decoded in decoder.push(data)? {
                        if let Some(payload) = decoded.into_pipeline_line() {
                            self.process_docker_payload(&payload, cutoff_micros, app_state)
                                .await;
                        }
                    }
                },
            }
        }
    }

    /// Conecta con gatewayd, completa el handshake HTTP/1.1 y envía la
    /// petición de streaming.
    ///
    /// El intento completo (conexión + handshake + cabecera de respuesta) está
    /// acotado por [`CONNECT_TIMEOUT`]: un servidor que acepta la conexión y
    /// nunca contesta no puede bloquear al reader para siempre.
    ///
    /// `hyper` sólo escribe la petición mientras se sondea la tarea de
    /// `Connection`, así que ésta se lanza en el mismo intento. El guard se
    /// devuelve junto a la respuesta para que quien lea el cuerpo lo retenga
    /// hasta el final.
    async fn send_request(
        &self,
        cursor: Option<&str>,
        cutoff_micros: u64,
    ) -> Result<(ConnectionTask, Response<Incoming>), JournalError> {
        let request = self.build_request(cursor, cutoff_micros)?;
        let endpoint = self.endpoint_display();

        let (connection_task, response) = tokio::time::timeout(CONNECT_TIMEOUT, async {
            let io = self.connect().await?;
            let (mut sender, connection) =
                http1::handshake(TokioIo::new(io)).await.map_err(|e| {
                    JournalError::connection_from(
                        format!("HTTP handshake with {endpoint} failed: {e}"),
                        e,
                    )
                })?;

            let connection_task = ConnectionTask::spawn(connection);

            let response = sender.send_request(request).await.map_err(|e| {
                JournalError::connection_from(format!("request to {endpoint} failed: {e}"), e)
            })?;

            Ok::<_, JournalError>((connection_task, response))
        })
        .await
        .map_err(|_| {
            JournalError::connection(format!(
                "connecting to {endpoint} timed out after {}s",
                CONNECT_TIMEOUT.as_secs()
            ))
        })??;

        Ok((connection_task, response))
    }

    /// Causa de un intento fallido por un status HTTP de error.
    ///
    /// El texto de gatewayd se conserva **literal** (`gateway returned HTTP
    /// {status}`): es el guardián de que el camino heredado no cambió. Con la
    /// API de Docker, nombrar un «gateway» sería falso —no hay ninguno— y no
    /// diría lo que de verdad pasa, así que la causa es backend-aware y un `404`
    /// se explica como lo que es: el contenedor no existe.
    fn error_status_cause(&self, status: StatusCode) -> String {
        if !self.transport.is_docker() {
            return format!("gateway returned HTTP {status}");
        }

        let container = &self.container_name;
        if status == StatusCode::NOT_FOUND {
            format!(
                "the Docker API returned HTTP {status} for container {container} (no such container)"
            )
        } else {
            format!("the Docker API returned HTTP {status} for container {container}")
        }
    }

    /// Aviso de que el backend no está disponible, emitido desde el **único**
    /// bucle de `run()` antes de reintentar.
    ///
    /// Nombra el backend y, en el caso de Docker, el endpoint efectivo —más la
    /// causa, que ya viene en `error`—: quien lee los logs no tiene que adivinar
    /// cuál de los dos falló ni a dónde se está intentando conectar. El texto de
    /// gatewayd se conserva **literal**, porque es el guardián de que el camino
    /// heredado no cambió.
    fn warn_backend_unavailable(&self, error: &JournalError) {
        let retry = self.retry_delay.as_secs();
        if self.transport.is_docker() {
            warn!(
                "docker api not available: {}: {error} (retrying in {retry}s)",
                self.endpoint_display()
            );
        } else {
            warn!("journald gateway not available: {error} (retrying in {retry}s)");
        }
    }

    /// Abre el socket del transporte configurado.
    ///
    /// Lo único que cambia entre los dos backends es la **petición**, no el
    /// transporte: «conectar por un path Unix» y «conectar por una autoridad TCP»
    /// son una sola implementación cada uno, compartida por gatewayd y por la API
    /// de Docker.
    async fn connect(&self) -> Result<BoxedIo, JournalError> {
        match &self.transport {
            JournaldTransport::UnixSocket { path }
            | JournaldTransport::DockerUnixSocket { path } => connect_unix(path).await,
            JournaldTransport::Tcp { base_url } | JournaldTransport::DockerTcp { base_url } => {
                connect_tcp(base_url).await
            },
        }
    }

    /// Construye la petición del backend configurado. **Gate único**: el camino
    /// real (`send_request`) y los tests llaman a esta misma función, de modo
    /// que la forma de la petición no se puede desviar entre uno y otros (tarea
    /// 5.3), y se puede asertar sin levantar ningún servidor.
    ///
    /// `cursor` es la pista de reanudación de gatewayd (`Range`) y
    /// `cutoff_micros` el corte del intento de conexión en curso, del que la API
    /// de Docker deriva su `since` en segundos. Cada backend usa el suyo e
    /// ignora el otro.
    fn build_request(
        &self,
        cursor: Option<&str>,
        cutoff_micros: u64,
    ) -> Result<Request<Empty<Bytes>>, JournalError> {
        if let JournaldTransport::DockerTcp { .. } | JournaldTransport::DockerUnixSocket { .. } =
            &self.transport
        {
            let request = Request::builder()
                .method(Method::GET)
                .uri(self.docker_request_target(cutoff_micros / MICROS_PER_SECOND))
                .header(header::HOST, self.host_header()?)
                .header(header::ACCEPT, "application/json")
                .body(Empty::<Bytes>::new())
                .map_err(|e| {
                    JournalError::connection_from(
                        format!("failed to build the Docker request: {e}"),
                        e,
                    )
                })?;

            return Ok(request);
        }

        let mut builder = Request::builder()
            .method(Method::GET)
            .uri(self.request_target())
            .header(header::HOST, self.host_header()?)
            .header(header::ACCEPT, "application/json");

        if let Some(cursor) = cursor {
            // `entries=<cursor>:1` skips the cursor entry itself, so the stream
            // resumes right after the last entry already processed.
            builder = builder.header(header::RANGE, format!("entries={cursor}:1"));
        }

        builder.body(Empty::<Bytes>::new()).map_err(|e| {
            JournalError::connection_from(format!("failed to build the request: {e}"), e)
        })
    }

    /// Valor de la cabecera `Host`: la autoridad del endpoint en los dos
    /// transportes TCP —`host:puerto`, con corchetes si es IPv6, como exige el
    /// RFC—, `localhost` en el socket de gatewayd —donde no hay autoridad
    /// real— y `docker` en el del motor, que es la convención de la propia API
    /// de Docker.
    fn host_header(&self) -> Result<String, JournalError> {
        match &self.transport {
            JournaldTransport::Tcp { base_url } | JournaldTransport::DockerTcp { base_url } => {
                Ok(tcp_endpoint(base_url)?.authority())
            },
            JournaldTransport::UnixSocket { .. } => Ok("localhost".to_string()),
            JournaldTransport::DockerUnixSocket { .. } => Ok("docker".to_string()),
        }
    }

    /// Target en forma origen de la petición de la **API de Docker**: el
    /// contenedor viaja URL-encoded en el segmento del path y `since` es el
    /// corte del intento en curso en segundos desde `UNIX_EPOCH`.
    ///
    /// `stderr=1` se pide aunque sus cargas se descarten después (decisión D3
    /// del `design.md`): así un error de Traefik en `stderr` aparece en los logs
    /// del reader en vez de desaparecer.
    fn docker_request_target(&self, since_epoch: u64) -> String {
        format!(
            "{}?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={since_epoch}",
            docker_logs_path(&self.container_name)
        )
    }

    /// Convierte los campos de una entrada del journal en un `ReportPayload`
    /// y lo entrega al pipeline Jail. Las entradas que no son access logs se
    /// descartan.
    async fn process_entry(&self, entry: &HashMap<String, String>, app_state: &Arc<AppState>) {
        match extract_report_payload(entry, &self.container_name) {
            Ok(Some(payload)) => {
                if let Err(e) = run_jail_pipeline(app_state, &payload).await {
                    warn!("journald reader: jail pipeline error: {e}");
                }
            },
            Ok(None) => {},
            Err(e) => {
                debug!("journald reader: MESSAGE is not a Traefik access log: {e}");
            },
        }
    }

    /// Entrega al pipeline Jail la carga de un frame de la **API de Docker**.
    ///
    /// El camino de Docker va **directo** a [`parse_traefik_access_log`]: no pasa
    /// por [`extract_report_payload`] ni por [`is_traefik_entry`], que exigen el
    /// campo `CONTAINER_NAME` —que el stream no trae— y descartarían el 100 % de
    /// las líneas en silencio.
    ///
    /// El instante que antepone `timestamps=1` alimenta el corte por intento
    /// ([`payload_is_after_cutoff`]) y la línea ya sin el prefijo alimenta el
    /// parser. Una carga sin prefijo utilizable, o anterior al corte de la
    /// conexión en curso, se descarta **sin abortar el stream**: mejor perder una
    /// carga que banear una IP por evidencia vieja.
    async fn process_docker_payload(
        &self,
        payload: &str,
        cutoff_micros: u64,
        app_state: &Arc<AppState>,
    ) {
        let Some((instant_micros, line)) = split_timestamp_prefix(payload) else {
            debug!("journald reader: discarding Docker payload without a usable timestamp prefix");
            return;
        };
        if !payload_is_after_cutoff(Some(instant_micros), cutoff_micros) {
            debug!(
                "journald reader: discarding Docker payload from before this connection attempt \
                 ({instant_micros} <= {cutoff_micros})"
            );
            return;
        }

        match parse_traefik_access_log(line) {
            Ok(Some(payload)) => {
                if let Err(e) = run_jail_pipeline(app_state, &payload).await {
                    warn!("journald reader: jail pipeline error: {e}");
                }
            },
            Ok(None) => {},
            Err(e) => {
                debug!("journald reader: Docker payload is not a Traefik access log: {e}");
            },
        }
    }
}

/// Endpoint TCP ya validado.
struct TcpEndpoint {
    /// Host sin corchetes, tal como lo acepta `TcpStream::connect`.
    connect_host: String,
    /// Host con corchetes si es IPv6, para la cabecera `Host` y para los logs.
    authority_host: String,
    port: u16,
}

impl TcpEndpoint {
    /// `host:puerto`, con corchetes sólo si el host es IPv6.
    fn authority(&self) -> String {
        format!("{}:{}", self.authority_host, self.port)
    }
}

/// Conecta con un socket `AF_UNIX` del sistema de ficheros.
///
/// El mismo código sirve a gatewayd y a la API de Docker: el socket no sabe de
/// protocolos.
async fn connect_unix(path: &Path) -> Result<BoxedIo, JournalError> {
    let stream = UnixStream::connect(path).await.map_err(|e| {
        JournalError::connection_from(
            format!("failed to connect to unix:{}: {e}", path.display()),
            e,
        )
    })?;
    Ok(Box::new(stream))
}

/// Conecta con la autoridad (`host:puerto`) de una URL base HTTP.
///
/// El mismo código sirve a gatewayd y a la API de Docker: lo único que se
/// resuelve aquí es a dónde ir.
async fn connect_tcp(base_url: &str) -> Result<BoxedIo, JournalError> {
    let endpoint = tcp_endpoint(base_url)?;
    let stream = TcpStream::connect((endpoint.connect_host.as_str(), endpoint.port))
        .await
        .map_err(|e| {
            JournalError::connection_from(
                format!("failed to connect to {}: {e}", endpoint.authority()),
                e,
            )
        })?;
    Ok(Box::new(stream))
}

/// Extrae `host:puerto` de una URL base HTTP.
///
/// El path base, si lo hay, se ignora: la petición siempre se envía en forma
/// origen contra [`JournaldReader::request_target`], igual que en el socket.
///
/// `Uri::host()` devuelve los literales IPv6 entre corchetes (`[::1]`), que
/// `TcpStream::connect` no acepta: se quitan para conectar y se reponen para
/// la cabecera `Host`.
///
/// # Errors
///
/// Devuelve [`JournalError::Connection`] si la URL no es válida, no lleva
/// esquema o usa `https`, que no está soportado (gatewayd se consulta en local
/// y sin TLS por diseño).
fn tcp_endpoint(base_url: &str) -> Result<TcpEndpoint, JournalError> {
    // Un path absoluto aquí es casi siempre el socket en la variable
    // equivocada: nombrar la variable correcta evita diagnósticos engañosos,
    // tanto si el valor no parsea como URI como si le falta el esquema.
    let hint = if base_url.starts_with('/') {
        " (hint: absolute paths belong in SHUUL_JOURNALD_SOCKET)"
    } else {
        ""
    };

    let uri: Uri = base_url.parse().map_err(|e| {
        JournalError::connection_from(format!("invalid gateway URL {base_url}: {e}{hint}"), e)
    })?;

    match uri.scheme_str() {
        Some("http") => {},
        Some("https") => {
            return Err(JournalError::connection(format!(
                "https is not supported for the gateway URL ({base_url}); use http or a unix socket"
            )));
        },
        Some(other) => {
            return Err(JournalError::connection(format!(
                "unsupported scheme {other} in gateway URL {base_url}"
            )));
        },
        None => {
            return Err(JournalError::connection(format!(
                "gateway URL {base_url} has no scheme{hint}"
            )));
        },
    }

    let host = uri
        .host()
        .ok_or_else(|| JournalError::connection(format!("gateway URL {base_url} has no host")))?;
    let connect_host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .to_string();
    let authority_host = if connect_host.contains(':') {
        format!("[{connect_host}]")
    } else {
        connect_host.clone()
    };

    Ok(TcpEndpoint {
        connect_host,
        authority_host,
        port: uri.port_u16().unwrap_or(80),
    })
}

/// Avisa de que el path de la URL se ignora, para que una URL con prefijo
/// (`http://proxy/journald`) no acabe en un 404 silencioso.
///
/// El nombre de la variable y el target de la petición entran por parámetro
/// para que **los dos backends** compartan el aviso sin divergir: el log de
/// journald se conserva **literal** —misma variable, mismo `/entries`—, y el de
/// Docker nombra `SHUUL_DOCKER_URL` y el path real de los logs del contenedor.
fn warn_ignored_base_path(url: &str, variable: &str, target: &str) {
    if let Ok(uri) = url.parse::<Uri>() {
        let path = uri.path();
        if !path.is_empty() && path != "/" {
            warn!(
                "journald reader: ignoring base path {path} in {variable}={url}: \
                 requests always use {target}"
            );
        }
    }
}

/// Path de la petición de logs de la API de Docker, **sin** la query.
///
/// Es el punto único donde se escribe el path del contenedor: lo usan el
/// constructor del target y el aviso del path base ignorado, de modo que no hay
/// dos sitios que puedan desviarse.
fn docker_logs_path(container_name: &str) -> String {
    format!(
        "/{DOCKER_API_VERSION}/containers/{}/logs",
        urlencoding::encode(container_name)
    )
}

// ── Task 2: parse_journal_entry (Journal JSON Format / NDJSON) ──

/// Parsea UNA línea del Journal JSON Format (NDJSON) a un mapa campo→valor.
///
/// gatewayd sirve las entradas con `Accept: application/json` como un objeto
/// JSON por línea. Este parser las normaliza a un `HashMap`:
///
/// - los valores de tipo string se conservan tal cual;
/// - los campos multi-valor (array de strings) usan el primer elemento;
/// - los campos binarios (array de números), booleanos y `null` se ignoran,
///   porque no son utilizables como valores de texto del journal.
///
/// # Errors
///
/// Devuelve [`JournalError::Parse`] si la línea está vacía, no es JSON válido,
/// no es un objeto JSON o no contiene ningún campo utilizable.
pub fn parse_journal_entry(line: &str) -> Result<HashMap<String, String>, JournalError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(JournalError::parse("empty journal entry"));
    }

    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| JournalError::parse_from(format!("invalid journal JSON: {e}"), e))?;

    let serde_json::Value::Object(object) = value else {
        return Err(JournalError::parse("journal entry is not a JSON object"));
    };

    let mut fields = HashMap::new();
    for (key, value) in object {
        if let Some(text) = journal_field_value(&value) {
            fields.insert(key, text);
        }
    }

    if fields.is_empty() {
        return Err(JournalError::parse("no usable fields in journal entry"));
    }

    Ok(fields)
}

/// Extrae el valor utilizable de un campo del Journal JSON Format.
///
/// Devuelve `None` para los tipos que no representan texto: arrays de números
/// (campos binarios), booleanos y `null`.
fn journal_field_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        // Multi-value field: the first usable element wins
        serde_json::Value::Array(items) => items.iter().find_map(journal_field_value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_GATEWAY_URL, JournalError, JournaldReader, JournaldTransport, parse_journal_entry,
    };

    // ── Task 1: Struct existence and default values ──

    #[test]
    fn test_journald_reader_default_values() {
        let reader = JournaldReader::default();
        assert_eq!(
            reader.transport,
            JournaldTransport::Tcp {
                base_url: DEFAULT_GATEWAY_URL.to_string()
            },
            "The default transport must be TCP against the local gatewayd"
        );
        assert_eq!(
            reader.container_name, "traefik",
            "Default container name should be traefik"
        );
    }

    #[test]
    fn test_journald_reader_custom_values() {
        let reader = JournaldReader::new("http://gateway.local:19531", "nginx");
        assert_eq!(
            reader.transport,
            JournaldTransport::Tcp {
                base_url: "http://gateway.local:19531".to_string()
            },
            "Custom gateway URL should be stored in the TCP transport"
        );
        assert_eq!(
            reader.container_name, "nginx",
            "Custom container name should be stored"
        );
    }

    #[test]
    fn test_journald_reader_unix_constructor() {
        let reader = JournaldReader::unix("/run/systemd/journal/gatewayd.sock", "traefik");
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: std::path::PathBuf::from("/run/systemd/journal/gatewayd.sock")
            },
            "unix() must build a UnixSocket transport"
        );
        assert_eq!(reader.container_name, "traefik");
    }

    // ── JournalError type existence ──

    #[test]
    fn test_journal_error_types_exist() {
        let err_socket = JournalError::socket("connection refused");
        assert!(
            err_socket.to_string().contains("connection refused"),
            "Socket should contain the message"
        );

        let err_parse = JournalError::parse("invalid format");
        assert!(
            err_parse.to_string().contains("invalid format"),
            "Parse should contain the message"
        );

        let err_traefik = JournalError::traefik_parse("bad json");
        assert!(
            err_traefik.to_string().contains("bad json"),
            "TraefikParse should contain the message"
        );

        let err_conn = JournalError::connection("socket not found");
        assert!(
            err_conn.to_string().contains("socket not found"),
            "Connection should contain the message"
        );
    }

    // ── Task 2: parse_journal_entry (Journal JSON Format / NDJSON) ──

    #[test]
    fn test_parse_journal_entry_valid() {
        let line = r#"{"__REALTIME_TIMESTAMP":"1723897600123456","CONTAINER_NAME":"traefik","CONTAINER_ID":"abc123","MESSAGE":"{\"ClientHost\":\"10.0.0.1\",\"OriginStatus\":401}"}"#;
        let result = parse_journal_entry(line);
        assert!(
            result.is_ok(),
            "Should parse a valid NDJSON line, got: {result:?}"
        );

        let fields = result.unwrap();
        assert_eq!(fields.get("CONTAINER_NAME").unwrap(), "traefik");
        assert_eq!(fields.get("CONTAINER_ID").unwrap(), "abc123");
        assert_eq!(
            fields.get("__REALTIME_TIMESTAMP").unwrap(),
            "1723897600123456"
        );
        assert!(
            fields.get("MESSAGE").unwrap().contains("OriginStatus"),
            "MESSAGE must keep the raw Traefik JSON as a string"
        );
    }

    #[test]
    fn test_parse_journal_entry_multi_value_uses_first() {
        let line = r#"{"_UDEV_DEVLINK":["/dev/alias1","/dev/alias2"],"CONTAINER_NAME":"traefik"}"#;
        let fields = parse_journal_entry(line).expect("should parse");
        assert_eq!(
            fields.get("_UDEV_DEVLINK").unwrap(),
            "/dev/alias1",
            "a multi-value field must use its first element"
        );
    }

    #[test]
    fn test_parse_journal_entry_binary_and_null_ignored() {
        let line = r#"{"CONTAINER_NAME":"traefik","BINARY":[116,104,105,115],"LARGE":null}"#;
        let fields = parse_journal_entry(line).expect("should parse");
        assert_eq!(fields.get("CONTAINER_NAME").unwrap(), "traefik");
        assert!(
            !fields.contains_key("BINARY"),
            "binary (number-array) fields must be ignored"
        );
        assert!(!fields.contains_key("LARGE"), "null fields must be ignored");
    }

    #[test]
    fn test_parse_journal_entry_invalid_json() {
        let result = parse_journal_entry("this is not json");
        assert!(result.is_err(), "invalid JSON must return an error");
        match result {
            Err(super::JournalError::Parse(msg, ..)) => {
                assert!(!msg.is_empty(), "error message should not be empty");
            },
            other => panic!("expected Parse error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_journal_entry_empty() {
        assert!(
            parse_journal_entry("").is_err(),
            "empty line must return an error"
        );
    }

    #[test]
    fn test_parse_journal_entry_whitespace() {
        assert!(
            parse_journal_entry("   \n  ").is_err(),
            "whitespace-only line must return an error"
        );
    }

    #[test]
    fn test_parse_journal_entry_no_usable_fields() {
        assert!(
            parse_journal_entry("{}").is_err(),
            "an empty JSON object has no usable fields"
        );
        assert!(
            parse_journal_entry(r#"{"A":null,"B":[1,2],"C":true}"#).is_err(),
            "a line whose fields are all unusable must return an error"
        );
    }

    #[test]
    fn test_parse_journal_entry_not_an_object() {
        assert!(
            parse_journal_entry("[1,2,3]").is_err(),
            "a JSON array is not a journal entry"
        );
        assert!(
            parse_journal_entry("\"just a string\"").is_err(),
            "a JSON string is not a journal entry"
        );
    }
}

// ── Task 3: Traefik entry filter ──

/// Comprueba si una entrada del journal pertenece al contenedor
/// cuyo nombre se pasa como argumento.
///
/// La entrada debe tener el campo `CONTAINER_NAME` (añadido por el
/// `LogDriver` journald de Docker) con el valor esperado.
pub fn is_traefik_entry(entry: &HashMap<String, String>, container_name: &str) -> bool {
    entry
        .get("CONTAINER_NAME")
        .is_some_and(|v| v.as_str() == container_name)
}

#[cfg(test)]
mod tests_task3 {
    use super::{HashMap, is_traefik_entry};

    fn make_entry(container_name: &str) -> HashMap<String, String> {
        let mut fields = HashMap::new();
        fields.insert("CONTAINER_NAME".to_string(), container_name.to_string());
        fields.insert("MESSAGE".to_string(), "some log".to_string());
        fields
    }

    #[test]
    fn test_is_traefik_entry_matches() {
        let entry = make_entry("traefik");
        assert!(
            is_traefik_entry(&entry, "traefik"),
            "entry with CONTAINER_NAME=traefik should match container_name=traefik"
        );
    }

    #[test]
    fn test_is_traefik_entry_no_match() {
        let entry = make_entry("nginx");
        assert!(
            !is_traefik_entry(&entry, "traefik"),
            "entry with CONTAINER_NAME=nginx should NOT match container_name=traefik"
        );
    }

    #[test]
    fn test_is_traefik_entry_missing_field() {
        let mut entry = HashMap::new();
        entry.insert("MESSAGE".to_string(), "no container".to_string());
        assert!(
            !is_traefik_entry(&entry, "traefik"),
            "entry without CONTAINER_NAME should return false"
        );
    }

    #[test]
    fn test_is_traefik_entry_different_container() {
        let entry = make_entry("my-other-app");
        assert!(
            !is_traefik_entry(&entry, "traefik"),
            "entry with CONTAINER_NAME=my-other-app should not match traefik"
        );
    }
}

// ── Task 4: parse_traefik_access_log ──

/// Modelo interno de los campos relevantes del access log JSON de Traefik.
#[derive(Debug, serde::Deserialize)]
struct TraefikAccessLog {
    #[serde(rename = "ClientHost")]
    client_host: Option<String>,
    #[serde(rename = "ClientAddr")]
    client_addr: Option<String>,
    #[serde(rename = "OriginStatus")]
    origin_status: Option<i64>,
    #[serde(rename = "RequestPath")]
    request_path: Option<String>,
    #[serde(rename = "RequestMethod")]
    request_method: Option<String>,
    #[serde(rename = "RequestHost")]
    request_host: Option<String>,
    #[serde(rename = "RequestScheme")]
    request_scheme: Option<String>,
    #[serde(rename = "request_User-Agent")]
    request_user_agent: Option<String>,
    #[serde(rename = "request_Referer")]
    request_referer: Option<String>,
    #[serde(rename = "request_Content-Type")]
    request_content_type: Option<String>,
    #[serde(rename = "request_Accept-Language")]
    request_accept_language: Option<String>,
    #[serde(rename = "request_X-Request-Id")]
    request_x_request_id: Option<String>,
}

/// Extrae la IP sin el puerto de una cadena `IP:puerto` o `[IPv6]:puerto`.
fn strip_port(addr: &str) -> String {
    if let Some(rest) = addr.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        return rest[..end].to_string();
    }
    match addr.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host.to_string(),
        _ => addr.to_string(),
    }
}

/// Separa un `RequestPath` en (path, query).
fn split_path_query(request_path: &str) -> (String, Option<String>) {
    match request_path.split_once('?') {
        Some((path, query)) if !query.is_empty() => (path.to_string(), Some(query.to_string())),
        Some((path, _)) => (path.to_string(), None),
        None => (request_path.to_string(), None),
    }
}

/// Parsea el JSON del access log de Traefik y lo convierte en un
/// [`ReportPayload`].
///
/// Devuelve `Ok(None)` cuando la entrada no representa una respuesta real
/// del backend (`OriginStatus == 0`, ausente, o sin IP de cliente) y por
/// tanto debe ignorarse en el pipeline Jail.
///
/// # Errors
///
/// Devuelve [`JournalError::TraefikParse`] si el texto no es JSON válido.
pub fn parse_traefik_access_log(json_str: &str) -> Result<Option<ReportPayload>, JournalError> {
    let log: TraefikAccessLog = serde_json::from_str(json_str)
        .map_err(|e| JournalError::traefik_parse_from(format!("invalid JSON: {e}"), e))?;

    // ── Resolve client IP: ClientHost preferred, ClientAddr fallback ──
    let ip_address = log.client_host.filter(|s| !s.is_empty()).or_else(|| {
        log.client_addr
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| strip_port(s))
    });

    let Some(ip_address) = ip_address else {
        // No client IP → cannot attribute the request to an IP → skip
        return Ok(None);
    };

    // ── Only backend responses count (OriginStatus != 0) ──
    let Some(origin_status) = log.origin_status else {
        return Ok(None);
    };
    let Ok(status_code) = u16::try_from(origin_status) else {
        return Ok(None);
    };
    if status_code == 0 {
        return Ok(None);
    }

    let (path, query) = log.request_path.as_deref().map_or((None, None), |rp| {
        let (p, q) = split_path_query(rp);
        (Some(p), q)
    });

    Ok(Some(ReportPayload {
        ip_address,
        status_code,
        path,
        method: log.request_method,
        user_agent: log.request_user_agent,
        referer: log.request_referer,
        fqdn: log.request_host,
        query,
        content_type: log.request_content_type,
        accept_language: log.request_accept_language,
        x_request_id: log.request_x_request_id,
        protocol: log.request_scheme,
    }))
}

#[cfg(test)]
mod tests_task4 {
    use super::parse_traefik_access_log;

    const FULL_JSON: &str = r#"{
        "ClientHost": "10.0.0.1",
        "ClientAddr": "10.0.0.1:54321",
        "OriginStatus": 401,
        "DownstreamStatus": 401,
        "RequestPath": "/api/login?next=%2Fdash",
        "RequestMethod": "POST",
        "RequestHost": "example.com",
        "RequestScheme": "https",
        "request_User-Agent": "curl/7.68",
        "request_Referer": "https://example.com/login",
        "request_Content-Type": "application/json",
        "request_Accept-Language": "es-ES",
        "request_X-Request-Id": "req-abc-123"
    }"#;

    #[test]
    fn test_parse_traefik_full_json() {
        let result = parse_traefik_access_log(FULL_JSON);
        assert!(result.is_ok(), "Should parse valid JSON, got: {result:?}");

        let payload = result.unwrap().expect("should be Some(payload)");
        assert_eq!(payload.ip_address, "10.0.0.1");
        assert_eq!(payload.status_code, 401);
        assert_eq!(payload.path.as_deref(), Some("/api/login"));
        assert_eq!(payload.query.as_deref(), Some("next=%2Fdash"));
        assert_eq!(payload.method.as_deref(), Some("POST"));
        assert_eq!(payload.fqdn.as_deref(), Some("example.com"));
        assert_eq!(payload.protocol.as_deref(), Some("https"));
        assert_eq!(payload.user_agent.as_deref(), Some("curl/7.68"));
        assert_eq!(
            payload.referer.as_deref(),
            Some("https://example.com/login")
        );
        assert_eq!(payload.content_type.as_deref(), Some("application/json"));
        assert_eq!(payload.accept_language.as_deref(), Some("es-ES"));
        assert_eq!(payload.x_request_id.as_deref(), Some("req-abc-123"));
    }

    #[test]
    fn test_parse_traefik_origin_status_zero_is_discarded() {
        let json = r#"{
            "ClientHost": "10.0.0.1",
            "OriginStatus": 0,
            "DownstreamStatus": 403,
            "RequestPath": "/admin",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let result = parse_traefik_access_log(json);
        assert!(result.is_ok(), "Should parse, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "OriginStatus == 0 means the request never reached a backend → must be discarded"
        );
    }

    #[test]
    fn test_parse_traefik_missing_origin_status_is_discarded() {
        let json = r#"{
            "ClientHost": "10.0.0.1",
            "RequestPath": "/",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let result = parse_traefik_access_log(json);
        assert!(result.is_ok(), "Should parse, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "Missing OriginStatus → cannot confirm backend response → discarded"
        );
    }

    #[test]
    fn test_parse_traefik_minimal_optional_fields_are_none() {
        let json = r#"{
            "ClientHost": "10.0.0.1",
            "OriginStatus": 200,
            "RequestPath": "/",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let result = parse_traefik_access_log(json);
        let payload = result.unwrap().expect("should be Some(payload)");
        assert_eq!(payload.status_code, 200);
        assert_eq!(payload.path.as_deref(), Some("/"));
        assert!(payload.query.is_none(), "no query → None");
        assert!(payload.user_agent.is_none());
        assert!(payload.referer.is_none());
        assert!(payload.content_type.is_none());
        assert!(payload.accept_language.is_none());
        assert!(payload.x_request_id.is_none());
        assert!(payload.protocol.is_none());
    }

    #[test]
    fn test_parse_traefik_client_addr_fallback_strips_port() {
        let json = r#"{
            "ClientAddr": "192.168.1.5:54321",
            "OriginStatus": 403,
            "RequestPath": "/secret",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let result = parse_traefik_access_log(json);
        let payload = result.unwrap().expect("should be Some(payload)");
        assert_eq!(
            payload.ip_address, "192.168.1.5",
            "ClientAddr fallback must strip the port"
        );
    }

    #[test]
    fn test_parse_traefik_no_client_ip_is_discarded() {
        let json = r#"{
            "OriginStatus": 200,
            "RequestPath": "/",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let result = parse_traefik_access_log(json);
        assert!(result.is_ok(), "Should parse, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "Without any client IP the request cannot be attributed → discarded"
        );
    }

    #[test]
    fn test_parse_traefik_invalid_json_errors() {
        let result = parse_traefik_access_log("this is not json");
        assert!(result.is_err(), "Invalid JSON must return an error");
        match result {
            Err(super::JournalError::TraefikParse(msg, ..)) => {
                assert!(!msg.is_empty(), "error message should not be empty");
            },
            other => panic!("expected TraefikParse error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_traefik_path_without_query() {
        let json = r#"{
            "ClientHost": "10.0.0.1",
            "OriginStatus": 404,
            "RequestPath": "/missing",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let payload = parse_traefik_access_log(json).unwrap().unwrap();
        assert_eq!(payload.path.as_deref(), Some("/missing"));
        assert!(payload.query.is_none(), "no '?' → query must be None");
    }

    #[test]
    fn test_parse_traefik_ipv6_client_addr_fallback() {
        let json = r#"{
            "ClientAddr": "[2001:db8::1]:443",
            "OriginStatus": 200,
            "RequestPath": "/",
            "RequestMethod": "GET",
            "RequestHost": "example.com"
        }"#;
        let payload = parse_traefik_access_log(json).unwrap().unwrap();
        assert_eq!(
            payload.ip_address, "2001:db8::1",
            "IPv6 ClientAddr fallback must strip the brackets and port"
        );
    }

    /// Escenario: An access log with only a subset of the request_ keys maps the present ones.
    /// Guardián de caracterización: con sólo `request_User-Agent` y
    /// `request_X-Request-Id`, esas dos claves se mapean, las otras tres quedan
    /// a `None` y la línea NO se descarta.
    #[test]
    fn test_parse_traefik_subset_of_request_keys_maps_present_ones() {
        let json = r#"{
            "ClientHost": "10.0.0.1",
            "OriginStatus": 200,
            "RequestPath": "/admin",
            "RequestMethod": "GET",
            "RequestHost": "example.com",
            "request_User-Agent": "Mozilla/5.0",
            "request_X-Request-Id": "abc-123"
        }"#;
        let result = parse_traefik_access_log(json);
        let payload = result
            .expect("un JSON válido debe parsear")
            .expect("una respuesta del backend no debe descartarse");

        assert_eq!(payload.user_agent.as_deref(), Some("Mozilla/5.0"));
        assert_eq!(payload.x_request_id.as_deref(), Some("abc-123"));
        assert!(payload.referer.is_none(), "referer ausente → None");
        assert!(
            payload.content_type.is_none(),
            "content_type ausente → None"
        );
        assert!(
            payload.accept_language.is_none(),
            "accept_language ausente → None"
        );
    }
}

// ── Task 6: extract_report_payload ──

/// Construye el `ReportPayload` para el pipeline Jail a partir de los campos
/// de una entrada del journal.
///
/// Devuelve `Ok(None)` cuando la entrada no debe procesarse:
///
/// - no pertenece al contenedor de Traefik,
/// - no trae el campo `MESSAGE`,
/// - el `MESSAGE` no representa una respuesta real del backend
///   (p. ej. `OriginStatus` ausente o `0`).
///
/// # Errors
///
/// Devuelve [`JournalError::TraefikParse`] si el `MESSAGE` no es JSON válido
/// (p. ej. una línea de log de Traefik que no es un access log).
pub fn extract_report_payload(
    entry: &HashMap<String, String>,
    container_name: &str,
) -> Result<Option<ReportPayload>, JournalError> {
    if !is_traefik_entry(entry, container_name) {
        return Ok(None);
    }

    let Some(message) = entry.get("MESSAGE") else {
        return Ok(None);
    };

    parse_traefik_access_log(message)
}

#[cfg(test)]
mod tests_task6 {
    use super::{HashMap, JournalError, extract_report_payload};

    fn entry_with(container: &str, message: Option<&str>) -> HashMap<String, String> {
        let mut fields = HashMap::new();
        fields.insert("CONTAINER_NAME".to_string(), container.to_string());
        fields.insert(
            "__REALTIME_TIMESTAMP".to_string(),
            "1723897600123456".to_string(),
        );
        if let Some(m) = message {
            fields.insert("MESSAGE".to_string(), m.to_string());
        }
        fields
    }

    const ACCESS_LOG: &str = r#"{"ClientHost":"10.0.0.1","OriginStatus":401,"RequestPath":"/api/login","RequestMethod":"POST","RequestHost":"example.com"}"#;

    #[test]
    fn test_extract_payload_from_traefik_entry() {
        let entry = entry_with("traefik", Some(ACCESS_LOG));
        let result = extract_report_payload(&entry, "traefik");
        assert!(result.is_ok(), "should succeed, got: {result:?}");

        let payload = result.unwrap().expect("should be Some(payload)");
        assert_eq!(payload.ip_address, "10.0.0.1");
        assert_eq!(payload.status_code, 401);
        assert_eq!(payload.path.as_deref(), Some("/api/login"));
        assert_eq!(payload.method.as_deref(), Some("POST"));
        assert_eq!(payload.fqdn.as_deref(), Some("example.com"));
    }

    #[test]
    fn test_extract_payload_wrong_container_is_discarded() {
        let entry = entry_with("nginx", Some(ACCESS_LOG));
        let result = extract_report_payload(&entry, "traefik");
        assert!(result.is_ok(), "should succeed, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "an entry from another container must be discarded"
        );
    }

    #[test]
    fn test_extract_payload_missing_message_is_discarded() {
        let entry = entry_with("traefik", None);
        let result = extract_report_payload(&entry, "traefik");
        assert!(result.is_ok(), "should succeed, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "an entry without MESSAGE must be discarded"
        );
    }

    #[test]
    fn test_extract_payload_origin_status_zero_is_discarded() {
        let message = r#"{"ClientHost":"10.0.0.1","OriginStatus":0,"DownstreamStatus":403,"RequestPath":"/admin","RequestMethod":"GET","RequestHost":"example.com"}"#;
        let entry = entry_with("traefik", Some(message));
        let result = extract_report_payload(&entry, "traefik");
        assert!(result.is_ok(), "should succeed, got: {result:?}");
        assert!(
            result.unwrap().is_none(),
            "OriginStatus == 0 means the request never reached a backend"
        );
    }

    #[test]
    fn test_extract_payload_invalid_message_json_errors() {
        let entry = entry_with("traefik", Some("Traefik starting, version 3.7"));
        let result = extract_report_payload(&entry, "traefik");
        match result {
            Err(JournalError::TraefikParse(msg, ..)) => {
                assert!(!msg.is_empty(), "error message should not be empty");
            },
            other => panic!("expected TraefikParse error, got: {other:?}"),
        }
    }
}

// ── Frames del stream de la API de Docker (`docker-logs-transport`) ──
//
// La API de Docker multiplexa el stream de un contenedor cuando éste NO tiene
// TTY: cada mensaje llega en un frame con una cabecera de 8 bytes —un byte de
// identificador de stream, tres de relleno y cuatro de longitud big-endian—.
// Este decodificador es incremental: mantiene el estado entre los chunks que
// llegan del socket, de modo que una cabecera o una carga partida entre dos
// lecturas se decodifica al completarse. El camino de journald sigue usando su
// NDJSON (`drain_ndjson_lines`, más abajo).
//
// Está cableado al **mismo** bucle de `stream_once` que el NDJSON: `StreamFraming`
// elige un backend u otro en un solo punto y el resto —conexión, petición,
// reconexión y respuesta a la señal de parada— es común a los dos (tarea 5.3).

/// Identificador de stream del primer byte de la cabecera de un frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockerStream {
    /// `0`: stdin. El endpoint `logs` no lo emite, pero la cabecera lo admite.
    Stdin,
    /// `1`: stdout, por donde sale el access log de Traefik.
    Stdout,
    /// `2`: stderr, que no alimenta el pipeline Jail.
    Stderr,
}

impl DockerStream {
    /// Identificador de stream del primer byte de una cabecera, o `None` si ese
    /// byte no es ninguno de los tres válidos.
    ///
    /// Un byte inválido es el síntoma de un contenedor arrancado con TTY, cuyo
    /// stream llega crudo y sin cabeceras: un JSON empieza por `{` (`0x7B`).
    const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Stdin),
            1 => Some(Self::Stdout),
            2 => Some(Self::Stderr),
            _ => None,
        }
    }

    /// Nombre con el que aparece el stream en los descartes.
    const fn name(self) -> &'static str {
        match self {
            Self::Stdin => "stdin",
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Frame ya decodificado del stream multiplexado de la API de Docker.
#[derive(Debug)]
struct DecodedFrame {
    /// Stream del que viene, según el primer byte de su cabecera.
    stream: DockerStream,
    /// Carga del frame como texto: la API entrega un mensaje por frame y el
    /// access log de Traefik es UTF-8.
    payload: String,
}

impl DecodedFrame {
    /// Línea que debe llegar al pipeline Jail, o `None` si este frame no
    /// alimenta el pipeline.
    ///
    /// El access log de Traefik sale por `stdout`: las cargas de `stderr` —y de
    /// `stdin`, que `logs` no emite— se descartan aquí, a nivel `debug`,
    /// nombrando el stream del que venían.
    fn into_pipeline_line(self) -> Option<String> {
        match self.stream {
            DockerStream::Stdout => Some(self.payload),
            DockerStream::Stderr | DockerStream::Stdin => {
                debug!(
                    "journald reader: discarding {} payload from the Docker log stream",
                    self.stream.name()
                );
                None
            },
        }
    }
}

/// Longitud de la cabecera de un frame de la API de Docker: un byte de
/// identificador de stream, tres de relleno y cuatro de longitud big-endian.
const DOCKER_FRAME_HEADER_LEN: usize = 8;

/// Cota superior de la carga que puede declarar la cabecera de un frame de la
/// API de Docker, en bytes.
///
/// La cabecera lleva la longitud como un `u32` —hasta ~4 GiB— y la fija el par
/// (el daemon o, en el modo B, un proxy de terceros), así que sin cota una
/// cabecera declarando gigabytes haría crecer el buffer del decodificador hasta
/// esa cifra a base de bytes reales: es un vector de agotamiento de memoria. 8
/// MiB es holgadísimo para una línea de access log de Traefik, que ronda 1 KiB;
/// lo que la excede no es un frame de este protocolo.
const MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

/// Decodificador incremental de los frames del stream multiplexado.
///
/// Guarda los bytes que todavía no forman un frame completo, de modo que el
/// corte se decide por la longitud de la cabecera y nunca por dónde caiga la
/// frontera entre dos lecturas del socket.
struct DockerFrameDecoder {
    /// Bytes leídos que todavía no completan un frame.
    buffer: Vec<u8>,
}

impl DockerFrameDecoder {
    /// Decodificador vacío.
    const fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Alimenta el decodificador con los bytes recién leídos y devuelve los
    /// frames que se hayan completado, en orden de llegada.
    ///
    /// Un frame con carga vacía (longitud `0`) se acepta y no entrega ninguna
    /// línea; la decodificación continúa con el frame siguiente.
    ///
    /// # Errors
    ///
    /// Devuelve [`JournalError::TtyStream`] si el primer byte de una cabecera
    /// no es un identificador de stream válido: el cuerpo llega crudo y quien
    /// llama SHALL descartar la conexión en vez de seguir decodificando.
    ///
    /// Devuelve [`JournalError::FrameTooLarge`] si la longitud declarada en una
    /// cabecera excede [`MAX_FRAME_LEN`]: quien llama SHALL descartar la
    /// conexión igual que con [`JournalError::TtyStream`], en vez de reservar
    /// memoria por lo que el par declare.
    fn push(&mut self, chunk: &[u8]) -> Result<Vec<DecodedFrame>, JournalError> {
        self.buffer.extend_from_slice(chunk);

        let mut frames = Vec::new();
        while !self.buffer.is_empty() {
            // El primer byte de la cabecera es el identificador de stream. Un
            // valor fuera de {0,1,2} significa que el cuerpo no lleva cabeceras
            // —un contenedor con TTY, cuyo stream llega crudo—: se diagnostica
            // en vez de gastar el buffer descodificando bytes que nunca serán
            // un frame.
            let stream_byte = self.buffer[0];
            let Some(stream) = DockerStream::from_byte(stream_byte) else {
                return Err(JournalError::TtyStream(stream_byte));
            };

            // Cabecera truncada: se queda en el buffer hasta que el resto de los
            // 8 bytes llegue en un chunk posterior.
            if self.buffer.len() < DOCKER_FRAME_HEADER_LEN {
                break;
            }
            let declared_len = u32::from_be_bytes([
                self.buffer[4],
                self.buffer[5],
                self.buffer[6],
                self.buffer[7],
            ]);

            // La longitud declarada la fija el par, así que lo que exceda la
            // cota se rechaza antes de reservar nada: sin cota, una cabecera
            // declarando gigabytes haría crecer el buffer hasta esa cifra con
            // bytes reales. `u32 -> usize` no puede fallar en las plataformas
            // soportadas, pero se resuelve con `try_from` para no depender de
            // ello: lo que no cupiera en `usize` es, por definición, mayor que
            // la cota.
            let payload_len = usize::try_from(declared_len)
                .ok()
                .filter(|len| *len <= MAX_FRAME_LEN)
                .ok_or(JournalError::FrameTooLarge {
                    declared: declared_len,
                    limit: MAX_FRAME_LEN,
                })?;

            // Carga truncada: lo mismo, se espera al resto. El corte se decide
            // por la longitud declarada, nunca por la frontera entre chunks.
            let Some(frame_len) = DOCKER_FRAME_HEADER_LEN.checked_add(payload_len) else {
                break;
            };
            if self.buffer.len() < frame_len {
                break;
            }

            let payload = String::from_utf8_lossy(&self.buffer[DOCKER_FRAME_HEADER_LEN..frame_len])
                .into_owned();
            self.buffer.drain(..frame_len);

            if payload.is_empty() {
                continue;
            }
            frames.push(DecodedFrame { stream, payload });
        }

        Ok(frames)
    }

    /// El stream se cerró: descarta el frame que quedara a medias.
    ///
    /// Devuelve cuántos bytes se han descartado (`0` si no quedaba ninguno).
    fn finish(&mut self) -> usize {
        std::mem::take(&mut self.buffer).len()
    }
}

/// Separa el instante que la API de Docker antepone a cada carga —cuando la
/// petición pide `timestamps=1`— de la línea del access log.
///
/// El prefijo es un instante `RFC3339Nano` seguido de un espacio
/// (`2026-10-21T08:31:07.123456789Z {…}`). Devuelve el instante en
/// microsegundos desde `UNIX_EPOCH` —la unidad del corte por intento de
/// [`now_micros`]— y la línea ya sin el prefijo, o `None` si la carga no trae
/// un prefijo utilizable: sin instante no se puede situar la carga después del
/// corte de la conexión en curso, así que se descarta.
fn split_timestamp_prefix(payload: &str) -> Option<(u64, &str)> {
    let (instant, line) = payload.split_once(' ')?;
    let parsed = chrono::DateTime::parse_from_rfc3339(instant).ok()?;
    let micros = u64::try_from(parsed.timestamp_micros()).ok()?;
    Some((micros, line))
}

/// Decide si una carga del backend de Docker cuenta en el intento de conexión
/// en curso.
///
/// `instant_micros` es el instante de la carga (`None` si no trae un prefijo
/// utilizable) y `cutoff_micros` el corte del intento, fijado cuando empezó.
/// Sólo cuenta lo **estrictamente posterior** al corte: una carga con el instante
/// exactamente igual al corte no es de este intento, y una sin instante no se
/// puede situar en el tiempo. En los dos casos se descarta —mejor perder una
/// carga que banear una IP por evidencia vieja—, y descartar una carga nunca
/// aborta el stream.
///
/// Es una función **pura** a propósito: la frontera se prueba con dos números,
/// sin reloj inyectado y sin servidor (tareas 3.5 (d) y (e) del change).
const fn payload_is_after_cutoff(instant_micros: Option<u64>, cutoff_micros: u64) -> bool {
    match instant_micros {
        Some(instant) => instant > cutoff_micros,
        None => false,
    }
}

/// Motor de decodificación del backend en curso.
///
/// Un único punto de decisión ([`StreamFraming::for_transport`]) elige entre el
/// NDJSON de `systemd-journal-gatewayd` y los frames multiplexados de la API de
/// Docker. El resto —conexión, petición, bucle de lectura, reconexión y
/// respuesta a la señal de parada— es común a los dos backends (tarea 5.3), de
/// modo que no hay un segundo `send_request` ni un segundo bucle de streaming.
enum StreamFraming {
    /// NDJSON de `systemd-journal-gatewayd`: líneas separadas por `\n`.
    Journald {
        /// Bytes leídos que todavía no completan una línea.
        buffer: Vec<u8>,
    },
    /// Frames multiplexados de la API de Docker.
    Docker {
        /// Decodificador incremental del stream multiplexado.
        decoder: DockerFrameDecoder,
    },
}

impl StreamFraming {
    /// Motor del backend que corresponde al transporte configurado.
    const fn for_transport(transport: &JournaldTransport) -> Self {
        match transport {
            JournaldTransport::DockerTcp { .. } | JournaldTransport::DockerUnixSocket { .. } => {
                Self::Docker {
                    decoder: DockerFrameDecoder::new(),
                }
            },
            JournaldTransport::Tcp { .. } | JournaldTransport::UnixSocket { .. } => {
                Self::Journald { buffer: Vec::new() }
            },
        }
    }

    /// El stream se cerró: descarta lo que quedara a medias y devuelve los bytes
    /// descartados.
    ///
    /// En NDJSON el fragmento final de una línea no es un frame y se tira con el
    /// buffer (`0`); en Docker, un frame partido por el cierre del stream se
    /// descarta aquí en vez de arrastrarlo a la siguiente conexión.
    fn finish(&mut self) -> usize {
        match self {
            Self::Journald { .. } => 0,
            Self::Docker { decoder } => decoder.finish(),
        }
    }
}

#[cfg(test)]
mod tests_docker_frames {
    use super::{
        DecodedFrame, DockerFrameDecoder, DockerStream, JournalError, parse_traefik_access_log,
        split_timestamp_prefix,
    };
    use std::sync::{Arc, Mutex};

    // ── fixtures ──

    /// Identificador de stream `stdout` en la cabecera de un frame.
    ///
    /// `pub(super)`: los tests del backend de Docker de `tests_task7` construyen
    /// frames con el mismo formato de la wire.
    pub(super) const STDOUT: u8 = 1;
    /// Identificador de stream `stderr` en la cabecera de un frame.
    const STDERR: u8 = 2;
    /// Tamaño de la cabecera de un frame: stream, relleno y longitud.
    const HEADER_LEN: usize = 8;

    /// Construye a mano un frame multiplexado: un byte de identificador de
    /// stream, tres de relleno y la longitud de la carga en big-endian.
    pub(super) fn docker_frame(stream: u8, payload: &str) -> Vec<u8> {
        let len = u32::try_from(payload.len()).expect("the fixture payload fits in u32");
        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.push(stream);
        frame.extend_from_slice(&[0_u8; 3]);
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(payload.as_bytes());
        frame
    }

    /// Línea del access log JSON de Traefik, con los campos que maneja el
    /// módulo.
    pub(super) fn access_log_line(status: u16, path: &str) -> String {
        serde_json::json!({
            "ClientHost": "10.0.0.1",
            "ClientAddr": "10.0.0.1:54321",
            "OriginStatus": status,
            "DownstreamStatus": status,
            "RequestPath": path,
            "RequestMethod": "GET",
            "RequestHost": "example.com",
            "RequestScheme": "https",
            "request_User-Agent": "Mozilla/5.0 (compatible; Nmap Scripting Engine)",
            "request_Referer": "https://example.com/login",
            "request_Content-Type": "application/x-www-form-urlencoded",
            "request_Accept-Language": "es-ES,es;q=0.9",
            "request_X-Request-Id": "req-7f3a1c2e-4b9d"
        })
        .to_string()
    }

    /// Alimenta el decodificador con un chunk, fallando en una aserción —y no
    /// en un pánico del decodificador— si el chunk no se decodifica.
    fn push_frames(decoder: &mut DockerFrameDecoder, chunk: &[u8]) -> Vec<DecodedFrame> {
        let result = decoder.push(chunk);
        assert!(
            result.is_ok(),
            "a well-formed chunk must decode without error, got: {result:?}"
        );
        result.unwrap_or_default()
    }

    /// Líneas que los frames de este chunk entregarían al pipeline Jail.
    fn pipeline_lines(decoder: &mut DockerFrameDecoder, chunk: &[u8]) -> Vec<String> {
        frames_to_lines(push_frames(decoder, chunk))
    }

    /// Líneas que estos frames entregarían al pipeline Jail, en orden.
    fn frames_to_lines(frames: Vec<DecodedFrame>) -> Vec<String> {
        frames
            .into_iter()
            .filter_map(DecodedFrame::into_pipeline_line)
            .collect()
    }

    // ── 2.1: decodificación incremental ──

    #[test]
    fn test_a_frame_split_between_two_chunks_is_decoded_exactly_once() {
        let line = access_log_line(404, "/wp-login.php");
        let frame = docker_frame(STDOUT, &line);
        // The cut falls in the middle of the payload, and neither half matches
        // the header (8 bytes) nor the payload length: this is where such
        // parsers break.
        let (head, tail) = frame.split_at(HEADER_LEN + line.len() / 2);
        assert!(
            !head.is_empty() && !tail.is_empty(),
            "the fixture must really split the frame in two"
        );

        let mut decoder = DockerFrameDecoder::new();
        let incomplete = push_frames(&mut decoder, head);
        assert!(
            incomplete.is_empty(),
            "half a frame is not a frame yet: the bytes must stay buffered"
        );
        let completed = push_frames(&mut decoder, tail);

        let lines = frames_to_lines(incomplete.into_iter().chain(completed).collect());
        assert_eq!(
            lines,
            vec![line],
            "the frame must be delivered once its payload completes, neither dropped nor \
             duplicated by the chunk boundary"
        );
    }

    #[test]
    fn test_two_frames_in_one_chunk_are_both_decoded() {
        let first = access_log_line(404, "/wp-login.php");
        let second = access_log_line(429, "/api/v1/items");
        let mut chunk = docker_frame(STDOUT, &first);
        chunk.extend(docker_frame(STDOUT, &second));

        let mut decoder = DockerFrameDecoder::new();
        let frames = push_frames(&mut decoder, &chunk);
        assert_eq!(
            frames.len(),
            2,
            "one chunk carries two complete frames and both must be decoded, in order"
        );

        let lines = frames_to_lines(frames);
        assert_eq!(
            lines,
            vec![first, second],
            "each frame delivers its own line, in the order they arrived"
        );

        let statuses: Vec<u16> = lines
            .iter()
            .map(|line| {
                parse_traefik_access_log(line)
                    .expect("a decoded line must be a Traefik access log")
                    .expect("a decoded line must be a backend response")
                    .status_code
            })
            .collect();
        assert_eq!(
            statuses,
            vec![404, 429],
            "each payload must reach the pipeline with its own status_code"
        );
    }

    #[test]
    fn test_a_frame_with_an_empty_payload_is_accepted_and_delivers_no_line() {
        let line = access_log_line(403, "/.env");
        let mut chunk = docker_frame(STDOUT, "");
        chunk.extend(docker_frame(STDOUT, &line));

        let mut decoder = DockerFrameDecoder::new();
        assert_eq!(
            pipeline_lines(&mut decoder, &chunk),
            vec![line],
            "a zero-length payload is accepted, delivers no line, and decoding continues with \
             the frame that follows"
        );
    }

    #[test]
    fn test_a_header_truncated_at_stream_close_is_discarded() {
        let frame = docker_frame(STDOUT, &access_log_line(404, "/admin"));
        let partial = &frame[..5];
        assert!(
            partial.len() < HEADER_LEN,
            "the fixture must leave less than a whole frame header"
        );

        let mut decoder = DockerFrameDecoder::new();
        let frames = push_frames(&mut decoder, partial);
        assert!(
            frames.is_empty(),
            "an incomplete header cannot be decoded while more bytes may still arrive"
        );
        assert_eq!(
            decoder.finish(),
            partial.len(),
            "a header cut short by the close of the stream must be dropped, not carried over to \
             the next connection"
        );
    }

    // ── 2.2: stdout frente a stderr ──

    #[test]
    fn test_stdout_and_stderr_are_told_apart_by_the_frame_header() {
        let stdout_line = access_log_line(401, "/api/login");
        // Traefik writes its own logs to stderr; they are not access logs.
        let stderr_line =
            r#"{"level":"error","msg":"no available server","time":"2026-10-21T08:31:07Z"}"#;
        let mut chunk = docker_frame(STDOUT, &stdout_line);
        chunk.extend(docker_frame(STDERR, stderr_line));

        let mut decoder = DockerFrameDecoder::new();
        let mut frames = push_frames(&mut decoder, &chunk);
        assert_eq!(
            frames.len(),
            2,
            "a stdout frame and a stderr frame must both be decoded, one per stream"
        );

        let stderr_frame = frames.pop().expect("two frames were decoded");
        let stdout_frame = frames.pop().expect("two frames were decoded");
        assert_eq!(
            stdout_frame.stream,
            DockerStream::Stdout,
            "stream id 1 in the header means stdout"
        );
        assert_eq!(
            stderr_frame.stream,
            DockerStream::Stderr,
            "stream id 2 in the header means stderr"
        );

        let logs = capture_debug(|| {
            assert!(
                stderr_frame.into_pipeline_line().is_none(),
                "a stderr payload must never reach the Jail pipeline"
            );
        });
        assert!(
            logs.contains("stderr"),
            "the discard must record which stream it dropped, at debug level; logs were: {logs:?}"
        );
        assert_eq!(
            stdout_frame.into_pipeline_line().as_deref(),
            Some(stdout_line.as_str()),
            "the stdout payload must be handed to the pipeline verbatim"
        );
    }

    // ── 2.3: detección de TTY ──

    #[test]
    fn test_a_raw_tty_stream_is_diagnosed_instead_of_decoded() {
        // With `tty: true` the container sends its stream raw: the first thing
        // that arrives is the message itself, and a JSON access log starts with
        // `{` (`0x7B`), which is never a valid stream id.
        let raw = access_log_line(404, "/wp-login.php");

        let mut decoder = DockerFrameDecoder::new();
        let result = decoder.push(raw.as_bytes());

        assert!(
            matches!(result.as_ref(), Err(JournalError::TtyStream(byte)) if *byte == b'{'),
            "the first byte of a raw stream must be diagnosed as a TTY stream instead of being \
             skipped in silence; got: {result:?}"
        );
        let message = result.err().map_or_else(String::new, |err| err.to_string());
        assert!(
            message.contains("TTY"),
            "the diagnosis must name the probable cause —a container with TTY enabled— so the \
             operator does not have to guess; got: {message:?}"
        );
    }

    // ── cota de la longitud declarada en la cabecera ──

    #[test]
    fn test_a_frame_declaring_more_than_the_limit_is_refused_without_buffering() {
        // La cabecera lleva la longitud de la carga como un `u32` (hasta ~4
        // GiB) y la fija el par —el daemon o, en el modo B, un proxy de
        // terceros—, que puede declararla sin enviar un solo byte de carga. Sin
        // cota, el buffer crecería hasta esa cifra con bytes reales: es un
        // vector de agotamiento de memoria. El decodificador SHALL rechazarlo
        // antes de reservar nada, nombrando la longitud declarada y la cota.
        const DECLARED: u32 = u32::MAX;
        // La cota, escrita como literal para que este test no dependa de un
        // símbolo que todavía no existe en el RED.
        const LIMIT: usize = 8 * 1024 * 1024;

        let mut header = vec![STDOUT, 0, 0, 0];
        header.extend_from_slice(&DECLARED.to_be_bytes());
        assert_eq!(
            header.len(),
            HEADER_LEN,
            "the fixture must be a bare header"
        );

        let mut decoder = DockerFrameDecoder::new();
        let err = decoder.push(&header).err().expect(
            "a frame declaring ~4 GiB must be refused, not parked waiting for a payload that \
             will never come",
        );

        let message = err.to_string();
        assert!(
            message.contains(&DECLARED.to_string()),
            "the refusal must name the declared length ({DECLARED}) so the peer can be told \
             apart; got: {message:?}"
        );
        assert!(
            message.contains(&LIMIT.to_string()),
            "the refusal must name the {LIMIT}-byte limit so the operator knows what to expect; \
             got: {message:?}"
        );
        assert!(
            std::error::Error::source(&err).is_none(),
            "the refusal is a context-only failure and must not carry a source"
        );
        assert!(
            decoder.buffer.capacity() <= LIMIT,
            "the refused frame must not be reserved for; capacity was {}",
            decoder.buffer.capacity()
        );
    }

    // ── 2.4: prefijo del instante ──

    #[test]
    fn test_the_payload_timestamp_prefix_is_stripped_before_parsing() {
        // With `timestamps=1` the API prefixes every line with the instant, in
        // RFC3339 with nanosecond precision.
        let instant = "2026-10-21T08:31:07.123456789Z";
        let line = access_log_line(401, "/api/login");
        let payload = format!("{instant} {line}");

        assert!(
            parse_traefik_access_log(&payload).is_err(),
            "the prefixed payload is not JSON: splitting the prefix off is what makes it parseable"
        );

        let split = split_timestamp_prefix(&payload);
        assert!(
            split.is_some(),
            "the payload must split into its timestamp prefix and the access log line; \
             got: {split:?}"
        );
        // `unwrap_or` instead of `expect`: this test must fail on the assertion
        // above, never on a panic.
        let (instant_micros, stripped) = split.unwrap_or((0, ""));

        assert_eq!(
            stripped, line,
            "the access log line must come out of the payload without the timestamp prefix"
        );
        assert_eq!(
            instant_micros,
            1_792_571_467_123_456, // 2026-10-21T08:31:07.123456789Z
            "the instant must travel in microseconds since UNIX_EPOCH, the unit the attempt \
             cutoff uses"
        );

        let with_prefix = parse_traefik_access_log(stripped)
            .expect("the stripped line must parse")
            .expect("the stripped line must be a backend response");
        let without_prefix = parse_traefik_access_log(&line)
            .expect("the plain line must parse")
            .expect("the plain line must be a backend response");
        // `ReportPayload` does not derive `PartialEq` (it is a production
        // model), so the identity is checked field by field through its `Debug`.
        assert_eq!(
            format!("{with_prefix:?}"),
            format!("{without_prefix:?}"),
            "the payload must be identical with and without the timestamp prefix"
        );
    }

    // ── log capture helper ──

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

    /// Runs `f` with a DEBUG-level subscriber that records every event, and
    /// returns what was written. The discard of a `stderr` payload is only
    /// observable that way.
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
}

#[cfg(test)]
mod tests_docker_transport {
    use super::{
        DEFAULT_RETRY_DELAY, JournaldReader, JournaldTransport, payload_is_after_cutoff,
        split_timestamp_prefix,
    };
    use http::{HeaderMap, header};
    use std::path::PathBuf;

    // ── fixtures ──

    /// Socket del motor de Docker.
    const DOCKER_SOCKET: &str = "/var/run/docker.sock";
    /// URL base de un proxy de la API de Docker.
    const DOCKER_PROXY: &str = "http://docker-socket-proxy:2375";
    /// Corte de un intento de conexión. Lleva una parte sub-segundo a propósito:
    /// el `since` de la API tiene granularidad de segundo y tiene que truncarla.
    ///
    /// `pub(super)`: el test del path base ignorado de `tests_task8` construye
    /// la petición con este mismo corte para asertar su forma.
    pub(super) const CUTOFF_MICROS: u64 = 1_792_571_467_123_456;

    /// Reader con el backend de Docker sobre socket Unix.
    ///
    /// Se construye a mano para **no depender del entorno** —las variables de
    /// proceso— al probar la petición y el endpoint: `from_values` sí resuelve
    /// ya la precedencia de Docker, pero pasar por él obligaría a manipular el
    /// entorno y a `#[serial]`.
    pub(super) fn docker_unix_reader(path: &str, container: &str) -> JournaldReader {
        JournaldReader {
            transport: JournaldTransport::DockerUnixSocket {
                path: PathBuf::from(path),
            },
            container_name: container.to_string(),
            retry_delay: DEFAULT_RETRY_DELAY,
        }
    }

    /// Reader con el backend de Docker sobre TCP.
    pub(super) fn docker_tcp_reader(base_url: &str, container: &str) -> JournaldReader {
        JournaldReader {
            transport: JournaldTransport::DockerTcp {
                base_url: base_url.to_string(),
            },
            container_name: container.to_string(),
            retry_delay: DEFAULT_RETRY_DELAY,
        }
    }

    /// Target y cabeceras de la petición que este reader enviaría, tomados del
    /// **mismo** constructor que usa el camino real —`build_request`, el gate
    /// único de la tarea 5.3—, sin levantar ningún servidor.
    ///
    /// Si la petición no se puede construir, la aserción de abajo falla el test
    /// y el valor vacío que devuelve el `else` no llega a usarse: el test no
    /// puede caer en un pánico del código de producción.
    fn request_parts(reader: &JournaldReader, cutoff_micros: u64) -> (String, HeaderMap) {
        let request = reader.build_request(None, cutoff_micros);
        assert!(request.is_ok(), "the request must build, got: {request:?}");
        let Ok(request) = request else {
            return (String::new(), HeaderMap::new());
        };
        (request.uri().to_string(), request.headers().clone())
    }

    /// Valor de texto de una cabecera, o `None` si no viene o no es ASCII.
    fn header_value<'a>(headers: &'a HeaderMap, name: header::HeaderName) -> Option<&'a str> {
        headers.get(name).and_then(|value| value.to_str().ok())
    }

    // ── 3.1: precedencia de los cinco niveles ──

    #[test]
    fn test_docker_socket_alone_selects_the_docker_unix_transport() {
        let reader =
            JournaldReader::from_values(Some(DOCKER_SOCKET.to_string()), None, None, None, None);
        assert_eq!(
            reader.transport,
            JournaldTransport::DockerUnixSocket {
                path: PathBuf::from(DOCKER_SOCKET)
            },
            "SHUUL_DOCKER_SOCKET must select the Docker API over its Unix socket"
        );
        assert_eq!(reader.container_name, "traefik");
    }

    #[test]
    fn test_docker_url_alone_selects_the_docker_tcp_transport() {
        let reader =
            JournaldReader::from_values(None, Some(DOCKER_PROXY.to_string()), None, None, None);
        assert_eq!(
            reader.transport,
            JournaldTransport::DockerTcp {
                base_url: DOCKER_PROXY.to_string()
            },
            "SHUUL_DOCKER_URL must select the Docker API over TCP"
        );
    }

    #[test]
    fn test_docker_socket_wins_over_the_docker_url() {
        let reader = JournaldReader::from_values(
            Some(DOCKER_SOCKET.to_string()),
            Some(DOCKER_PROXY.to_string()),
            None,
            None,
            None,
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::DockerUnixSocket {
                path: PathBuf::from(DOCKER_SOCKET)
            },
            "with both Docker variables set the socket wins"
        );
    }

    #[test]
    fn test_docker_variables_win_over_every_journald_variable() {
        let docker_socket = Some(DOCKER_SOCKET.to_string());
        let docker_url = Some(DOCKER_PROXY.to_string());
        let journald_socket = Some("/run/systemd/journal/gatewayd.sock".to_string());
        let journald_url = Some("http://remote:19531".to_string());

        // The four transport variables set: Docker's socket must win.
        // `docker_socket` no se vuelve a usar en este test, así que se mueve en
        // vez de clonarse (clippy::redundant_clone está en `deny`).
        let reader = JournaldReader::from_values(
            docker_socket,
            docker_url.clone(),
            journald_socket.clone(),
            journald_url.clone(),
            None,
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::DockerUnixSocket {
                path: PathBuf::from(DOCKER_SOCKET)
            },
            "the highest priority backend must win, with the others never reaching the transport"
        );

        // Without the Docker socket, Docker's URL must win.
        let reader = JournaldReader::from_values(
            None,
            docker_url,
            journald_socket.clone(),
            journald_url.clone(),
            None,
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::DockerTcp {
                base_url: DOCKER_PROXY.to_string()
            },
            "the Docker URL goes before both journald variables"
        );

        // Without both Docker variables, journald's socket must win.
        let reader =
            JournaldReader::from_values(None, None, journald_socket, journald_url.clone(), None);
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: PathBuf::from("/run/systemd/journal/gatewayd.sock")
            },
            "removing the Docker backend must fall back to the journald socket, not to the URL"
        );

        // And with only the journald URL left, that is the transport.
        let reader = JournaldReader::from_values(None, None, None, journald_url, None);
        assert_eq!(
            reader.transport,
            JournaldTransport::Tcp {
                base_url: "http://remote:19531".to_string()
            },
            "the last level of the precedence is the journald URL"
        );
    }

    #[test]
    fn test_blank_docker_variable_counts_as_undefined() {
        let reader = JournaldReader::from_values(
            Some("   ".to_string()),
            Some(String::new()),
            Some("/run/systemd/journal/gatewayd.sock".to_string()),
            None,
            None,
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: PathBuf::from("/run/systemd/journal/gatewayd.sock")
            },
            "a blank Docker variable must not activate the Docker backend"
        );
    }

    // ── 3.2: la petición del backend de Docker ──

    #[test]
    fn test_docker_request_is_the_engine_log_endpoint() {
        let reader = docker_unix_reader(DOCKER_SOCKET, "traefik");
        let (target, headers) = request_parts(&reader, CUTOFF_MICROS);

        assert_eq!(
            target,
            "/v1.44/containers/traefik/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1\
             &since=1792571467",
            "the Docker request must be the engine log endpoint, with the attempt cutoff in \
             seconds as `since`"
        );
        assert_eq!(
            header_value(&headers, header::ACCEPT),
            Some("application/json"),
            "the engine serves the stream as JSON lines"
        );
        assert_eq!(
            header_value(&headers, header::HOST),
            Some("docker"),
            "over a Unix socket there is no real authority and Docker's own convention is `docker`"
        );
        assert!(
            headers.get(header::UPGRADE).is_none(),
            "asking for Upgrade makes the engine answer 101 UPGRADED instead of serving the stream"
        );
        assert!(
            headers.get(header::CONNECTION).is_none(),
            "no `Connection: Upgrade` either: the stream is a plain 200 OK body"
        );
    }

    #[test]
    fn test_docker_request_is_identical_in_both_transports_but_for_the_host() {
        let unix = docker_unix_reader(DOCKER_SOCKET, "traefik");
        let tcp = docker_tcp_reader(DOCKER_PROXY, "traefik");

        let (unix_target, _) = request_parts(&unix, CUTOFF_MICROS);
        let (tcp_target, tcp_headers) = request_parts(&tcp, CUTOFF_MICROS);

        assert_eq!(
            tcp_target, unix_target,
            "the path and the query must be identical in both transports"
        );
        assert_eq!(
            header_value(&tcp_headers, header::HOST),
            Some("docker-socket-proxy:2375"),
            "in TCP the Host header is the authority of the configured endpoint"
        );
    }

    #[test]
    fn test_docker_request_url_encodes_the_container_name() {
        let reader = docker_unix_reader(DOCKER_SOCKET, "my traefik");
        let (target, _) = request_parts(&reader, CUTOFF_MICROS);

        assert!(
            target.starts_with("/v1.44/containers/my%20traefik/logs?"),
            "the container travels in the path segment, URL-encoded, so the lines need no \
             CONTAINER_NAME field; got: {target}"
        );
    }

    // ── 3.5 (d) y (e): la frontera del corte, con dos números y sin servidor ──

    #[test]
    fn test_a_payload_at_the_attempt_cutoff_is_discarded() {
        assert!(
            !payload_is_after_cutoff(Some(CUTOFF_MICROS), CUTOFF_MICROS),
            "a payload dated exactly at the attempt cutoff is not strictly after it"
        );
        assert!(
            !payload_is_after_cutoff(Some(CUTOFF_MICROS - 1), CUTOFF_MICROS),
            "a payload dated before the attempt cutoff is history, not this attempt"
        );
        assert!(
            payload_is_after_cutoff(Some(CUTOFF_MICROS + 1), CUTOFF_MICROS),
            "one microsecond after the cutoff is the first payload of this attempt: the \
             comparison is strictly greater, not greater-or-equal"
        );
    }

    #[test]
    fn test_a_payload_without_a_usable_timestamp_is_discarded_without_aborting() {
        // Neither a payload without the prefix nor one whose prefix is not an
        // instant can be placed in time.
        for payload in [
            r#"{"ClientHost":"10.0.0.1","OriginStatus":401}"#,
            r#"not-an-instant {"ClientHost":"10.0.0.1","OriginStatus":401}"#,
        ] {
            let instant = split_timestamp_prefix(payload).map(|(instant, _)| instant);
            assert!(
                instant.is_none(),
                "the payload {payload:?} has no usable instant prefix to split off"
            );
            assert!(
                !payload_is_after_cutoff(instant, CUTOFF_MICROS),
                "a payload that cannot be placed in time must be discarded, not counted"
            );
        }

        assert!(
            payload_is_after_cutoff(Some(CUTOFF_MICROS + 1), CUTOFF_MICROS),
            "the well-formed payload that follows must still be processed: discarding one \
             payload never aborts the stream"
        );
    }

    // ── 3.3: endpoint_display ──

    #[test]
    fn test_docker_endpoint_display_names_the_backend_and_the_container() {
        assert_eq!(
            docker_unix_reader(DOCKER_SOCKET, "traefik").endpoint_display(),
            "docker:unix:/var/run/docker.sock (traefik)"
        );
        assert_eq!(
            docker_tcp_reader(DOCKER_PROXY, "traefik").endpoint_display(),
            "docker:http://docker-socket-proxy:2375 (traefik)"
        );
    }
}

// ── Task 7: JournaldReader::run ──

/// Extrae del buffer las líneas NDJSON completas, dejando en él sólo el
/// fragmento incompleto final.
///
/// Trabaja sobre bytes para que el corte se haga siempre en `\n`, que nunca
/// forma parte de un carácter UTF-8 multibyte.
fn drain_ndjson_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
        let drained: Vec<u8> = buffer.drain(..=pos).collect();
        let line = String::from_utf8_lossy(&drained[..drained.len() - 1]);
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines
}

#[cfg(test)]
mod tests_task7 {
    use super::{
        CONNECT_TIMEOUT, JournalError, JournaldReader, drain_ndjson_lines, now_micros, tcp_endpoint,
    };
    use crate::models::log_collector::{LOG_COLLECTOR, TEST_LOCK};
    use crate::models::{AppState, BanManager, GeoIpService, Settings, StatsCollector, TorService};
    use http::header;
    use maxminddb::Reader;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

    // ── drain_ndjson_lines ──

    #[test]
    fn test_drain_no_newline_keeps_buffer() {
        let mut buf = b"{\"partial\":".to_vec();
        let lines = drain_ndjson_lines(&mut buf);
        assert!(lines.is_empty(), "an incomplete line must not be drained");
        assert_eq!(buf, b"{\"partial\":", "the partial line stays buffered");
    }

    #[test]
    fn test_drain_complete_line_and_keeps_remainder() {
        let mut buf = b"{\"a\":1}\n{\"partial\"".to_vec();
        let lines = drain_ndjson_lines(&mut buf);
        assert_eq!(lines, vec!["{\"a\":1}".to_string()]);
        assert_eq!(buf, b"{\"partial\"", "remainder must stay buffered");
    }

    #[test]
    fn test_drain_multiple_lines() {
        let mut buf = b"{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n".to_vec();
        let lines = drain_ndjson_lines(&mut buf);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2], "{\"c\":3}");
        assert!(buf.is_empty(), "fully consumed buffer must be empty");
    }

    #[test]
    fn test_drain_skips_blank_lines_and_handles_crlf() {
        let mut buf = b"{\"a\":1}\r\n\n   \n{\"b\":2}\n".to_vec();
        let lines = drain_ndjson_lines(&mut buf);
        assert_eq!(
            lines,
            vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()]
        );
    }

    // ── request_target / endpoint_display ──

    #[test]
    fn test_request_target_default() {
        let reader = JournaldReader::default();
        assert_eq!(
            reader.request_target(),
            "/entries?follow&CONTAINER_NAME=traefik"
        );
    }

    #[test]
    fn test_request_target_encodes_container_name() {
        let reader = JournaldReader::new("http://gateway.local:19531", "my traefik");
        assert_eq!(
            reader.request_target(),
            "/entries?follow&CONTAINER_NAME=my%20traefik",
            "the container name must be URL-encoded"
        );

        let reader = JournaldReader::unix("/run/gw.sock", "a&b=c");
        assert_eq!(
            reader.request_target(),
            "/entries?follow&CONTAINER_NAME=a%26b%3Dc",
            "reserved characters in the container name must be escaped"
        );
    }

    #[test]
    fn test_request_target_is_identical_in_both_transports() {
        let tcp = JournaldReader::new("http://gateway.local:19531/base", "traefik");
        let unix = JournaldReader::unix("/run/systemd/journal/gatewayd.sock", "traefik");
        assert_eq!(
            tcp.request_target(),
            unix.request_target(),
            "both transports must send the very same origin-form request target"
        );
        assert_eq!(
            tcp.request_target(),
            "/entries?follow&CONTAINER_NAME=traefik"
        );
    }

    #[test]
    fn test_endpoint_display_tcp() {
        assert_eq!(
            JournaldReader::default().endpoint_display(),
            "http://localhost:19531"
        );
        assert_eq!(
            JournaldReader::new("http://gateway.local:19531/", "traefik").endpoint_display(),
            "http://gateway.local:19531",
            "a trailing slash must not leak into the log output"
        );
    }

    #[test]
    fn test_endpoint_display_unix() {
        assert_eq!(
            JournaldReader::unix("/run/systemd/journal/gatewayd.sock", "traefik")
                .endpoint_display(),
            "unix:/run/systemd/journal/gatewayd.sock"
        );
    }

    #[test]
    fn test_endpoint_display_tcp_shows_the_effective_endpoint_only() {
        let reader = JournaldReader::new("http://proxy:19531/journald", "traefik");
        assert_eq!(
            reader.endpoint_display(),
            "http://proxy:19531",
            "the base path is ignored when building the request, so it must not appear in logs"
        );
    }

    #[test]
    fn test_tcp_endpoint_handles_ipv6_literals() {
        let endpoint = tcp_endpoint("http://[::1]:19531").expect("IPv6 URL must parse");
        assert_eq!(
            endpoint.connect_host, "::1",
            "TcpStream::connect needs the literal without brackets"
        );
        assert_eq!(
            endpoint.authority(),
            "[::1]:19531",
            "the Host header must keep the brackets"
        );

        let reader = JournaldReader::new("http://[::1]:19531", "traefik");
        assert_eq!(reader.endpoint_display(), "http://[::1]:19531");

        let request = reader
            .build_request(None, now_micros())
            .expect("request must build");
        assert_eq!(
            request.headers()[header::HOST],
            "[::1]:19531",
            "RFC 9110 requires bracketed IPv6 in the Host header"
        );
        assert_eq!(
            request.uri().to_string(),
            "/entries?follow&CONTAINER_NAME=traefik",
            "the target stays in origin form"
        );
    }

    #[test]
    fn test_tcp_endpoint_hints_the_socket_variable_for_an_absolute_path() {
        let err = tcp_endpoint("/run/systemd/journal/gatewayd.sock")
            .err()
            .expect("an absolute path without a scheme must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("/run/systemd/journal/gatewayd.sock"),
            "the rejected value must appear in the diagnosis, got: {message}"
        );
        assert!(
            message.contains("SHUUL_JOURNALD_SOCKET"),
            "an absolute path belongs in the socket variable and the diagnosis must say so, got: {message}"
        );
    }

    #[test]
    fn test_tcp_endpoint_hints_the_socket_variable_for_an_unparsable_path() {
        let err = tcp_endpoint("/run/systemd/journal/my gateway.sock")
            .err()
            .expect("a path with a space cannot be a valid URI");
        let message = err.to_string();
        assert!(
            message.contains("/run/systemd/journal/my gateway.sock"),
            "the rejected value must appear in the diagnosis, got: {message}"
        );
        assert!(
            message.contains("SHUUL_JOURNALD_SOCKET"),
            "an absolute path belongs in the socket variable even when it does not parse as a URI, got: {message}"
        );
    }

    #[test]
    fn test_tcp_endpoint_does_not_hint_for_a_scheme_less_host_port() {
        let err = tcp_endpoint("localhost:19531")
            .err()
            .expect("a scheme-less host:port must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("has no scheme"),
            "the diagnosis must point at the missing scheme, got: {message}"
        );
        assert!(
            !message.contains("SHUUL_JOURNALD_SOCKET"),
            "a host:port is not a socket path, so the hint would be misleading, got: {message}"
        );
    }

    // ── run() integration ──

    /// Minimal `AppState` with **no rules**, so every report ends up as a
    /// `cleared` audit log — enough to prove the entry reached the pipeline.
    async fn build_test_app_state() -> Arc<AppState> {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("in-memory SQLite pool");

        let geoip = ["../geo/GeoLite2-City.mmdb", "geo/GeoLite2-City.mmdb"]
            .iter()
            .find_map(|p| Reader::open_readfile(p).ok())
            .map(GeoIpService::new)
            .expect("a GeoIP database is required for these tests");

        Arc::new(AppState {
            pool,
            secret: "test-secret".to_string(),
            geoip,
            tor_service: TorService::new(),
            rules: Mutex::new(Vec::new()),
            stats: StatsCollector::new(),
            static_dir: "static".to_string(),
            ban_manager: Mutex::new(BanManager::new(3600, false, vec![1], 86400, 7)),
            rate_limiter: Mutex::new(HashMap::new()),
            settings: Mutex::new(Settings {
                default_rule_mode: "enforce".to_string(),
                log_retention_days: 30,
                log_all_requests: "all".to_string(),
            }),
            pending_bans: Mutex::new(Vec::new()),
            oidc_metadata: tokio::sync::RwLock::new(None),
            jwt_validator: tokio::sync::RwLock::new(None),
            oidc_states: tokio::sync::Mutex::new(HashMap::new()),
            oidc_client_id: None,
            oidc_redirect_url: None,
        })
    }

    /// A temporary `AF_UNIX` socket path that is removed when dropped.
    struct TempSocketPath(PathBuf);

    impl TempSocketPath {
        fn new() -> Self {
            // `AF_UNIX` limita el path a ~108 bytes y `TMPDIR` puede ser
            // larga, así que se prefiere `/tmp` con un nombre de 12 hex.
            let dir = if Path::new("/tmp").is_dir() {
                PathBuf::from("/tmp")
            } else {
                std::env::temp_dir()
            };
            let id = uuid::Uuid::new_v4().simple().to_string();
            Self(dir.join(format!("shuul-{}.sock", &id[..12])))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempSocketPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Serves one HTTP response with the given NDJSON body over an `AF_UNIX`
    /// socket, then closes.
    async fn spawn_fake_unix_gateway(socket_path: &Path, body: String) {
        let listener = tokio::net::UnixListener::bind(socket_path).expect("bind fake unix gateway");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0_u8; 4096];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });
    }

    /// Reads an HTTP request head (up to the blank line) in a loop: a header
    /// split across TCP segments would otherwise make assertions flaky.
    async fn read_request_head(stream: &mut (impl AsyncRead + Unpin)) -> String {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 512];
        loop {
            match stream.read(&mut chunk).await {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                    // A well-formed head is far smaller; don't let a broken
                    // peer grow this buffer without bound.
                    if buffer.len() > 8 * 1024 {
                        break;
                    }
                },
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// Serves one HTTP response per connection over an `AF_UNIX` socket,
    /// recording the request head received on each of them. Each response is a
    /// `(status, body)` pair, so error statuses can be exercised too.
    async fn spawn_recording_unix_gateway(
        socket_path: &Path,
        responses: Vec<(u16, String)>,
    ) -> Arc<Mutex<Vec<String>>> {
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("bind recording unix gateway");
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);

        tokio::spawn(async move {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = read_request_head(&mut stream).await;
                if let Ok(mut sink) = sink.lock() {
                    sink.push(request);
                }
                let response = format!(
                    "HTTP/1.1 {status} Gateway\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        recorded
    }

    /// Replays the same entry on every connection unless the request carries
    /// `Range: entries=<cursor>:1`, in which case it behaves like gatewayd and
    /// skips it. That way a reader that resumed without the right cursor would
    /// get the entry back and the test would see it twice.
    async fn spawn_cursor_aware_unix_gateway(
        socket_path: &Path,
        line: String,
        cursor: &'static str,
    ) -> Arc<Mutex<Vec<String>>> {
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("bind cursor-aware unix gateway");
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);

        tokio::spawn(async move {
            let expected = format!("range: entries={}:1", cursor.to_lowercase());
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = read_request_head(&mut stream).await;
                let resumes = request.to_lowercase().contains(&expected);
                if let Ok(mut sink) = sink.lock() {
                    sink.push(request);
                }

                let body = if resumes {
                    String::new()
                } else {
                    format!("{line}\n")
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        recorded
    }

    /// Serves a single response over `AF_UNIX`, writing the body in several
    /// chunks with a pause in between so line splits happen mid-body.
    async fn spawn_chunked_unix_gateway(socket_path: &Path, chunks: Vec<String>) {
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("bind chunked unix gateway");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0_u8; 4096];
                let _ = stream.read(&mut request).await;

                let total: usize = chunks.iter().map(String::len).sum();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.flush().await;

                for chunk in chunks {
                    let _ = stream.write_all(chunk.as_bytes()).await;
                    let _ = stream.flush().await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        });
    }

    /// Leads the reader's cutoff by an hour: fixtures are built *before* the
    /// connection attempt starts, so a live entry must be dated in the future
    /// to pass a cutoff fixed later.
    const LIVE_SKEW_MICROS: u64 = 60 * 60 * 1_000_000;

    /// A 2024 timestamp, far older than any cutoff these tests can fix.
    const OLD_TIMESTAMP_MICROS: u64 = 1_723_897_600_123_456;

    /// Timestamp that survives the cutoff of any attempt made during the test.
    fn live_timestamp() -> u64 {
        now_micros() + LIVE_SKEW_MICROS
    }

    /// Builds a journal NDJSON line that carries a Traefik access log, with an
    /// explicit `__REALTIME_TIMESTAMP` (`None` leaves the field out).
    fn journal_line_with_timestamp(
        cursor: Option<&str>,
        status: u16,
        path: &str,
        timestamp: Option<&str>,
    ) -> String {
        let message = serde_json::json!({
            "ClientHost": "10.0.0.1",
            "OriginStatus": status,
            "RequestPath": path,
            "RequestMethod": "GET",
            "RequestHost": "example.com",
        })
        .to_string();

        let mut entry = serde_json::Map::new();
        if let Some(cursor) = cursor {
            entry.insert("__CURSOR".to_string(), serde_json::json!(cursor));
        }
        if let Some(timestamp) = timestamp {
            entry.insert(
                "__REALTIME_TIMESTAMP".to_string(),
                serde_json::json!(timestamp),
            );
        }
        entry.insert("CONTAINER_NAME".to_string(), serde_json::json!("traefik"));
        entry.insert("MESSAGE".to_string(), serde_json::json!(message));
        serde_json::Value::Object(entry).to_string()
    }

    /// Builds a journal NDJSON line dated `timestamp_micros`.
    fn journal_line_at(
        cursor: Option<&str>,
        status: u16,
        path: &str,
        timestamp_micros: u64,
    ) -> String {
        let timestamp = timestamp_micros.to_string();
        journal_line_with_timestamp(cursor, status, path, Some(&timestamp))
    }

    /// Builds a journal NDJSON line dated far enough in the future to pass the
    /// cutoff of any connection attempt the reader makes during the test.
    fn journal_line(cursor: Option<&str>, status: u16, path: &str) -> String {
        journal_line_at(cursor, status, path, live_timestamp())
    }

    /// Waits until the Jail pipeline has logged an entry with this status and
    /// path, up to ~2 s.
    async fn wait_for_pipeline_entry(status: i32, path: &str) -> bool {
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();
            if entries
                .iter()
                .any(|e| e.status_code == Some(status) && e.path.as_deref() == Some(path))
            {
                return true;
            }
        }
        false
    }

    fn pipeline_entry_count(status: i32, path: &str) -> usize {
        LOG_COLLECTOR
            .lock()
            .map(|c| {
                c.all()
                    .iter()
                    .filter(|e| e.status_code == Some(status) && e.path.as_deref() == Some(path))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Serves one HTTP response with the given NDJSON body, then closes.
    async fn spawn_fake_gateway(body: String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake gateway");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0_u8; 4096];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        format!("http://{addr}")
    }

    #[tokio::test]
    async fn test_run_streams_entry_into_jail_pipeline() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let body = format!("{}\n", journal_line(None, 404, "/api/login"));
        let gateway_url = spawn_fake_gateway(body).await;

        let reader = JournaldReader::new(&gateway_url, "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn({
            let app_state = Arc::clone(&app_state);
            async move { reader.run(app_state, shutdown_rx).await }
        });

        // Wait for the entry to be processed (or give up after ~2s). The path
        // keeps this from matching the `cleared` entries other tests generate.
        let found = wait_for_pipeline_entry(404, "/api/login").await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(found, "the journal entry must reach the Jail pipeline");
        assert!(
            result.is_ok(),
            "run() should return Ok on shutdown: {result:?}"
        );
    }

    /// Serves one response per connection, recording the raw HTTP request
    /// received on each of them.
    async fn spawn_recording_gateway(bodies: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind recording gateway");
        let addr = listener.local_addr().expect("local addr");
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);

        tokio::spawn(async move {
            for body in bodies {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = read_request_head(&mut stream).await;
                if let Ok(mut sink) = sink.lock() {
                    sink.push(request);
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        (format!("http://{addr}"), recorded)
    }

    #[tokio::test]
    async fn test_run_resumes_from_last_cursor_after_reconnect() {
        const CURSOR: &str = "s=abc;i=42;b=def";

        let body = format!("{}\n", journal_line(Some(CURSOR), 404, "/x"));
        let (gateway_url, recorded) = spawn_recording_gateway(vec![body, String::new()]).await;

        let reader = JournaldReader::new(&gateway_url, "traefik")
            .with_retry_delay(tokio::time::Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        // Wait until the reader has reconnected (second request recorded)
        let mut second_request = None;
        for _ in 0..60 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let requests = recorded.lock().map(|r| r.clone()).unwrap_or_default();
            if requests.len() >= 2 {
                second_request = Some(requests[1].clone());
                break;
            }
        }

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");
        assert!(result.is_ok(), "run() must return Ok on shutdown");

        let second_request =
            second_request.expect("the reader must reconnect after the stream closes");
        let lower = second_request.to_lowercase();
        assert!(
            lower.contains("range:")
                && lower.contains(&format!("entries={}:1", CURSOR.to_lowercase())),
            "the reconnect request must resume from the last cursor; got:\n{second_request}"
        );
    }

    // ── Docker backend: servidor de prueba y fixtures ──
    //
    // El servidor de la API de Docker **reutiliza** lo que ya tienen los tests de
    // gatewayd: `read_request_head` para leer la petición, el patrón de "una
    // respuesta por conexión con la petición registrada", `TempSocketPath` para el
    // socket temporal y `BoxedIo` —el mismo tipo borrado que usa el reader— para
    // servir los dos transportes con **un solo** bucle de aceptación.

    use super::tests_docker_frames::{STDOUT, access_log_line, docker_frame};
    use super::tests_docker_transport::{docker_tcp_reader, docker_unix_reader};
    use super::tests_task8::capture_logs_while_running;
    use super::{BoxedIo, MICROS_PER_SECOND};
    use tokio::io::AsyncWrite;

    /// Servidor de prueba de la API de Docker sobre cualquiera de los dos
    /// transportes del change.
    enum DockerTestListener {
        /// Socket `AF_UNIX` del sistema de ficheros.
        Unix(tokio::net::UnixListener),
        /// TCP en un puerto efímero de `127.0.0.1`.
        Tcp(tokio::net::TcpListener),
    }

    impl DockerTestListener {
        /// Acepta la siguiente conexión del transporte que sea, ya borrada de
        /// tipo: `AF_UNIX` y TCP no comparten struct, pero sí los traits de I/O.
        async fn accept_stream(&self) -> Option<BoxedIo> {
            match self {
                Self::Unix(listener) => listener
                    .accept()
                    .await
                    .ok()
                    .map(|(stream, _)| Box::new(stream) as BoxedIo),
                Self::Tcp(listener) => listener
                    .accept()
                    .await
                    .ok()
                    .map(|(stream, _)| Box::new(stream) as BoxedIo),
            }
        }
    }

    /// Atiende `connections` conexiones: registra la cabecera de cada petición y
    /// responde lo que devuelva `respond`, que recibe la petición recién leída y
    /// el índice de la conexión. Así el fixture se fecha con el `since` que el
    /// reader acaba de mandar, en vez de con un instante inventado.
    ///
    /// `respond` devuelve `(status, chunks)`: un intento fallido se sirve con un
    /// status de error y sin cuerpo.
    fn spawn_docker_api<R>(
        listener: DockerTestListener,
        connections: usize,
        respond: R,
    ) -> Arc<Mutex<Vec<String>>>
    where
        R: Fn(&str, usize) -> (u16, Vec<Vec<u8>>) + Send + 'static,
    {
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);

        tokio::spawn(async move {
            for index in 0..connections {
                let Some(mut stream) = listener.accept_stream().await else {
                    return;
                };
                let request = read_request_head(&mut stream).await;
                if let Ok(mut sink) = sink.lock() {
                    sink.push(request.clone());
                }
                let (status, chunks) = respond(&request, index);
                write_docker_stream(&mut stream, status, &chunks).await;
            }
        });

        recorded
    }

    /// Escribe la respuesta de la API de Docker: el status pedido con el
    /// content-type del stream multiplexado y el cuerpo chunk a chunk, con una
    /// pausa entre ellos, para que las fronteras de chunk no coincidan con las de
    /// los frames.
    async fn write_docker_stream(
        stream: &mut (impl AsyncWrite + Unpin),
        status: u16,
        chunks: &[Vec<u8>],
    ) {
        let total: usize = chunks.iter().map(Vec::len).sum();
        let header = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/vnd.docker.raw-stream\r\n\
             Content-Length: {total}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(header.as_bytes()).await;
        let _ = stream.flush().await;

        for chunk in chunks {
            let _ = stream.write_all(chunk).await;
            let _ = stream.flush().await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Valor de `since` (segundos desde `UNIX_EPOCH`) de la petición que acaba de
    /// recibir el servidor: es el corte del intento en curso, visto desde fuera.
    ///
    /// La query se lee del **request-target**, que es el segundo token de la
    /// línea de petición (`METHOD target HTTP/x.y`), no de la línea entera: en la
    /// línea completa el último par sería `since=<epoch> HTTP/1.1` y no parsearía
    /// como entero. Se mira el segundo token por espacios a propósito, porque es
    /// donde el protocolo sitúa la query.
    fn since_from_request(request: &str) -> u64 {
        let query = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|target| target.split_once('?').map(|(_, query)| query))
            .unwrap_or("");
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix("since="))
            .and_then(|value| value.parse::<u64>().ok())
            .expect("the Docker request must carry since=<epoch>")
    }

    /// Carga de un frame de la API de Docker —`<instante RFC3339Nano> <línea>`—
    /// con el instante `offset_seconds` segundos por delante de `epoch_seconds`.
    fn docker_payload_at(
        epoch_seconds: u64,
        offset_seconds: i64,
        status: u16,
        path: &str,
    ) -> String {
        let at = i64::try_from(epoch_seconds).expect("epoch fits in i64") + offset_seconds;
        let instant = chrono::DateTime::from_timestamp(at, 0)
            .expect("a valid instant")
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        format!("{instant} {}", access_log_line(status, path))
    }

    /// Los frames multiplexados de 3.4, los **mismos** en los dos transportes: un
    /// frame partido entre dos chunks, un chunk que lleva dos frames y una carga
    /// con el prefijo del instante.
    fn docker_stream_chunks(since: u64) -> Vec<Vec<u8>> {
        let split_frame = docker_frame(STDOUT, &docker_payload_at(since, 1, 404, "/wp-login.php"));
        let (head, tail) = split_frame.split_at(split_frame.len() / 2);

        let mut second_chunk = tail.to_vec();
        second_chunk.extend(docker_frame(
            STDOUT,
            &docker_payload_at(since, 1, 429, "/api/v1/items"),
        ));

        vec![
            head.to_vec(),
            second_chunk,
            docker_frame(STDOUT, &docker_payload_at(since, 2, 401, "/api/login")),
        ]
    }

    /// Espera hasta que el pipeline haya registrado `expected` entradas con este
    /// status y path, hasta ~2 s.
    async fn wait_for_pipeline_count(status: i32, path: &str, expected: usize) -> bool {
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if pipeline_entry_count(status, path) >= expected {
                return true;
            }
        }
        false
    }

    /// Manda la orden de parada y comprueba que `run()` termina limpiamente.
    async fn shut_down(
        shutdown_tx: tokio::sync::watch::Sender<bool>,
        handle: tokio::task::JoinHandle<Result<(), JournalError>>,
    ) {
        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");
        assert!(result.is_ok(), "run() must return Ok on shutdown");
    }

    /// Levanta el servidor de prueba sobre un socket `AF_UNIX` temporal y devuelve
    /// el guard del path, el reader de Docker que lo ataca y la lista de
    /// peticiones recibidas.
    async fn docker_unix_fixture<R>(
        socket: &TempSocketPath,
        connections: usize,
        respond: R,
    ) -> (JournaldReader, Arc<Mutex<Vec<String>>>)
    where
        R: Fn(&str, usize) -> (u16, Vec<Vec<u8>>) + Send + 'static,
    {
        let listener = DockerTestListener::Unix(
            tokio::net::UnixListener::bind(socket.path()).expect("bind the docker test server"),
        );
        let recorded = spawn_docker_api(listener, connections, respond);
        let reader = docker_unix_reader(socket.path().to_string_lossy().as_ref(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        (reader, recorded)
    }

    // ── 3.4: el stream de frames del backend de Docker ──

    #[tokio::test]
    async fn test_run_streams_docker_frames_into_the_jail_pipeline_over_unix_socket() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let (reader, recorded) = docker_unix_fixture(&socket, 1, |request, _| {
            (200, docker_stream_chunks(since_from_request(request)))
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let split_frame = wait_for_pipeline_entry(404, "/wp-login.php").await;
        let two_frames = wait_for_pipeline_entry(429, "/api/v1/items").await;
        let prefixed = wait_for_pipeline_entry(401, "/api/login").await;

        shut_down(shutdown_tx, handle).await;

        assert!(
            recorded.lock().map_or(0, |r| r.len()) >= 1,
            "the reader must have sent the Docker log request"
        );
        assert!(
            split_frame,
            "the payload of a frame split between two chunks must reach the Jail pipeline"
        );
        assert!(
            two_frames,
            "both frames of one chunk must reach the pipeline, each with its own status_code"
        );
        assert!(
            prefixed,
            "the payload must reach the pipeline after its timestamp prefix is stripped"
        );
    }

    #[tokio::test]
    async fn test_run_streams_docker_frames_into_the_jail_pipeline_over_tcp() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the docker test server");
        let addr = listener.local_addr().expect("local addr");
        let recorded = spawn_docker_api(DockerTestListener::Tcp(listener), 1, |request, _| {
            (200, docker_stream_chunks(since_from_request(request)))
        });

        let reader = docker_tcp_reader(&format!("http://{addr}"), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let split_frame = wait_for_pipeline_entry(404, "/wp-login.php").await;
        let two_frames = wait_for_pipeline_entry(429, "/api/v1/items").await;
        let prefixed = wait_for_pipeline_entry(401, "/api/login").await;

        shut_down(shutdown_tx, handle).await;

        assert!(
            recorded.lock().map_or(0, |r| r.len()) >= 1,
            "the reader must have sent the Docker log request"
        );
        assert!(
            split_frame,
            "the same frames must be decoded over TCP as over the Unix socket"
        );
        assert!(
            two_frames,
            "both frames of one chunk must reach the pipeline over TCP too"
        );
        assert!(
            prefixed,
            "the timestamp prefix must be stripped over TCP too"
        );
    }

    // ── 3.5: no history replay en el backend de Docker ──

    #[tokio::test]
    async fn test_run_discards_docker_payloads_older_than_the_attempt() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let (reader, _recorded) = docker_unix_fixture(&socket, 1, |request, _| {
            let since = since_from_request(request);
            (
                200,
                vec![
                    docker_frame(STDOUT, &docker_payload_at(since, -3600, 407, "/history")),
                    docker_frame(STDOUT, &docker_payload_at(since, 1, 408, "/live")),
                ],
            )
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let live = wait_for_pipeline_entry(408, "/live").await;
        // Both payloads travel one after the other: once the live one is in, any
        // entry accepted from the historical one would already be there.
        tokio::time::sleep(Duration::from_millis(200)).await;

        shut_down(shutdown_tx, handle).await;

        assert!(
            live,
            "a payload dated after the connection attempt must reach the pipeline"
        );
        assert_eq!(
            pipeline_entry_count(407, "/history"),
            0,
            "a payload dated before the attempt cutoff is history: it must not reach the \
             pipeline, whatever the server sent"
        );
    }

    #[tokio::test]
    async fn test_run_counts_two_identical_docker_payloads_with_different_instants() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let (reader, _recorded) = docker_unix_fixture(&socket, 1, |request, _| {
            let since = since_from_request(request);
            (
                200,
                vec![
                    docker_frame(STDOUT, &docker_payload_at(since, 1, 401, "/dup")),
                    docker_frame(STDOUT, &docker_payload_at(since, 2, 401, "/dup")),
                ],
            )
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let both = wait_for_pipeline_count(401, "/dup", 2).await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        shut_down(shutdown_tx, handle).await;

        assert!(
            both,
            "two payloads with identical content and different instants are two events: the \
             Docker backend has no cursor, so nothing may deduplicate them by content"
        );
    }

    #[tokio::test]
    async fn test_run_moves_the_docker_cutoff_forward_after_a_failed_attempt() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let outage_at: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
        let outage_sink = Arc::clone(&outage_at);

        let (reader, recorded) = docker_unix_fixture(&socket, 2, move |request, index| {
            if index == 0 {
                // The attempt fails; whatever the engine writes while the reader
                // is down is dated here.
                if let Ok(mut slot) = outage_sink.lock() {
                    *slot = Some(now_micros());
                }
                return (500, Vec::new());
            }

            let during_outage = outage_sink
                .lock()
                .ok()
                .and_then(|slot| *slot)
                .map_or(0, |micros| micros / MICROS_PER_SECOND);
            let since = since_from_request(request);
            (
                200,
                vec![
                    docker_frame(
                        STDOUT,
                        &docker_payload_at(during_outage, 0, 412, "/during-outage"),
                    ),
                    docker_frame(STDOUT, &docker_payload_at(since, 1, 413, "/after-outage")),
                ],
            )
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let after_outage = wait_for_pipeline_entry(413, "/after-outage").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        shut_down(shutdown_tx, handle).await;

        assert!(
            recorded.lock().map_or(0, |r| r.len()) >= 2,
            "the reader must retry after the failed attempt"
        );
        assert!(
            after_outage,
            "a payload dated after the new attempt must reach the pipeline"
        );
        assert_eq!(
            pipeline_entry_count(412, "/during-outage"),
            0,
            "whatever was written while the reader was down is older than the new cutoff: each \
             attempt fixes its own when it starts"
        );
    }

    #[tokio::test]
    async fn test_run_reconnects_the_docker_stream_without_counting_a_payload_twice() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let (reader, recorded) = docker_unix_fixture(&socket, 2, |request, index| {
            let since = since_from_request(request);
            if index == 0 {
                // The whole payload travels first; then the stream is cut in the
                // middle of the next frame (a header with more bytes promised
                // than delivered), which the reader must not decode.
                let frame = docker_frame(STDOUT, &docker_payload_at(since, 1, 401, "/boundary"));
                let partial = docker_frame(STDOUT, &docker_payload_at(since, 2, 402, "/never"));
                return (200, vec![frame, partial[..12].to_vec()]);
            }

            // `since` has second granularity, so the engine can hand back a
            // payload dated inside the new cutoff's own second. It is not
            // strictly after the cutoff, so it must be discarded: that is what
            // keeps the boundary payload from being counted twice.
            (
                200,
                vec![docker_frame(
                    STDOUT,
                    &docker_payload_at(since, 0, 401, "/boundary"),
                )],
            )
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let delivered = wait_for_pipeline_count(401, "/boundary", 1).await;

        // Wait until the reader has reconnected: the second request is in.
        let mut second_request = None;
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let requests = recorded.lock().map(|r| r.clone()).unwrap_or_default();
            if requests.len() >= 2 {
                second_request = Some(requests[1].clone());
                break;
            }
        }
        let observed_seconds = now_micros() / MICROS_PER_SECOND;
        // Give the reader time to process (and discard) the second attempt's
        // payload before counting.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let counted = pipeline_entry_count(401, "/boundary");

        let first_request = recorded
            .lock()
            .map(|r| r.first().cloned())
            .unwrap_or_default();

        shut_down(shutdown_tx, handle).await;

        assert!(
            delivered,
            "the payload delivered before the stream was cut must reach the pipeline once"
        );
        assert_eq!(
            counted, 1,
            "no payload of the new cutoff's second may be counted twice: the exact comparison \
             with the attempt cutoff is what discards it"
        );

        let first_request =
            first_request.expect("the first Docker request must have been recorded");
        let second_request =
            second_request.expect("the reader must reconnect after the stream is cut");
        let since_first = since_from_request(&first_request);
        let since_second = since_from_request(&second_request);
        assert!(
            since_second >= since_first,
            "the reconnect must not travel further back than the first attempt \
             ({since_second} < {since_first})"
        );
        // Margen de 2 s: `observed_seconds` se lee *después* de observar el
        // bucle, así que un desalojo del hilo o un salto del reloj hacia atrás
        // entre el instante del segundo intento y esa lectura lo desplazan; el
        // contrato fino —que la carga del boundary no se cuente dos veces— ya lo
        // cubre el `assert_eq!` del recuento, no esta aserción, que sólo exige
        // que el segundo intento no viaje más atrás que «ahora».
        assert!(
            since_second + 2 >= observed_seconds,
            "the second request must carry the cutoff of the new attempt as a coarse hint \
             (since={since_second}, the reconnect was observed at {observed_seconds}), not a \
             stale one frozen at the last processed payload"
        );
    }

    // ── 3.6: fail-open con el backend de Docker ──
    //
    // El bucle, el reintento, la respuesta a la señal de parada y el mecanismo
    // para no dejar tareas colgadas son **los mismos** que los de gatewayd (lo
    // prohíbe la tarea 4.7): estos tests manejan el mismo `run()` con un
    // transporte de Docker configurado, y sólo cambia lo que el `connect()`
    // devuelve.

    #[tokio::test]
    async fn test_run_survives_a_missing_docker_socket() {
        let socket = TempSocketPath::new();
        let (logs, _subscriber) = capture_logs_while_running();

        let reader = docker_unix_reader(socket.path().to_string_lossy().as_ref(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        // Several retries must have happened by now.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let captured = logs.captured();

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stay responsive to shutdown while retrying")
            .expect("run() must not panic when the Docker socket is missing");
        assert!(
            result.is_ok(),
            "a missing Docker socket is a retryable condition, not a fatal error: {result:?}"
        );

        assert!(
            captured.matches("retrying in").count() >= 2,
            "the reader must retry after retry_delay; captured logs were: {captured:?}"
        );
        assert!(
            captured.to_lowercase().contains("docker api not available"),
            "the warning must say the Docker API is not available instead of leaving the \
             operator guessing which backend broke; captured logs were: {captured:?}"
        );
        let endpoint = format!("docker:unix:{} (traefik)", socket.path().display());
        assert!(
            captured.contains(&endpoint),
            "the warning must name the endpoint ({endpoint}); captured logs were: {captured:?}"
        );
    }

    #[tokio::test]
    async fn test_run_survives_an_unreachable_docker_proxy() {
        let (logs, _subscriber) = capture_logs_while_running();

        // Port 1 is not listening → connection refused.
        let reader = docker_tcp_reader("http://127.0.0.1:1", "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        tokio::time::sleep(Duration::from_millis(300)).await;
        let captured = logs.captured();

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stay responsive to shutdown while retrying")
            .expect("run() must not panic when the Docker proxy is down");
        assert!(
            result.is_ok(),
            "an unreachable Docker proxy is a retryable condition, not a fatal error: {result:?}"
        );

        assert!(
            captured.matches("retrying in").count() >= 2,
            "the reader must retry after retry_delay; captured logs were: {captured:?}"
        );
        assert!(
            captured.to_lowercase().contains("docker api not available"),
            "the warning must say the Docker API is not available; captured logs were: {captured:?}"
        );
        assert!(
            captured.contains("docker:http://127.0.0.1:1 (traefik)"),
            "the warning must name the endpoint of the configured proxy; captured logs were: \
             {captured:?}"
        );
    }

    #[tokio::test]
    async fn test_run_survives_a_container_that_does_not_exist() {
        let socket = TempSocketPath::new();
        let (logs, _subscriber) = capture_logs_while_running();

        // The engine answers 404 with its own message when the container is
        // unknown; the reader never decodes that body, so only the status and
        // the container name can name the cause.
        let (reader, recorded) = docker_unix_fixture(&socket, 3, |_, _| {
            (
                404,
                vec![br#"{"message":"No such container: traefik"}"#.to_vec()],
            )
        })
        .await;

        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        // Several retries must have happened by now.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let captured = logs.captured();

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stay responsive to shutdown while retrying")
            .expect("run() must not panic when the container does not exist");
        assert!(
            result.is_ok(),
            "an unknown container is a retryable condition, not a fatal error: {result:?}"
        );

        assert!(
            recorded.lock().map_or(0, |r| r.len()) >= 2,
            "the reader must retry after retry_delay; captured logs were: {captured:?}"
        );
        assert!(
            captured.to_lowercase().contains("docker api not available"),
            "the warning must say the Docker API is not available; captured logs were: {captured:?}"
        );
        let endpoint = format!("docker:unix:{} (traefik)", socket.path().display());
        assert!(
            captured.contains(&endpoint),
            "the warning must name the endpoint ({endpoint}); captured logs were: {captured:?}"
        );
        let cause = captured.to_lowercase();
        assert!(
            cause.contains("404") && cause.contains("no such container"),
            "the warning must name the cause —a 404 for the container that does not exist— \
             instead of blaming a gateway that the Docker backend does not have; captured logs \
             were: {captured:?}"
        );
    }

    // ── 3.7: shutdown a mitad de stream con el backend de Docker ──

    #[tokio::test]
    async fn test_run_stops_docker_mid_stream_when_shutdown_arrives() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let listener =
            tokio::net::UnixListener::bind(socket.path()).expect("bind the holding docker stream");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let request = read_request_head(&mut stream).await;
                let frame = docker_frame(
                    STDOUT,
                    &docker_payload_at(since_from_request(&request), 1, 503, "/pending"),
                );
                // More bytes promised than written, and the connection stays
                // open: the reader is really blocked in `body.frame()` when the
                // stop signal arrives.
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.docker.raw-stream\r\n\
                     Content-Length: {}\r\n\r\n",
                    frame.len() + 4096
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&frame).await;
                let _ = stream.flush().await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        let reader = docker_unix_reader(socket.path().to_string_lossy().as_ref(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        assert!(
            wait_for_pipeline_entry(503, "/pending").await,
            "the open Docker stream must still deliver the payload of its first frame"
        );

        let started = Instant::now();
        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("shutdown must interrupt a pending frame()")
            .expect("run() must not panic");
        let elapsed = started.elapsed();

        assert!(
            result.is_ok(),
            "run() must return Ok on shutdown: {result:?}"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "a shutdown in the middle of a Docker stream must not wait for the pending frame: \
             took {elapsed:?}"
        );
    }

    // ── History replay filter ──

    /// Empties the shared audit buffer so a test only observes its own entries.
    ///
    /// Callers must hold `TEST_LOCK`.
    fn reset_log_collector() {
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }
    }

    /// Serves two connections over `AF_UNIX`, recording the request head of
    /// each. The first one is rejected with HTTP 500; the second one returns
    /// the body built from the instant the first request arrived, which is the
    /// moment the journal content is "written during the outage".
    fn spawn_outage_unix_gateway(socket_path: &Path) -> Arc<Mutex<Vec<String>>> {
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("bind outage unix gateway");
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);

        tokio::spawn(async move {
            // First attempt: reject it, keeping the instant of the request.
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let request = read_request_head(&mut stream).await;
            let outage_at = now_micros();
            if let Ok(mut sink) = sink.lock() {
                sink.push(request);
            }
            let _ = stream
                .write_all(
                    b"HTTP/1.1 500 Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            let _ = stream.flush().await;

            // Second attempt: what the journal accumulated while the reader was
            // down, next to an entry produced after it reconnected.
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let request = read_request_head(&mut stream).await;
            if let Ok(mut sink) = sink.lock() {
                sink.push(request);
            }
            let body = format!(
                "{}\n{}\n",
                journal_line_at(None, 412, "/during-outage", outage_at),
                journal_line_at(None, 413, "/after-outage", outage_at + LIVE_SKEW_MICROS),
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        });

        recorded
    }

    #[tokio::test]
    async fn test_run_ignores_entries_older_than_the_connection_attempt() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let body = format!(
            "{}\n{}\n",
            journal_line_at(None, 407, "/history", OLD_TIMESTAMP_MICROS),
            journal_line(None, 408, "/live"),
        );
        let socket = TempSocketPath::new();
        spawn_fake_unix_gateway(socket.path(), body).await;

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let live = wait_for_pipeline_entry(408, "/live").await;
        // Both lines travel in the same body: once the live one is in, any
        // entry accepted from the historical one would already be there.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            live,
            "an entry dated after the connection attempt must reach the pipeline"
        );
        assert_eq!(
            pipeline_entry_count(407, "/history"),
            0,
            "an entry dated before the connection attempt must not reach the pipeline"
        );
    }

    #[tokio::test]
    async fn test_run_discards_entries_without_a_usable_timestamp() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let body = format!(
            "{}\n{}\n{}\n",
            journal_line_with_timestamp(None, 409, "/no-timestamp", None),
            journal_line_with_timestamp(None, 410, "/bad-timestamp", Some("abc")),
            journal_line(None, 411, "/with-timestamp"),
        );
        let socket = TempSocketPath::new();
        spawn_fake_unix_gateway(socket.path(), body).await;

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let valid = wait_for_pipeline_entry(411, "/with-timestamp").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            valid,
            "the entry with a usable timestamp must still reach the pipeline"
        );
        assert_eq!(
            pipeline_entry_count(409, "/no-timestamp"),
            0,
            "an entry without __REALTIME_TIMESTAMP cannot be placed in time and must be discarded"
        );
        assert_eq!(
            pipeline_entry_count(410, "/bad-timestamp"),
            0,
            "an entry whose __REALTIME_TIMESTAMP is not a number must be discarded"
        );
    }

    #[tokio::test]
    async fn test_run_moves_the_cutoff_forward_after_a_failed_attempt() {
        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        let socket = TempSocketPath::new();
        let recorded = spawn_outage_unix_gateway(socket.path());

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(150));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let after_outage = wait_for_pipeline_entry(413, "/after-outage").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            recorded.lock().map_or(0, |r| r.len()) >= 2,
            "the reader must retry after the failed attempt"
        );
        assert!(
            after_outage,
            "an entry dated after the reconnect must reach the pipeline"
        );
        assert_eq!(
            pipeline_entry_count(412, "/during-outage"),
            0,
            "an entry written while the reader was down is older than the new cutoff: \
             the cutoff of each attempt is fixed when that attempt starts"
        );
    }

    #[tokio::test]
    async fn test_run_does_not_reprocess_the_entry_at_the_cursor_after_reconnecting() {
        const CURSOR: &str = "s=dup;i=7;b=abc";

        let _guard = TEST_LOCK.lock().await;
        reset_log_collector();

        // Dated in the future on purpose: only the cursor rule can discard the
        // replay, so the assertion cannot pass for the cutoff's sake.
        let line = journal_line(Some(CURSOR), 414, "/replayed");
        let body = format!("{line}\n");
        let (gateway_url, recorded) = spawn_recording_gateway(vec![body.clone(), body]).await;

        let reader = JournaldReader::new(&gateway_url, "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let processed = wait_for_pipeline_entry(414, "/replayed").await;
        let mut reconnected = false;
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if recorded.lock().map_or(0, |r| r.len()) >= 2 {
                reconnected = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            processed,
            "the entry must reach the pipeline on the first read"
        );
        assert!(
            reconnected,
            "the reader must have reconnected and been served the entry again"
        );
        assert_eq!(
            pipeline_entry_count(414, "/replayed"),
            1,
            "the entry at the last processed cursor must not be processed twice"
        );
    }

    #[tokio::test]
    async fn test_tcp_request_uses_origin_form_and_required_headers() {
        let (gateway_url, recorded) = spawn_recording_gateway(vec![String::new()]).await;
        let host = gateway_url.trim_start_matches("http://").to_string();

        let reader = JournaldReader::new(&gateway_url, "traefik")
            .with_retry_delay(tokio::time::Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let mut request = None;
        for _ in 0..60 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let requests = recorded.lock().map(|r| r.clone()).unwrap_or_default();
            if let Some(first) = requests.first() {
                request = Some(first.clone());
                break;
            }
        }

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");
        assert!(result.is_ok(), "run() must return Ok on shutdown");

        let request = request.expect("the TCP transport must send a request");
        assert!(
            request.starts_with("GET /entries?follow&CONTAINER_NAME=traefik HTTP/1.1\r\n"),
            "the TCP request must use the origin form; got:\n{request}"
        );
        let lower = request.to_lowercase();
        assert!(
            lower.contains(&format!("host: {host}")),
            "the TCP request must send the host:port it connected to; got:\n{request}"
        );
        assert!(
            lower.contains("accept: application/json"),
            "the TCP request must ask for the Journal JSON Format; got:\n{request}"
        );
    }

    #[tokio::test]
    async fn test_run_survives_unreachable_gateway() {
        // Port 1 is not listening → connection refused
        let reader = JournaldReader::new("http://127.0.0.1:1", "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn({
            let app_state = Arc::clone(&app_state);
            async move { reader.run(app_state, shutdown_rx).await }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let _ = shutdown_tx.send(true);

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle)
            .await
            .expect("run() must stay responsive to shutdown while retrying")
            .expect("run() must not panic when the gateway is down");

        assert!(
            result.is_ok(),
            "an unreachable gateway is a retryable condition, not a fatal error: {result:?}"
        );
    }

    // ── Task 2: hyper connection layer ──

    #[tokio::test]
    async fn test_connect_succeeds_over_unix_socket() {
        let socket = TempSocketPath::new();
        let listener = tokio::net::UnixListener::bind(socket.path()).expect("bind unix listener");

        let reader = JournaldReader::unix(socket.path(), "traefik");
        assert!(
            reader.connect().await.is_ok(),
            "connect() must reach a socket that is being listened on"
        );

        drop(listener);
    }

    #[tokio::test]
    async fn test_connect_succeeds_over_tcp() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind tcp listener");
        let addr = listener.local_addr().expect("local addr");

        let reader = JournaldReader::new(&format!("http://{addr}"), "traefik");
        assert!(
            reader.connect().await.is_ok(),
            "connect() must reach a listening TCP endpoint"
        );

        drop(listener);
    }

    #[tokio::test]
    async fn test_connect_missing_unix_socket_is_a_connection_error() {
        let socket = TempSocketPath::new();
        let reader = JournaldReader::unix(socket.path(), "traefik");

        match reader.connect().await {
            Err(JournalError::Connection(msg, ..)) => assert!(
                msg.contains("shuul-"),
                "the error must name the socket path, got: {msg}"
            ),
            Ok(_) => panic!("connecting to a missing socket must fail"),
            Err(other) => panic!("expected a Connection error, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_connect_rejects_https() {
        let reader = JournaldReader::new("https://remote:19531", "traefik");

        match reader.connect().await {
            Err(JournalError::Connection(msg, ..)) => {
                let lower = msg.to_lowercase();
                assert!(
                    lower.contains("https") && lower.contains("not supported"),
                    "TLS must be rejected with a clear message, got: {msg}"
                );
            },
            Ok(_) => panic!("https is not supported and must not connect"),
            Err(other) => panic!("expected a Connection error, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_connect_rejects_invalid_url() {
        let reader = JournaldReader::new("not a url", "traefik");
        assert!(
            matches!(reader.connect().await, Err(JournalError::Connection(..))),
            "an unparseable base URL is a Connection error"
        );
    }

    // ── Task 3: streaming over a Unix socket ──

    /// Mirrors `test_run_streams_entry_into_jail_pipeline` over `AF_UNIX`.
    #[tokio::test]
    async fn test_run_streams_entry_into_jail_pipeline_over_unix_socket() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let body = format!("{}\n", journal_line(None, 404, "/api/login"));
        let socket = TempSocketPath::new();
        spawn_fake_unix_gateway(socket.path(), body).await;

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn({
            let app_state = Arc::clone(&app_state);
            async move { reader.run(app_state, shutdown_rx).await }
        });

        let found = wait_for_pipeline_entry(404, "/api/login").await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(
            found,
            "an access log served over AF_UNIX must reach the Jail pipeline"
        );
        assert!(
            result.is_ok(),
            "run() should return Ok on shutdown: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_run_reassembles_an_ndjson_line_split_across_chunks() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let line = journal_line(None, 418, "/split");
        let split = line.len() / 2;
        let body = format!("{line}\n");
        let socket = TempSocketPath::new();
        spawn_chunked_unix_gateway(
            socket.path(),
            vec![body[..split].to_string(), body[split..].to_string()],
        )
        .await;

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let found = wait_for_pipeline_entry(418, "/split").await;

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown, not hang")
            .expect("run() task must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            found,
            "a line split across two body frames must be reassembled"
        );
    }

    #[tokio::test]
    async fn test_run_resumes_from_last_cursor_over_unix_socket() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        const CURSOR: &str = "s=abc;i=42;b=def";

        let line = journal_line(Some(CURSOR), 429, "/resume");
        let socket = TempSocketPath::new();
        let recorded = spawn_cursor_aware_unix_gateway(socket.path(), line, CURSOR).await;

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let mut requests = Vec::new();
        for _ in 0..80 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            requests = recorded.lock().map(|r| r.clone()).unwrap_or_default();
            if requests.len() >= 2 {
                break;
            }
        }

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown")
            .expect("run() task must not panic");

        assert!(
            requests.len() >= 2,
            "the reader must reconnect after the stream closes, got {} requests",
            requests.len()
        );
        let second_request = &requests[1];
        let lower = second_request.to_lowercase();
        assert!(
            lower.contains("range:")
                && lower.contains(&format!("entries={}:1", CURSOR.to_lowercase())),
            "the reconnect request must resume from the last cursor; got:\n{second_request}"
        );
        assert!(
            lower.contains("/entries?follow&container_name=traefik"),
            "the socket request must use the origin-form target; got:\n{second_request}"
        );
        assert!(
            lower.contains("accept: application/json"),
            "the request must ask for the Journal JSON Format; got:\n{second_request}"
        );
        assert!(
            lower.contains("host: localhost"),
            "a socket request still needs a Host header; got:\n{second_request}"
        );
        // The fake replays the entry unless the cursor matches, so a wrong
        // cursor would show up here as a second processed entry.
        assert_eq!(
            pipeline_entry_count(429, "/resume"),
            1,
            "the entry at the cursor must not be processed twice"
        );
        assert!(result.is_ok(), "run() must return Ok on shutdown");
    }

    #[tokio::test]
    async fn test_run_restarts_without_cursor_when_the_gateway_rejects_it() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        const CURSOR: &str = "s=stale;i=7;b=dead";

        let line = journal_line(Some(CURSOR), 451, "/stale");
        let socket = TempSocketPath::new();
        let recorded = spawn_recording_unix_gateway(
            socket.path(),
            vec![
                (200, format!("{line}\n")),
                (400, String::new()),
                (200, String::new()),
            ],
        )
        .await;

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let mut requests = Vec::new();
        for _ in 0..80 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            requests = recorded.lock().map(|r| r.clone()).unwrap_or_default();
            if requests.len() >= 3 {
                break;
            }
        }

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown")
            .expect("run() task must not panic");

        assert!(
            requests.len() >= 3,
            "a rejected cursor must be followed by a fresh attempt, got {} requests",
            requests.len()
        );
        assert!(
            requests[1].to_lowercase().contains("range:"),
            "the attempt after the entry was processed must carry the cursor; got:\n{}",
            requests[1]
        );
        assert!(
            !requests[2].to_lowercase().contains("range:"),
            "a rejected cursor must be dropped, so the next attempt starts from scratch; got:\n{}",
            requests[2]
        );
        assert!(result.is_ok(), "run() must return Ok on shutdown");
    }

    #[tokio::test]
    async fn test_run_survives_a_missing_unix_socket() {
        let socket = TempSocketPath::new();

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        // Several retries must have happened by now; none may abort the loop.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = shutdown_tx.send(true);

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stay responsive to shutdown while retrying")
            .expect("run() must not panic when the socket is missing");

        assert!(
            result.is_ok(),
            "a missing socket is a retryable condition, not a fatal error: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_run_closes_the_connection_after_an_error_status() {
        let socket = TempSocketPath::new();
        let listener = tokio::net::UnixListener::bind(socket.path()).expect("bind error gateway");
        let (eof_tx, mut eof_rx) = tokio::sync::mpsc::channel::<bool>(1);

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = read_request_head(&mut stream).await;
                // No `Connection: close`: the server keeps its side open. The
                // reader must still release the socket after the error status.
                let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;

                let mut buf = [0_u8; 64];
                let eof = matches!(
                    tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf)).await,
                    Ok(Ok(0))
                );
                let _ = eof_tx.send(eof).await;
            }
        });

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let closed = tokio::time::timeout(Duration::from_secs(3), eof_rx.recv())
            .await
            .expect("the server must observe the client closing the socket")
            .expect("the gateway task must report EOF");

        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown")
            .expect("run() task must not panic");

        assert!(
            closed,
            "an error status must close the connection even if the gateway keeps it open"
        );
        assert!(result.is_ok(), "run() must return Ok on shutdown");
    }

    #[tokio::test]
    async fn test_run_stops_when_shutdown_interrupts_a_pending_frame() {
        let _guard = TEST_LOCK.lock().await;
        if let Ok(mut collector) = LOG_COLLECTOR.lock() {
            *collector = crate::models::log_collector::LogCollector::new(1000);
        }

        let body = format!("{}\n", journal_line(None, 503, "/pending"));
        let socket = TempSocketPath::new();
        let listener = tokio::net::UnixListener::bind(socket.path()).expect("bind holding gateway");

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0_u8; 4096];
                let _ = stream.read(&mut request).await;
                let header = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n";
                let _ = stream.write_all(header.as_bytes()).await;
                let chunk = format!("{:x}\r\n{body}\r\n", body.len());
                let _ = stream.write_all(chunk.as_bytes()).await;
                let _ = stream.flush().await;
                // Never terminate the chunked stream: the reader stays blocked
                // waiting for the next frame.
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        assert!(
            wait_for_pipeline_entry(503, "/pending").await,
            "the open stream must still deliver its first line"
        );

        let started = Instant::now();
        let _ = shutdown_tx.send(true);
        let result = tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("shutdown must interrupt a pending frame()")
            .expect("run() must not panic");

        assert!(result.is_ok(), "run() must return Ok on shutdown");
        assert!(
            started.elapsed() < Duration::from_millis(900),
            "a pending frame must not delay shutdown; took {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn test_run_aborts_a_mute_handshake_after_connect_timeout() {
        assert_eq!(
            CONNECT_TIMEOUT,
            Duration::from_secs(10),
            "the connect timeout is part of the public contract"
        );

        let app_state = build_test_app_state().await;
        let socket = TempSocketPath::new();
        let listener = tokio::net::UnixListener::bind(socket.path()).expect("bind mute gateway");
        let accepts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&accepts);

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, Ordering::SeqCst);
                // Accept but say nothing: no HTTP response is ever sent.
                tokio::spawn(async move {
                    let _stream = stream;
                    tokio::time::sleep(Duration::from_secs(600)).await;
                });
            }
        });

        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // The clock pauses only now: `build_test_app_state` needs real time for
        // the SQLite pool. From here on, advancing the clock is what makes
        // CONNECT_TIMEOUT fire, so the test does not wait 10 real seconds.
        tokio::time::pause();

        // Virtual clock: `tokio::time::Instant` so the elapsed time below is
        // measured in the same timeline the timeout uses.
        let started = tokio::time::Instant::now();
        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        let mut retried = false;
        for _ in 0..400 {
            tokio::time::advance(Duration::from_millis(50)).await;
            if accepts.load(Ordering::SeqCst) >= 2 {
                retried = true;
                break;
            }
        }
        let elapsed = started.elapsed();

        let _ = shutdown_tx.send(true);
        // Back to the real clock for the watchdog: a shutdown regression must
        // fail the test, not hang it on a paused clock.
        tokio::time::resume();
        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("run() must stop on shutdown")
            .expect("run() task must not panic");

        assert!(
            result.is_ok(),
            "a timed-out connect must be retried, not fatal: {result:?}"
        );
        assert!(
            retried,
            "a server that never answers must be aborted and retried"
        );
        assert!(
            elapsed >= CONNECT_TIMEOUT,
            "the attempt must not be abandoned before CONNECT_TIMEOUT; took {elapsed:?}"
        );
        assert!(
            elapsed < CONNECT_TIMEOUT + Duration::from_secs(2),
            "the attempt must be aborted by CONNECT_TIMEOUT; took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn test_run_keeps_running_when_the_shutdown_sender_is_dropped() {
        let socket = TempSocketPath::new();
        let reader = JournaldReader::unix(socket.path(), "traefik")
            .with_retry_delay(Duration::from_millis(50));
        let app_state = build_test_app_state().await;
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let handle = tokio::spawn(async move { reader.run(app_state, shutdown_rx).await });

        // Dropping every sender closes the channel. That is not a stop order:
        // the reader must keep running instead of returning silently.
        drop(shutdown_tx);
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert!(
            !handle.is_finished(),
            "a closed shutdown channel must not be treated as a shutdown signal"
        );

        handle.abort();
    }

    // ── JournalError source chain (M-7) ──

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn test_journal_error_stays_send_sync_static() {
        assert_send_sync_static::<JournalError>();
    }

    #[test]
    fn test_parse_failure_exposes_the_serde_json_error() {
        let err = super::parse_journal_entry("this is not json")
            .err()
            .expect("invalid JSON must fail");
        assert!(matches!(err, JournalError::Parse(..)));

        let source = std::error::Error::source(&err)
            .expect("the serde_json error must be attached as the source");
        let json = source
            .downcast_ref::<serde_json::Error>()
            .expect("the source must be a serde_json::Error");
        assert_eq!(json.line(), 1, "the single-line input is on line 1");
        assert!(json.column() >= 1, "the column must be reported");
        assert!(
            err.to_string().contains("invalid journal JSON"),
            "the Display must still name the context, got: {err}"
        );
    }

    #[test]
    fn test_traefik_parse_failure_exposes_the_serde_json_error() {
        let err = super::parse_traefik_access_log("this is not json")
            .err()
            .expect("invalid JSON must fail");
        assert!(matches!(err, JournalError::TraefikParse(..)));

        let json = std::error::Error::source(&err)
            .and_then(|source| source.downcast_ref::<serde_json::Error>())
            .expect("the source must be a serde_json::Error");
        assert_eq!(json.line(), 1);
        assert!(json.column() >= 1);
        assert!(
            err.to_string().contains("invalid JSON"),
            "the Display must still name the context, got: {err}"
        );
    }

    #[test]
    fn test_invalid_gateway_url_exposes_the_invalid_uri() {
        let err = tcp_endpoint("not a url")
            .err()
            .expect("a value with spaces cannot be a URI");
        assert!(matches!(err, JournalError::Connection(..)));

        let source =
            std::error::Error::source(&err).expect("the InvalidUri must be attached as the source");
        assert!(
            source.downcast_ref::<http::uri::InvalidUri>().is_some(),
            "the source must be the http::uri::InvalidUri, got: {source}"
        );
        let message = err.to_string();
        assert!(message.contains("invalid gateway URL"), "got: {message}");
        assert!(message.contains("not a url"), "got: {message}");
    }

    #[tokio::test]
    async fn test_missing_unix_socket_exposes_an_io_error_not_found() {
        let socket = TempSocketPath::new();
        let reader = JournaldReader::unix(socket.path(), "traefik");

        let err = reader
            .connect()
            .await
            .err()
            .expect("connecting to a missing socket must fail");
        assert!(matches!(err, JournalError::Connection(..)));

        let io = std::error::Error::source(&err)
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .expect("the source must be the io::Error");
        assert_eq!(
            io.kind(),
            std::io::ErrorKind::NotFound,
            "a missing socket must be distinguishable from a gateway that is down"
        );
    }

    #[tokio::test]
    async fn test_closed_tcp_port_exposes_an_io_error_connection_refused() {
        let reader = JournaldReader::new("http://127.0.0.1:1", "traefik");

        let err = reader
            .connect()
            .await
            .err()
            .expect("connecting to a closed port must fail");
        assert!(matches!(err, JournalError::Connection(..)));

        let io = std::error::Error::source(&err)
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .expect("the source must be the io::Error");
        assert_eq!(
            io.kind(),
            std::io::ErrorKind::ConnectionRefused,
            "a closed port must be distinguishable from a missing socket"
        );
    }

    #[tokio::test]
    async fn test_truncated_response_body_exposes_a_hyper_error() {
        // The gateway announces 100 bytes but only sends 10 and closes: the
        // body stream must fail with a `hyper::Error`, not end cleanly.
        let socket = TempSocketPath::new();
        let listener =
            tokio::net::UnixListener::bind(socket.path()).expect("bind truncating gateway");
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = read_request_head(&mut stream).await;
                let header = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(b"0123456789").await;
                let _ = stream.flush().await;
                // Dropping the stream closes it: EOF before 100 bytes.
            }
        });

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (_shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let mut cursor = None;

        let err = reader
            .stream_once(&app_state, &mut shutdown_rx, &mut cursor)
            .await
            .err()
            .expect("a truncated body must be an error, not a clean disconnect");
        assert!(
            matches!(err, JournalError::Socket(..)),
            "the frame error must surface as Socket, got: {err:?}"
        );

        // The `hyper::Error` must be reachable by type instead of being
        // flattened. No assertion on its wording: that belongs to hyper.
        assert!(
            std::error::Error::source(&err)
                .and_then(|source| source.downcast_ref::<hyper::Error>())
                .is_some(),
            "the source must be the hyper::Error"
        );
    }

    #[tokio::test]
    async fn test_send_request_exposes_a_hyper_error_when_the_peer_closes_early() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = read_request_head(&mut stream).await;
                // Close without ever writing a response.
            }
        });

        let reader = JournaldReader::new(&format!("http://{addr}"), "traefik");
        let err = reader
            .send_request(None, now_micros())
            .await
            .err()
            .expect("a peer that closes early must produce an error");
        assert!(matches!(err, JournalError::Connection(..)));

        let source = std::error::Error::source(&err)
            .expect("the hyper::Error must be attached as the source");
        assert!(
            source.downcast_ref::<hyper::Error>().is_some(),
            "the source must be the hyper::Error, got: {source}"
        );
    }

    #[test]
    fn test_context_only_errors_have_no_source_and_keep_their_display() {
        let parse = super::parse_journal_entry("")
            .err()
            .expect("an empty line must fail");
        assert!(matches!(parse, JournalError::Parse(..)));
        assert!(
            std::error::Error::source(&parse).is_none(),
            "a context-only failure must not carry a source"
        );
        assert_eq!(
            parse.to_string(),
            "Journal entry parse error: empty journal entry",
            "the Display of a context-only failure must not change"
        );

        let conn = tcp_endpoint("localhost:19531")
            .err()
            .expect("a scheme-less host:port must fail");
        assert!(matches!(conn, JournalError::Connection(..)));
        assert!(std::error::Error::source(&conn).is_none());
        assert_eq!(
            conn.to_string(),
            "Connection error: gateway URL localhost:19531 has no scheme",
            "the Display of a context-only failure must not change"
        );
    }

    #[derive(Debug, thiserror::Error)]
    #[error("outer wrapper")]
    struct OuterWrapper(#[source] std::io::Error);

    #[test]
    fn test_the_source_chain_is_walkable_end_to_end() {
        let root = std::io::Error::new(std::io::ErrorKind::Other, "root cause");
        let err = JournalError::connection_from("ctx", OuterWrapper(root));

        let first = std::error::Error::source(&err).expect("the source must be present");
        assert_eq!(first.to_string(), "outer wrapper");
        let second =
            std::error::Error::source(first).expect("the source of the source must be present");
        assert_eq!(second.to_string(), "root cause");
        assert!(
            std::error::Error::source(second).is_none(),
            "the chain must end after the root cause"
        );
    }

    #[tokio::test]
    async fn test_gateway_error_status_keeps_its_variant_and_text() {
        let socket = TempSocketPath::new();
        spawn_recording_unix_gateway(socket.path(), vec![(400, String::new())]).await;

        let reader = JournaldReader::unix(socket.path(), "traefik");
        let app_state = build_test_app_state().await;
        let (_shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let mut cursor = None;

        let err = reader
            .stream_once(&app_state, &mut shutdown_rx, &mut cursor)
            .await
            .err()
            .expect("a non-success status must be an error");
        match err {
            JournalError::Connection(msg, source) => {
                assert_eq!(msg, "gateway returned HTTP 400 Bad Request");
                assert!(
                    source.is_none(),
                    "a context-only failure must not carry a source"
                );
            },
            other => panic!("expected a Connection error, got: {other:?}"),
        }
    }
}

// ── Task 8: configuration from environment ──

#[cfg(test)]
mod tests_task8 {
    use super::tests_docker_transport::CUTOFF_MICROS;
    use super::{DEFAULT_GATEWAY_URL, JournaldReader, JournaldTransport};
    use http::header;
    use serial_test::serial;
    use std::sync::{Arc, Mutex};

    const DEFAULT_CONTAINER: &str = "traefik";

    fn tcp(base_url: &str) -> JournaldTransport {
        JournaldTransport::Tcp {
            base_url: base_url.to_string(),
        }
    }

    #[test]
    fn test_from_values_uses_defaults_when_absent() {
        let reader = JournaldReader::from_values(None, None, None, None, None);
        assert_eq!(reader.transport, tcp(DEFAULT_GATEWAY_URL));
        assert_eq!(reader.container_name, DEFAULT_CONTAINER);
    }

    #[test]
    fn test_from_values_honours_url_and_container() {
        let reader = JournaldReader::from_values(
            None,
            None,
            None,
            Some("http://host.docker.internal:19531".to_string()),
            Some("my-traefik".to_string()),
        );
        assert_eq!(reader.transport, tcp("http://host.docker.internal:19531"));
        assert_eq!(reader.container_name, "my-traefik");
    }

    #[test]
    fn test_from_values_socket_only_selects_unix_transport() {
        let reader = JournaldReader::from_values(
            None,
            None,
            Some("/run/systemd/journal/gatewayd.sock".to_string()),
            None,
            None,
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: std::path::PathBuf::from("/run/systemd/journal/gatewayd.sock")
            }
        );
    }

    #[test]
    fn test_from_values_socket_takes_precedence_over_url() {
        let reader = JournaldReader::from_values(
            None,
            None,
            Some("/run/systemd/journal/gatewayd.sock".to_string()),
            Some("http://remote:19531".to_string()),
            Some("traefik".to_string()),
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: std::path::PathBuf::from("/run/systemd/journal/gatewayd.sock")
            },
            "when both are set the socket must win"
        );
    }

    #[test]
    fn test_from_values_socket_takes_precedence_over_url_and_warns() {
        let logs = capture_logs(|| {
            let reader = JournaldReader::from_values(
                None,
                None,
                Some("/run/systemd/journal/gatewayd.sock".to_string()),
                Some("http://remote:19531".to_string()),
                None,
            );
            assert_eq!(
                reader.transport,
                JournaldTransport::UnixSocket {
                    path: std::path::PathBuf::from("/run/systemd/journal/gatewayd.sock")
                }
            );
        });
        assert!(
            logs.contains("ignoring SHUUL_JOURNALD_URL"),
            "the ambiguous configuration must be reported once; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_socket_only_does_not_warn() {
        let logs = capture_logs(|| {
            let _ = JournaldReader::from_values(
                None,
                None,
                Some("/run/gw.sock".to_string()),
                None,
                None,
            );
        });
        assert!(
            !logs.contains("SHUUL_JOURNALD_URL"),
            "no warning is due when only the socket is set; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_blank_values_are_equivalent_to_absent() {
        let reader = JournaldReader::from_values(
            None,
            None,
            Some("   ".to_string()),
            Some(String::new()),
            Some("\t\n".to_string()),
        );
        assert_eq!(
            reader.transport,
            tcp(DEFAULT_GATEWAY_URL),
            "blank variables must fall back to the default TCP transport"
        );
        assert_eq!(
            reader.container_name, DEFAULT_CONTAINER,
            "a blank SHUUL_JOURNALD_CONTAINER must fall back to the default"
        );
    }

    #[test]
    fn test_from_values_blank_socket_falls_back_to_url() {
        let reader = JournaldReader::from_values(
            None,
            None,
            Some("  ".to_string()),
            Some("http://remote:19531".to_string()),
            None,
        );
        assert_eq!(
            reader.transport,
            tcp("http://remote:19531"),
            "a blank socket must not shadow a valid URL"
        );
    }

    // ── from_env ──

    const ENV_VARS: [&str; 6] = [
        "SHUUL_DOCKER_SOCKET",
        "SHUUL_DOCKER_URL",
        "SHUUL_DOCKER_CONTAINER",
        "SHUUL_JOURNALD_SOCKET",
        "SHUUL_JOURNALD_URL",
        "SHUUL_JOURNALD_CONTAINER",
    ];

    /// Clears the three variables and restores them when dropped, even on
    /// panic. The owning test must be `#[serial]` so no other test observes
    /// the mutation.
    struct ScopedEnv {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl ScopedEnv {
        fn new() -> Self {
            let saved = ENV_VARS
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect();
            for name in ENV_VARS {
                // SAFETY: the calling test is `#[serial]` and no other test
                // reads or writes these variables.
                unsafe { std::env::remove_var(name) };
            }
            Self { saved }
        }

        fn set(self, name: &'static str, value: &str) -> Self {
            assert!(
                ENV_VARS.contains(&name),
                "{name} is not managed by ScopedEnv"
            );
            // SAFETY: see `ScopedEnv::new`.
            unsafe { std::env::set_var(name, value) };
            self
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                // SAFETY: see `ScopedEnv::new`.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                };
            }
        }
    }

    #[test]
    #[serial]
    fn test_from_env_defaults_when_nothing_is_set() {
        let _env = ScopedEnv::new();
        let reader = JournaldReader::from_env();
        assert_eq!(reader.transport, tcp(DEFAULT_GATEWAY_URL));
        assert_eq!(reader.container_name, DEFAULT_CONTAINER);
    }

    #[test]
    #[serial]
    fn test_from_env_reads_all_three_variables() {
        let _env = ScopedEnv::new()
            .set("SHUUL_JOURNALD_SOCKET", "/run/custom/gatewayd.sock")
            .set("SHUUL_JOURNALD_URL", "http://ignored:19531")
            .set("SHUUL_JOURNALD_CONTAINER", "my-traefik");

        let reader = JournaldReader::from_env();
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: std::path::PathBuf::from("/run/custom/gatewayd.sock")
            },
            "SHUUL_JOURNALD_SOCKET must win over SHUUL_JOURNALD_URL"
        );
        assert_eq!(reader.container_name, "my-traefik");
    }

    #[test]
    #[serial]
    fn test_from_env_uses_url_when_the_socket_is_blank() {
        let _env = ScopedEnv::new()
            .set("SHUUL_JOURNALD_SOCKET", "   ")
            .set("SHUUL_JOURNALD_URL", "http://remote:19531")
            .set("SHUUL_JOURNALD_CONTAINER", "   ");

        let reader = JournaldReader::from_env();
        assert_eq!(reader.transport, tcp("http://remote:19531"));
        assert_eq!(
            reader.container_name, DEFAULT_CONTAINER,
            "a blank container variable is equivalent to it being absent"
        );
    }

    #[test]
    #[serial]
    fn test_from_env_blank_variables_are_equivalent_to_absent() {
        let _env = ScopedEnv::new()
            .set("SHUUL_JOURNALD_SOCKET", "")
            .set("SHUUL_JOURNALD_URL", "")
            .set("SHUUL_JOURNALD_CONTAINER", "");

        let reader = JournaldReader::from_env();
        assert_eq!(reader.transport, tcp(DEFAULT_GATEWAY_URL));
        assert_eq!(reader.container_name, DEFAULT_CONTAINER);
    }

    // ── the container of the Docker backend (decision D1) ──

    #[test]
    #[serial]
    fn test_from_env_prefers_the_docker_container_variable() {
        let _env = ScopedEnv::new()
            .set("SHUUL_DOCKER_CONTAINER", "my-traefik")
            .set("SHUUL_JOURNALD_CONTAINER", "legacy-traefik");

        let reader = JournaldReader::from_env();
        assert_eq!(
            reader.container_name, "my-traefik",
            "SHUUL_DOCKER_CONTAINER must win over SHUUL_JOURNALD_CONTAINER: whoever defines it \
             can point the two backends at different containers (design D1)"
        );
    }

    #[test]
    #[serial]
    fn test_from_env_falls_back_to_the_journald_container_variable() {
        let _env = ScopedEnv::new().set("SHUUL_JOURNALD_CONTAINER", "legacy-traefik");

        let reader = JournaldReader::from_env();
        assert_eq!(
            reader.container_name, "legacy-traefik",
            "without SHUUL_DOCKER_CONTAINER the existing variable keeps pointing at the same \
             container when switching backends (design D1), instead of silently falling back \
             to the default"
        );
    }

    // ── trimming and base path ──

    #[test]
    fn test_from_values_trims_surrounding_whitespace() {
        let reader = JournaldReader::from_values(
            None,
            None,
            Some("  /run/systemd/journal/gatewayd.sock  ".to_string()),
            None,
            Some("  my-traefik  ".to_string()),
        );
        assert_eq!(
            reader.transport,
            JournaldTransport::UnixSocket {
                path: std::path::PathBuf::from("/run/systemd/journal/gatewayd.sock")
            },
            "a socket path with stray spaces must be trimmed, not passed to connect()"
        );
        assert_eq!(reader.container_name, "my-traefik");

        let reader = JournaldReader::from_values(
            None,
            None,
            None,
            Some(" http://remote:19531 ".to_string()),
            None,
        );
        assert_eq!(
            reader.transport,
            tcp("http://remote:19531"),
            "the URL must be stored trimmed"
        );
    }

    #[test]
    fn test_from_values_warns_and_hides_an_ignored_base_path() {
        let logs = capture_logs(|| {
            let reader = JournaldReader::from_values(
                None,
                None,
                None,
                Some("http://proxy:19531/journald".to_string()),
                None,
            );
            assert_eq!(
                reader.endpoint_display(),
                "http://proxy:19531",
                "logs must not show a path the request never uses"
            );
        });
        assert!(
            logs.contains("ignoring base path /journald"),
            "the ignored path must be reported once; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_does_not_warn_for_a_plain_url() {
        let logs = capture_logs(|| {
            let _ = JournaldReader::from_values(
                None,
                None,
                None,
                Some("http://proxy:19531".to_string()),
                None,
            );
        });
        assert!(
            !logs.contains("ignoring base path"),
            "a URL without a path must not warn; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_warns_and_hides_an_ignored_docker_base_path() {
        let logs = capture_logs(|| {
            let reader = JournaldReader::from_values(
                None,
                Some("http://proxy:2375/base".to_string()),
                None,
                None,
                None,
            );
            assert_eq!(
                reader.endpoint_display(),
                "docker:http://proxy:2375 (traefik)",
                "the logs must show the effective endpoint, not a path the request never uses"
            );

            // El escenario promete dos THEN más sobre la **petición**: que va en
            // forma origen —como el hermano de journald— y que su Host es la
            // autoridad, sin el path base.
            let request = reader
                .build_request(None, CUTOFF_MICROS)
                .expect("the Docker request must build");
            assert_eq!(
                request.uri().to_string(),
                "/v1.44/containers/traefik/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1\
                 &since=1792571467",
                "the request must go in origin form, without the base path the URL carried"
            );
            assert_eq!(
                request.headers()[header::HOST],
                "proxy:2375",
                "the Host header must be the authority only, without the ignored base path"
            );
        });
        assert!(
            logs.contains("ignoring base path /base") && logs.contains("SHUUL_DOCKER_URL"),
            "the ignored path must be reported once, naming SHUUL_DOCKER_URL so the operator \
             knows which variable to fix; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_does_not_warn_for_a_plain_docker_url() {
        let logs = capture_logs(|| {
            let _ = JournaldReader::from_values(
                None,
                Some("http://proxy:2375".to_string()),
                None,
                None,
                None,
            );
        });
        assert!(
            !logs.contains("ignoring base path"),
            "a URL without a path must not warn; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_warns_about_the_ignored_docker_url() {
        let logs = capture_logs(|| {
            let reader = JournaldReader::from_values(
                Some("/var/run/docker.sock".to_string()),
                Some("http://docker-socket-proxy:2375".to_string()),
                None,
                None,
                None,
            );
            assert_eq!(
                reader.transport,
                JournaldTransport::DockerUnixSocket {
                    path: std::path::PathBuf::from("/var/run/docker.sock")
                },
                "with both Docker variables set the socket wins"
            );
        });
        assert!(
            logs.contains("ignoring SHUUL_DOCKER_URL"),
            "the Docker URL that is not used must be reported once, naming the variable, or the \
             operator cannot tell which of the two is in force; logs were: {logs:?}"
        );
    }

    // ── log capture helper ──

    #[derive(Clone, Default)]
    pub(super) struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl CapturedLog {
        /// Lo capturado hasta ahora, como texto.
        pub(super) fn captured(&self) -> String {
            let bytes = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            String::from_utf8_lossy(&bytes).to_string()
        }
    }

    impl std::io::Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Igual que [`capture_logs`], pero **sin** closure: instala el subscriber en
    /// este hilo y devuelve el sumidero junto al guard.
    ///
    /// Hace falta para los tests en los que el trabajo sigue después de montarlo:
    /// `capture_logs` sólo sirve para trabajo síncrono y terminaría antes de que
    /// el reader hiciera su primer `warn!`. Con el runtime `current_thread` de
    /// `#[tokio::test]`, la tarea del reader corre en este mismo hilo y sus
    /// eventos llegan a este sumidero.
    pub(super) fn capture_logs_while_running() -> (CapturedLog, tracing::subscriber::DefaultGuard) {
        let sink = CapturedLog::default();
        let writer = sink.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();

        let guard = tracing::subscriber::set_default(subscriber);
        (sink, guard)
    }

    /// Runs `f` with a WARN-level subscriber that records every event, and
    /// returns what was written.
    fn capture_logs<F: FnOnce()>(f: F) -> String {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let writer = CapturedLog(Arc::clone(&sink));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .without_time()
            .with_writer(move || CapturedLog(Arc::clone(&writer.0)))
            .finish();

        tracing::subscriber::with_default(subscriber, f);

        let bytes = sink.lock().unwrap_or_else(|e| e.into_inner()).clone();
        String::from_utf8_lossy(&bytes).to_string()
    }
}

//! # Journald Reader
//!
//! Lee en streaming los access logs de Traefik desde `systemd-journal-gatewayd`
//! (HTTP), los parsea a `ReportPayload` y los inyecta en el pipeline Jail.
//!
//! ## Transportes
//!
//! El mismo protocolo HTTP se sirve sobre dos transportes:
//!
//! - **TCP** contra `http://host:puerto` (`SHUUL_JOURNALD_URL`).
//! - **Socket Unix** `AF_UNIX` (`SHUUL_JOURNALD_SOCKET`), el preferido cuando
//!   gatewayd corre en el mismo host.
//!
//! El socket de `/run/systemd/journal/socket` NO se usa: es write-only. El
//! contenido sólo se sirve por la API HTTP de gatewayd.
//!
//! ## Protocolo
//!
//! 1. `GET /entries?follow&CONTAINER_NAME={container_name}` con
//!    `Accept: application/json` (Journal JSON Format, NDJSON) y `Host`
//! 2. Cada línea del cuerpo es un objeto JSON con los campos del journal
//! 3. Filtra por `CONTAINER_NAME`
//! 4. Parsea el campo `MESSAGE` como JSON de access log de Traefik
//! 5. Construye un `ReportPayload` y lo envía al pipeline Jail
//!
//! ## Concurrencia
//!
//! El reader corre como background task en main.rs. No comparte estado mutable
//! más allá del `AppState` que recibe para inyectar payloads en el Jail pipeline.

use crate::http::report::run_jail_pipeline;
use crate::models::{AppState, ReportPayload};
use http::{Method, Request, header};
use http_body_util::{BodyExt, Empty};
use hyper::body::{Bytes, Incoming};
use hyper::client::conn::http1;
use hyper::{Response, Uri};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UnixStream};
use tracing::{debug, info, warn};

/// Errors that can occur in the journald reader.
#[derive(Debug, Error)]
pub enum JournalError {
    /// Error de E/S al conectar o leer del gateway
    #[error("Socket error: {0}")]
    Socket(String),

    /// Error al parsear el formato nativo del journal
    #[error("Journal entry parse error: {0}")]
    Parse(String),

    /// Error al parsear el JSON del access log de Traefik
    #[error("Traefik access log parse error: {0}")]
    TraefikParse(String),

    /// Error de conexión (gateway no disponible)
    #[error("Connection error: {0}")]
    Connection(String),
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
    /// precedencia: socket → URL → default.
    ///
    /// Los valores ausentes o en blanco se consideran no definidos. Si llegan
    /// socket y URL a la vez, gana el socket y se avisa una sola vez.
    ///
    /// Está separado de [`JournaldReader::from_env`] para poder probarlo sin
    /// tocar las variables de entorno del proceso.
    pub fn from_values(
        socket: Option<String>,
        url: Option<String>,
        container_name: Option<String>,
    ) -> Self {
        let socket = socket
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let url = url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let container_name = container_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_CONTAINER_NAME.to_string());

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
                warn_ignored_base_path(&url);
                Self::new(&url, &container_name)
            },
            (None, None) => Self::new(DEFAULT_GATEWAY_URL, &container_name),
        }
    }

    /// Construye un reader desde `SHUUL_JOURNALD_SOCKET`,
    /// `SHUUL_JOURNALD_URL` y `SHUUL_JOURNALD_CONTAINER`, con los defaults
    /// documentados.
    pub fn from_env() -> Self {
        Self::from_values(
            std::env::var("SHUUL_JOURNALD_SOCKET").ok(),
            std::env::var("SHUUL_JOURNALD_URL").ok(),
            std::env::var("SHUUL_JOURNALD_CONTAINER").ok(),
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

/// Espera por defecto entre reintentos cuando el gateway no está disponible.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(30);

/// URL por defecto del endpoint de `systemd-journal-gatewayd`.
pub const DEFAULT_GATEWAY_URL: &str = "http://localhost:19531";

/// Contenedor por defecto cuyos access logs se leen.
pub const DEFAULT_CONTAINER_NAME: &str = "traefik";

/// Tiempo máximo para conectar, completar el handshake HTTP/1.1 y obtener la
/// cabecera de respuesta del gateway.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
                Err(e) => {
                    warn!(
                        "journald gateway not available: {e} (retrying in {}s)",
                        self.retry_delay.as_secs()
                    );
                },
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
        // Bound with a name on purpose: `_` would drop the ConnectionTask
        // immediately and stop the body from ever being driven.
        let (_connection, response) = tokio::select! {
            biased;

            // A shutdown must not wait for CONNECT_TIMEOUT to elapse.
            () = wait_for_shutdown(shutdown) => return Ok(StreamOutcome::Shutdown),

            response = self.send_request(last_cursor.as_deref()) => response?,
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
            return Err(JournalError::Connection(format!(
                "gateway returned HTTP {status}"
            )));
        }

        info!(
            "journald reader: streaming access logs from {}",
            self.endpoint_display()
        );

        let mut body = response.into_body();
        let mut buffer: Vec<u8> = Vec::new();
        loop {
            let frame = tokio::select! {
                biased;

                () = wait_for_shutdown(shutdown) => return Ok(StreamOutcome::Shutdown),

                frame = body.frame() => frame,
            };

            let Some(frame) = frame else {
                return Ok(StreamOutcome::Disconnected);
            };
            let frame = frame.map_err(|e| JournalError::Socket(e.to_string()))?;

            // Trailer frames carry no NDJSON, so only data frames are appended.
            let Some(data) = frame.data_ref() else {
                continue;
            };

            buffer.extend_from_slice(data);
            for line in drain_ndjson_lines(&mut buffer) {
                let entry = match parse_journal_entry(&line) {
                    Ok(entry) => entry,
                    Err(e) => {
                        debug!("journald reader: skipping unsuitable journal line: {e}");
                        continue;
                    },
                };

                if let Some(cursor) = entry.get("__CURSOR") {
                    *last_cursor = Some(cursor.clone());
                }

                self.process_entry(&entry, app_state).await;
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
    ) -> Result<(ConnectionTask, Response<Incoming>), JournalError> {
        let request = self.build_request(cursor)?;
        let endpoint = self.endpoint_display();

        let (connection_task, response) = tokio::time::timeout(CONNECT_TIMEOUT, async {
            let io = self.connect().await?;
            let (mut sender, connection) =
                http1::handshake(TokioIo::new(io)).await.map_err(|e| {
                    JournalError::Connection(format!("HTTP handshake with {endpoint} failed: {e}"))
                })?;

            let connection_task = ConnectionTask::spawn(connection);

            let response = sender.send_request(request).await.map_err(|e| {
                JournalError::Connection(format!("request to {endpoint} failed: {e}"))
            })?;

            Ok::<_, JournalError>((connection_task, response))
        })
        .await
        .map_err(|_| {
            JournalError::Connection(format!(
                "connecting to {endpoint} timed out after {}s",
                CONNECT_TIMEOUT.as_secs()
            ))
        })??;

        Ok((connection_task, response))
    }

    /// Abre el socket correspondiente al transporte configurado.
    async fn connect(&self) -> Result<BoxedIo, JournalError> {
        match &self.transport {
            JournaldTransport::UnixSocket { path } => {
                let stream = UnixStream::connect(path).await.map_err(|e| {
                    JournalError::Connection(format!(
                        "failed to connect to unix:{}: {e}",
                        path.display()
                    ))
                })?;
                Ok(Box::new(stream))
            },
            JournaldTransport::Tcp { base_url } => {
                let endpoint = tcp_endpoint(base_url)?;
                let stream = TcpStream::connect((endpoint.connect_host.as_str(), endpoint.port))
                    .await
                    .map_err(|e| {
                        JournalError::Connection(format!(
                            "failed to connect to {}: {e}",
                            endpoint.authority()
                        ))
                    })?;
                Ok(Box::new(stream))
            },
        }
    }

    /// Construye la petición en forma origen, idéntica en ambos transportes.
    fn build_request(&self, cursor: Option<&str>) -> Result<Request<Empty<Bytes>>, JournalError> {
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

        builder
            .body(Empty::<Bytes>::new())
            .map_err(|e| JournalError::Connection(format!("failed to build the request: {e}")))
    }

    /// Valor de la cabecera `Host`: `host:puerto` en TCP (con corchetes si es
    /// IPv6, como exige el RFC) y `localhost` en el socket Unix (donde no hay
    /// autoridad real).
    fn host_header(&self) -> Result<String, JournalError> {
        match &self.transport {
            JournaldTransport::Tcp { base_url } => Ok(tcp_endpoint(base_url)?.authority()),
            JournaldTransport::UnixSocket { .. } => Ok("localhost".to_string()),
        }
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
        JournalError::Connection(format!("invalid gateway URL {base_url}: {e}{hint}"))
    })?;

    match uri.scheme_str() {
        Some("http") => {},
        Some("https") => {
            return Err(JournalError::Connection(format!(
                "https is not supported for the gateway URL ({base_url}); use http or a unix socket"
            )));
        },
        Some(other) => {
            return Err(JournalError::Connection(format!(
                "unsupported scheme {other} in gateway URL {base_url}"
            )));
        },
        None => {
            return Err(JournalError::Connection(format!(
                "gateway URL {base_url} has no scheme{hint}"
            )));
        },
    }

    let host = uri
        .host()
        .ok_or_else(|| JournalError::Connection(format!("gateway URL {base_url} has no host")))?;
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

/// Avisa de que el path de la URL se ignora, para que un `SHUUL_JOURNALD_URL`
/// con prefijo (`http://proxy/journald`) no acabe en un 404 silencioso.
fn warn_ignored_base_path(url: &str) {
    if let Ok(uri) = url.parse::<Uri>() {
        let path = uri.path();
        if !path.is_empty() && path != "/" {
            warn!(
                "journald reader: ignoring base path {path} in SHUUL_JOURNALD_URL={url}: \
                 requests always use {}",
                "/entries"
            );
        }
    }
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
        return Err(JournalError::Parse("empty journal entry".to_string()));
    }

    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| JournalError::Parse(format!("invalid journal JSON: {e}")))?;

    let serde_json::Value::Object(object) = value else {
        return Err(JournalError::Parse(
            "journal entry is not a JSON object".to_string(),
        ));
    };

    let mut fields = HashMap::new();
    for (key, value) in object {
        if let Some(text) = journal_field_value(&value) {
            fields.insert(key, text);
        }
    }

    if fields.is_empty() {
        return Err(JournalError::Parse(
            "no usable fields in journal entry".to_string(),
        ));
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
        let err_socket = JournalError::Socket("connection refused".to_string());
        assert!(
            err_socket.to_string().contains("connection refused"),
            "Socket should contain the message"
        );

        let err_parse = JournalError::Parse("invalid format".to_string());
        assert!(
            err_parse.to_string().contains("invalid format"),
            "Parse should contain the message"
        );

        let err_traefik = JournalError::TraefikParse("bad json".to_string());
        assert!(
            err_traefik.to_string().contains("bad json"),
            "TraefikParse should contain the message"
        );

        let err_conn = JournalError::Connection("socket not found".to_string());
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
            Err(super::JournalError::Parse(msg)) => {
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
        .map_err(|e| JournalError::TraefikParse(format!("invalid JSON: {e}")))?;

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
            Err(super::JournalError::TraefikParse(msg)) => {
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
            Err(JournalError::TraefikParse(msg)) => {
                assert!(!msg.is_empty(), "error message should not be empty");
            },
            other => panic!("expected TraefikParse error, got: {other:?}"),
        }
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
    use super::{CONNECT_TIMEOUT, JournalError, JournaldReader, drain_ndjson_lines, tcp_endpoint};
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

        let request = reader.build_request(None).expect("request must build");
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

    /// Builds a journal NDJSON line that carries a Traefik access log.
    fn journal_line(cursor: Option<&str>, status: u16, path: &str) -> String {
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
        entry.insert("CONTAINER_NAME".to_string(), serde_json::json!("traefik"));
        entry.insert("MESSAGE".to_string(), serde_json::json!(message));
        serde_json::Value::Object(entry).to_string()
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
    async fn spawn_fake_gateway(body: &'static str) -> String {
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

        let body = "{\"__REALTIME_TIMESTAMP\":\"1723897600123456\",\"CONTAINER_NAME\":\"traefik\",\"MESSAGE\":\"{\\\"ClientHost\\\":\\\"10.0.0.1\\\",\\\"OriginStatus\\\":404,\\\"RequestPath\\\":\\\"/api/login\\\",\\\"RequestMethod\\\":\\\"GET\\\",\\\"RequestHost\\\":\\\"example.com\\\"}\"}\n";
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
    async fn spawn_recording_gateway(
        bodies: Vec<&'static str>,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
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

        let message = serde_json::json!({
            "ClientHost": "10.0.0.1",
            "OriginStatus": 404,
            "RequestPath": "/x",
            "RequestMethod": "GET",
            "RequestHost": "example.com",
        })
        .to_string();

        let line = serde_json::json!({
            "__CURSOR": CURSOR,
            "CONTAINER_NAME": "traefik",
            "MESSAGE": message,
        })
        .to_string();

        let body: &'static str = Box::leak(format!("{line}\n").into_boxed_str());
        let (gateway_url, recorded) = spawn_recording_gateway(vec![body, ""]).await;

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

    #[tokio::test]
    async fn test_tcp_request_uses_origin_form_and_required_headers() {
        let (gateway_url, recorded) = spawn_recording_gateway(vec![""]).await;
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
            Err(JournalError::Connection(msg)) => assert!(
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
            Err(JournalError::Connection(msg)) => {
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
            matches!(reader.connect().await, Err(JournalError::Connection(_))),
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
}

// ── Task 8: configuration from environment ──

#[cfg(test)]
mod tests_task8 {
    use super::{DEFAULT_GATEWAY_URL, JournaldReader, JournaldTransport};
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
        let reader = JournaldReader::from_values(None, None, None);
        assert_eq!(reader.transport, tcp(DEFAULT_GATEWAY_URL));
        assert_eq!(reader.container_name, DEFAULT_CONTAINER);
    }

    #[test]
    fn test_from_values_honours_url_and_container() {
        let reader = JournaldReader::from_values(
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
            let _ = JournaldReader::from_values(Some("/run/gw.sock".to_string()), None, None);
        });
        assert!(
            !logs.contains("SHUUL_JOURNALD_URL"),
            "no warning is due when only the socket is set; logs were: {logs:?}"
        );
    }

    #[test]
    fn test_from_values_blank_values_are_equivalent_to_absent() {
        let reader = JournaldReader::from_values(
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

    const ENV_VARS: [&str; 3] = [
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

    // ── trimming and base path ──

    #[test]
    fn test_from_values_trims_surrounding_whitespace() {
        let reader = JournaldReader::from_values(
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

        let reader =
            JournaldReader::from_values(None, Some(" http://remote:19531 ".to_string()), None);
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
            let _ = JournaldReader::from_values(None, Some("http://proxy:19531".to_string()), None);
        });
        assert!(
            !logs.contains("ignoring base path"),
            "a URL without a path must not warn; logs were: {logs:?}"
        );
    }

    // ── log capture helper ──

    #[derive(Clone, Default)]
    struct CapturedLog(Arc<Mutex<Vec<u8>>>);

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

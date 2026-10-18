//! # Journald Reader
//!
//! Lee en streaming los access logs de Traefik desde `systemd-journal-gatewayd`
//! (HTTP), los parsea a `ReportPayload` y los inyecta en el pipeline Jail.
//!
//! ## Protocolo
//!
//! 1. `GET {gateway_url}/entries?follow&CONTAINER_NAME={container_name}`
//!    con `Accept: application/json` (Journal JSON Format, NDJSON)
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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
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

/// Lector de access logs desde `systemd-journal-gatewayd`.
///
/// Hace streaming NDJSON contra el endpoint `/entries?follow` del gateway,
/// filtra las entradas por contenedor y las convierte en `ReportPayload`
/// para el pipeline Jail.
pub struct JournaldReader {
    /// URL base del endpoint HTTP de `systemd-journal-gatewayd`
    /// (default: <http://localhost:19531>)
    pub gateway_url: String,
    /// Nombre del contenedor Docker a filtrar (default: "traefik")
    pub container_name: String,
    /// Espera entre reintentos cuando el gateway no está disponible
    /// (default: [`DEFAULT_RETRY_DELAY`])
    pub retry_delay: Duration,
}

impl JournaldReader {
    /// Crea un reader con configuración personalizada.
    pub fn new(gateway_url: &str, container_name: &str) -> Self {
        Self {
            gateway_url: gateway_url.to_string(),
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

/// Espera por defecto entre reintentos cuando el gateway no está disponible.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(30);

/// URL por defecto del endpoint de `systemd-journal-gatewayd`.
pub const DEFAULT_GATEWAY_URL: &str = "http://localhost:19531";

/// Contenedor por defecto cuyos access logs se leen.
pub const DEFAULT_CONTAINER_NAME: &str = "traefik";

impl JournaldReader {
    /// Construye un reader a partir de valores ya resueltos, aplicando los
    /// defaults cuando falten o estén en blanco.
    ///
    /// Está separado de [`JournaldReader::from_env`] para poder probarlo sin
    /// tocar las variables de entorno del proceso.
    pub fn from_values(gateway_url: Option<String>, container_name: Option<String>) -> Self {
        let gateway_url = gateway_url
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string());
        let container_name = container_name
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_CONTAINER_NAME.to_string());
        Self::new(&gateway_url, &container_name)
    }

    /// Construye un reader desde `SHUUL_JOURNALD_URL` y
    /// `SHUUL_JOURNALD_CONTAINER`, con los defaults documentados.
    pub fn from_env() -> Self {
        Self::from_values(
            std::env::var("SHUUL_JOURNALD_URL").ok(),
            std::env::var("SHUUL_JOURNALD_CONTAINER").ok(),
        )
    }

    /// URL del endpoint de streaming de gatewayd.
    pub fn entries_url(&self) -> String {
        let base = self.gateway_url.trim_end_matches('/');
        format!(
            "{base}/entries?follow&CONTAINER_NAME={}",
            urlencoding::encode(&self.container_name)
        )
    }

    /// Bucle de streaming: conecta con gatewayd, recibe NDJSON y entrega cada
    /// access log de Traefik al pipeline Jail.
    ///
    /// Si el gateway no está disponible se registra un warning y se reintenta
    /// tras [`RETRY_DELAY`]. Termina limpiamente cuando `shutdown` pasa a
    /// `true`.
    ///
    /// # Errors
    ///
    /// Devuelve [`JournalError::Connection`] sólo si no se puede construir el
    /// cliente HTTP (configuración irrecuperable). La indisponibilidad del
    /// gateway es recuperable: se registra y se reintenta.
    pub async fn run(
        &self,
        app_state: Arc<AppState>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), JournalError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| JournalError::Connection(format!("failed to build HTTP client: {e}")))?;

        // Last cursor seen; survives reconnects so no entry is missed.
        let mut last_cursor: Option<String> = None;

        loop {
            if *shutdown.borrow() {
                info!("journald reader: shutdown requested");
                return Ok(());
            }

            match self
                .stream_once(&client, &app_state, &mut shutdown, &mut last_cursor)
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
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return Ok(());
                    }
                },
            }
        }
    }

    /// Abre una conexión de streaming y procesa las líneas que van llegando.
    async fn stream_once(
        &self,
        client: &reqwest::Client,
        app_state: &Arc<AppState>,
        shutdown: &mut tokio::sync::watch::Receiver<bool>,
        last_cursor: &mut Option<String>,
    ) -> Result<StreamOutcome, JournalError> {
        let url = self.entries_url();
        let mut request = client.get(&url).header("Accept", "application/json");

        if let Some(cursor) = last_cursor.as_deref() {
            // `entries=<cursor>:1` skips the cursor entry itself, so the stream
            // resumes right after the last entry already processed.
            request = request.header("Range", format!("entries={cursor}:1"));
        }

        let mut response = request
            .send()
            .await
            .map_err(|e| JournalError::Connection(e.to_string()))?;

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

        info!("journald reader: streaming access logs from {url}");

        let mut buffer: Vec<u8> = Vec::new();
        loop {
            let chunk = tokio::select! {
                biased;

                _ = shutdown.changed() => return Ok(StreamOutcome::Shutdown),

                chunk = response.chunk() => {
                    chunk.map_err(|e| JournalError::Socket(e.to_string()))?
                },
            };

            let Some(chunk) = chunk else {
                return Ok(StreamOutcome::Disconnected);
            };

            buffer.extend_from_slice(&chunk);
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
    use super::{JournalError, JournaldReader, parse_journal_entry};

    // ── Task 1: Struct existence and default values ──

    #[test]
    fn test_journald_reader_default_values() {
        let reader = JournaldReader::default();
        assert_eq!(
            reader.gateway_url, "http://localhost:19531",
            "Default gateway URL should be http://localhost:19531"
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
            reader.gateway_url, "http://gateway.local:19531",
            "Custom gateway URL should be stored"
        );
        assert_eq!(
            reader.container_name, "nginx",
            "Custom container name should be stored"
        );
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
    use super::{JournaldReader, drain_ndjson_lines};
    use crate::models::log_collector::LOG_COLLECTOR;
    use crate::models::{AppState, BanManager, GeoIpService, Settings, StatsCollector, TorService};
    use maxminddb::Reader;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    // ── entries_url ──

    #[test]
    fn test_entries_url_default() {
        let reader = JournaldReader::default();
        assert_eq!(
            reader.entries_url(),
            "http://localhost:19531/entries?follow&CONTAINER_NAME=traefik"
        );
    }

    #[test]
    fn test_entries_url_trims_trailing_slash() {
        let reader = JournaldReader::new("http://gateway.local:19531/", "my traefik");
        assert_eq!(
            reader.entries_url(),
            "http://gateway.local:19531/entries?follow&CONTAINER_NAME=my%20traefik",
            "trailing slash must be trimmed and the container name URL-encoded"
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

        // Wait for the entry to be processed (or give up after ~2s)
        let mut found = false;
        for _ in 0..40 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let entries = LOG_COLLECTOR.lock().map(|c| c.all()).unwrap_or_default();
            if entries
                .iter()
                .any(|e| e.event == "cleared" && e.status_code == Some(404))
            {
                found = true;
                break;
            }
        }

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
                let mut request = vec![0_u8; 8192];
                let read = stream.read(&mut request).await.unwrap_or(0);
                if let Ok(mut sink) = sink.lock() {
                    sink.push(String::from_utf8_lossy(&request[..read]).to_string());
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
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;

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
}

// ── Task 8: configuration from environment ──

#[cfg(test)]
mod tests_task8 {
    use super::JournaldReader;

    const DEFAULT_URL: &str = "http://localhost:19531";
    const DEFAULT_CONTAINER: &str = "traefik";

    #[test]
    fn test_from_values_uses_defaults_when_absent() {
        let reader = JournaldReader::from_values(None, None);
        assert_eq!(reader.gateway_url, DEFAULT_URL);
        assert_eq!(reader.container_name, DEFAULT_CONTAINER);
    }

    #[test]
    fn test_from_values_honours_overrides() {
        let reader = JournaldReader::from_values(
            Some("http://host.docker.internal:19531".to_string()),
            Some("my-traefik".to_string()),
        );
        assert_eq!(reader.gateway_url, "http://host.docker.internal:19531");
        assert_eq!(reader.container_name, "my-traefik");
    }

    #[test]
    fn test_from_values_blank_strings_fall_back_to_defaults() {
        let reader = JournaldReader::from_values(Some(String::new()), Some("   ".to_string()));
        assert_eq!(
            reader.gateway_url, DEFAULT_URL,
            "an empty SHUUL_JOURNALD_URL must not break the reader"
        );
        assert_eq!(
            reader.container_name, DEFAULT_CONTAINER,
            "a blank SHUUL_JOURNALD_CONTAINER must fall back to the default"
        );
    }
}

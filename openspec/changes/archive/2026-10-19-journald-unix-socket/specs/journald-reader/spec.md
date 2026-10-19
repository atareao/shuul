## MODIFIED Requirements

### Requirement: Journald reader transport

El módulo `journald_reader` SHALL obtener los access logs de Traefik desde
`systemd-journal-gatewayd` mediante HTTP en streaming, sobre uno de dos
transportes: **TCP** o **socket Unix `AF_UNIX`**.

NO SHALL leer directamente `/run/systemd/journal/socket`, porque ese socket es
**write-only** (las aplicaciones lo usan para *enviar* entradas al journal); el
contenido sólo se sirve a través de la API del journal, que gatewayd expone por
HTTP.

El transporte SHALL seleccionarse con esta precedencia:

1. Si `SHUUL_JOURNALD_SOCKET` está definida y no está en blanco → transporte
   **Unix**, conectando al path indicado.
2. Si no, si `SHUUL_JOURNALD_URL` está definida y no está en blanco → transporte
   **TCP** contra esa URL base.
3. Si no, transporte **TCP** contra `http://localhost:19531`.

Cuando ambas variables estén definidas y no vacías, SHALL usarse el socket y
SHALL registrarse un warning indicando que `SHUUL_JOURNALD_URL` se ignora.

La petición SHALL ser `GET /entries?follow&CONTAINER_NAME={container_name}` con
las cabeceras `Accept: application/json` (Journal JSON Format, NDJSON: un objeto
JSON por línea) y `Host`. El filtrado por contenedor SHALL delegarse en el
servidor mediante el parámetro `KEY=match`. La misma ruta de petición SHALL
usarse en ambos transportes; sólo cambia el transporte subyacente.

Ambos transportes SHALL compartir el mismo comportamiento de streaming,
reconexión, cursor, shutdown y fail-open.

```rust
pub enum JournaldTransport {
    Tcp { base_url: String },
    UnixSocket { path: PathBuf },
}

pub struct JournaldReader {
    pub transport: JournaldTransport,
    pub container_name: String,
    pub retry_delay: Duration,
}

pub const DEFAULT_GATEWAY_URL: &str = "http://localhost:19531";
pub const DEFAULT_CONTAINER_NAME: &str = "traefik";
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(30);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl JournaldReader {
    pub fn new(base_url: &str, container_name: &str) -> Self;
    pub fn unix(path: impl Into<PathBuf>, container_name: &str) -> Self;
    pub fn from_values(
        socket: Option<String>,
        url: Option<String>,
        container_name: Option<String>,
    ) -> Self;
    pub fn from_env() -> Self;
    pub fn request_target(&self) -> String;
    pub fn endpoint_display(&self) -> String;
    pub const fn with_retry_delay(self, retry_delay: Duration) -> Self;
    pub async fn run(
        &self,
        app_state: Arc<AppState>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), JournalError>;
}

impl Default for JournaldReader { /* gatewayd local por TCP + contenedor "traefik" */ }
```

#### Scenario: Stream entries from gatewayd
- **GIVEN** gatewayd escuchando en el transporte configurado
- **WHEN** `JournaldReader::run()` se ejecuta
- **THEN** hace `GET /entries?follow&CONTAINER_NAME={container_name}`
- **AND** envía las cabeceras `Accept: application/json` y `Host`
- **AND** lee el cuerpo en chunks sin bloquear
- **AND** procesa cada línea NDJSON completa, conservando las parciales hasta completarlas

#### Scenario: Socket transport selected from the environment
- **GIVEN** `SHUUL_JOURNALD_SOCKET=/run/systemd/journal/gatewayd.sock` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `UnixSocket` con ese path
- **AND** la URL por defecto no se usa

#### Scenario: Socket takes precedence over the URL
- **GIVEN** `SHUUL_JOURNALD_SOCKET` y `SHUUL_JOURNALD_URL` definidas y no vacías
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `UnixSocket` con el path de `SHUUL_JOURNALD_SOCKET`
- **AND** se registra un warning de que `SHUUL_JOURNALD_URL` se ignora

#### Scenario: Stream entries over a Unix socket
- **GIVEN** gatewayd escuchando en un socket `AF_UNIX` en el path configurado
- **WHEN** `JournaldReader::run()` se ejecuta
- **THEN** conecta al socket, envía la petición HTTP/1.1 y recibe el NDJSON
- **AND** cada access log de Traefik llega al pipeline Jail igual que por TCP

#### Scenario: Missing socket does not abort startup
- **GIVEN** `SHUUL_JOURNALD_SOCKET` apunta a un path que no existe
- **WHEN** `JournaldReader::run()` se ejecuta
- **THEN** se registra el warning `journald gateway not available` incluyendo el path
- **AND** se reintenta tras `retry_delay`
- **AND** el proceso no se cae y el WAF sigue funcionando

#### Scenario: Connect attempt times out
- **GIVEN** el endpoint acepta la conexión pero no responde al handshake HTTP
- **WHEN** transcurren `CONNECT_TIMEOUT` (10 s)
- **THEN** el intento se aborta
- **AND** se registra el warning `journald gateway not available`
- **AND** se reintenta tras `retry_delay`

#### Scenario: Graceful shutdown on stop signal
- **GIVEN** el reader está recibiendo entradas
- **WHEN** el canal `shutdown` pasa a `true`
- **THEN** el bucle de streaming termina
- **AND** `run()` devuelve `Ok(())`
- **AND** la espera entre reintentos también se interrumpe de inmediato

#### Scenario: Gateway not available
- **GIVEN** gatewayd NO está escuchando en el transporte configurado
- **WHEN** `JournaldReader::run()` se ejecuta
- **THEN** se registra un warning `journald gateway not available`
- **AND** se reintenta tras `retry_delay`
- **AND** `run()` no devuelve error ni se cae el proceso

#### Scenario: Resume from the last cursor after reconnect
- **GIVEN** el reader ya procesó entradas y guardó el último `__CURSOR`
- **WHEN** la conexión se cierra y el reader reconecta
- **THEN** repite la petición con la cabecera `Range: entries=<cursor>:1`
- **AND** no reprocesa la entrada del cursor
- **AND** el comportamiento es idéntico en ambos transportes

#### Scenario: Rejected cursor restarts the stream
- **GIVEN** el reader tiene un cursor guardado
- **AND** gatewayd responde con un status de error (p. ej. el journal se reinició y el cursor ya no existe)
- **WHEN** el reader procesa la respuesta
- **THEN** descarta el cursor guardado
- **AND** el siguiente intento empieza desde el principio del journal

#### Scenario: Configuration from environment
- **GIVEN** las variables `SHUUL_JOURNALD_SOCKET`, `SHUUL_JOURNALD_URL` y `SHUUL_JOURNALD_CONTAINER`
- **WHEN** se construye el reader con `from_env()`
- **THEN** usa sus valores cuando están definidos y no están en blanco
- **AND** cae en `http://localhost:19531` y `traefik` cuando faltan o están en blanco
- **AND** una variable en blanco equivale a no definirla

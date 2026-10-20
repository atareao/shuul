# journald-reader Specification

## Purpose
Define el módulo `journald_reader`, que obtiene en streaming los access logs de Traefik desde `systemd-journal-gatewayd` —por socket Unix o TCP— los mapea a `ReportPayload` y los entrega al pipeline Jail, la misma función que alimenta el endpoint HTTP `POST /api/v1/report`. Cubre el transporte seleccionable por entorno, el filtrado por contenedor, los diagnósticos de configuración y el corte que descarta el backlog previo al inicio del intento de conexión, de modo que el rate limiter nunca cuenta tráfico anterior al arranque.

## Requirements

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

### Requirement: Journal JSON entry parsing

`parse_journal_entry` SHALL convertir una línea del Journal JSON Format (NDJSON)
en un mapa campo→valor:

- los valores de tipo string se conservan tal cual;
- los campos multi-valor (array de strings) usan el primer elemento;
- los campos binarios (array de números), booleanos y `null` se ignoran.

```rust
pub fn parse_journal_entry(line: &str) -> Result<HashMap<String, String>, JournalError>;
```

#### Scenario: Parse a valid NDJSON journal entry
- **GIVEN** la línea `{"__REALTIME_TIMESTAMP":"1723897600123456","CONTAINER_NAME":"traefik","MESSAGE":"{...}"}`
- **WHEN** `parse_journal_entry()` se ejecuta
- **THEN** devuelve un mapa con `CONTAINER_NAME → "traefik"` y el `MESSAGE` crudo

#### Scenario: Multi-value field uses the first value
- **GIVEN** una línea con `"_UDEV_DEVLINK":["/dev/alias1","/dev/alias2"]`
- **WHEN** `parse_journal_entry()` se ejecuta
- **THEN** el campo vale `"/dev/alias1"`

#### Scenario: Binary and null fields are ignored
- **GIVEN** una línea con `"BINARY":[116,104,105,115]` y `"LARGE":null`
- **WHEN** `parse_journal_entry()` se ejecuta
- **THEN** ni `BINARY` ni `LARGE` aparecen en el mapa

#### Scenario: Malformed NDJSON line
- **GIVEN** una línea vacía, que no es JSON, o que es un JSON que no es un objeto
- **WHEN** `parse_journal_entry()` se ejecuta
- **THEN** devuelve `Err(JournalError::Parse)`
- **AND** el reader la descarta a nivel `debug` y sigue con la siguiente línea

### Requirement: Container filtering

`is_traefik_entry` SHALL comprobar que el campo `CONTAINER_NAME` de la entrada
coincide con el contenedor configurado. Es una segunda barrera defensiva: el
filtrado principal lo hace gatewayd en el servidor.

```rust
pub fn is_traefik_entry(entry: &HashMap<String, String>, container_name: &str) -> bool;
```

#### Scenario: Traefik entry is accepted
- **GIVEN** un mapa con `CONTAINER_NAME → "traefik"`
- **WHEN** `is_traefik_entry(&entry, "traefik")` se ejecuta
- **THEN** devuelve `true`

#### Scenario: Non-Traefik entry is discarded
- **GIVEN** un mapa con `CONTAINER_NAME → "nginx"`
- **WHEN** el reader procesa la entrada
- **THEN** `is_traefik_entry` devuelve `false`
- **AND** `extract_report_payload` devuelve `Ok(None)`
- **AND** no se invoca el pipeline Jail

#### Scenario: Entry without CONTAINER_NAME is discarded
- **GIVEN** un mapa sin el campo `CONTAINER_NAME`
- **WHEN** `is_traefik_entry(&entry, "traefik")` se ejecuta
- **THEN** devuelve `false`

### Requirement: Traefik access log mapping

`parse_traefik_access_log` SHALL convertir el `MESSAGE` de una entrada del
journal (el access log JSON de Traefik) en un `ReportPayload`.

```rust
pub fn parse_traefik_access_log(
    json_str: &str,
) -> Result<Option<ReportPayload>, JournalError>;
```

Mapeo de campos (nombres reales del formato JSON de Traefik v3):

| Campo Traefik (JSON) | ReportPayload | Notas |
|---|---|---|
| `ClientHost` | `ip_address` | IP remota. Fallback a `ClientAddr` sin el puerto (IPv4 e IPv6). |
| `OriginStatus` | `status_code` | Status devuelto **por el backend**. |
| `RequestPath` | `path` + `query` | Se divide en el primer `?`. |
| `RequestMethod` | `method` | |
| `RequestHost` | `fqdn` | Host sin puerto. |
| `RequestScheme` | `protocol` | `http` / `https`. |
| `request_User-Agent` | `user_agent` | Los headers llevan prefijo `request_`. |
| `request_Referer` | `referer` | |
| `request_Content-Type` | `content_type` | |
| `request_Accept-Language` | `accept_language` | |
| `request_X-Request-Id` | `x_request_id` | |

Traefik reporta dos status: `OriginStatus` (resultado del proxy al backend) y
`DownstreamStatus` (lo devuelto al cliente tras la cadena de middlewares). Según
la documentación de Traefik, cuando `OriginStatus` es `0` la request **nunca
llegó al backend** (la respuesta viene del routing o de un middleware:
autenticación, rate limiting, redirección). Por tanto el reader SHALL descartar
esas entradas, lo que preserva el comportamiento del plugin Go anterior (que
sólo se ejecutaba en requests que habían pasado el WAF forwardAuth).

#### Scenario: Parse a full Traefik access log
- **GIVEN** un access log con `ClientHost`, `OriginStatus: 401`, `RequestPath: "/api/login?next=%2Fdash"`, `RequestMethod: "POST"`, `RequestHost: "example.com"`, `RequestScheme: "https"` y los headers `request_User-Agent`, `request_Referer`, `request_Content-Type`, `request_Accept-Language`, `request_X-Request-Id`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` con `status_code = 401`, `path = "/api/login"`, `query = "next=%2Fdash"`, `protocol = "https"` y el resto de campos mapeados

#### Scenario: Missing optional fields become None
- **GIVEN** un access log con sólo `ClientHost`, `OriginStatus: 200`, `RequestPath: "/"`, `RequestMethod: "GET"` y `RequestHost`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` con `status_code = 200`
- **AND** `query`, `user_agent`, `referer`, `content_type`, `accept_language`, `x_request_id` y `protocol` son `None`

#### Scenario: Path without query
- **GIVEN** un access log con `RequestPath: "/missing"` (sin `?`)
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `path = "/missing"` y `query` es `None`

#### Scenario: ClientAddr fallback strips the port
- **GIVEN** un access log sin `ClientHost` pero con `ClientAddr: "192.168.1.5:54321"`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `ip_address = "192.168.1.5"`

#### Scenario: IPv6 ClientAddr fallback strips brackets and port
- **GIVEN** un access log con `ClientAddr: "[2001:db8::1]:443"` y sin `ClientHost`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `ip_address = "2001:db8::1"`

#### Scenario: Request never proxied to a backend is discarded
- **GIVEN** un access log con `OriginStatus: 0` y `DownstreamStatus: 403`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Missing OriginStatus is discarded
- **GIVEN** un access log sin el campo `OriginStatus`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Missing client IP is discarded
- **GIVEN** un access log sin `ClientHost` ni `ClientAddr`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Invalid JSON returns an error
- **GIVEN** un texto que no es JSON válido
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Err(JournalError::TraefikParse)`

### Requirement: Jail pipeline invocation from the journald reader

`extract_report_payload` SHALL combinar el filtro de contenedor, la extracción
del `MESSAGE` y el mapeo del access log, produciendo el `ReportPayload` listo
para el pipeline Jail.

El bucle del reader SHALL entregar cada payload a `run_jail_pipeline`, la misma
función que usa el endpoint HTTP `POST /api/v1/report`, de modo que ambos
caminos compartan exactamente la misma lógica de rate limiting, stats y
auditoría.

```rust
pub fn extract_report_payload(
    entry: &HashMap<String, String>,
    container_name: &str,
) -> Result<Option<ReportPayload>, JournalError>;

pub async fn run_jail_pipeline(
    app_state: &Arc<AppState>,
    payload: &ReportPayload,
) -> Result<(), AppError>;
```

#### Scenario: Extract payload from a Traefik journal entry
- **GIVEN** un mapa con `CONTAINER_NAME → "traefik"` y un `MESSAGE` con `OriginStatus: 401`
- **WHEN** `extract_report_payload(&entry, "traefik")` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` con `status_code = 401`

#### Scenario: Entry without MESSAGE is discarded
- **GIVEN** un mapa de Traefik sin campo `MESSAGE`
- **WHEN** `extract_report_payload()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: MESSAGE that is not an access log
- **GIVEN** un mapa de Traefik cuyo `MESSAGE` no es JSON válido (p. ej. una línea de arranque de Traefik con `log.format: common`)
- **WHEN** `extract_report_payload()` se ejecuta
- **THEN** devuelve `Err(JournalError::TraefikParse)`
- **AND** el bucle del reader lo registra a nivel `debug` y continúa con la siguiente línea

#### Scenario: Journal entry reaches the Jail pipeline
- **GIVEN** el reader activo y gatewayd sirviendo un access log de Traefik con `OriginStatus: 404`
- **WHEN** el reader procesa la entrada
- **THEN** parsea la línea NDJSON
- **AND** verifica `CONTAINER_NAME`
- **AND** extrae el `ReportPayload`
- **AND** llama a `run_jail_pipeline`
- **AND** el pipeline Jail produce su audit log correspondiente (`cleared`, `registered` o `pending`)

#### Scenario: report_handler delegates to run_jail_pipeline
- **GIVEN** `report_handler` recibe un `ReportPayload` por HTTP
- **WHEN** procesa la request
- **THEN** llama a `run_jail_pipeline`
- **AND** devuelve 200 OK, con el mismo comportamiento que antes del refactor

#### Scenario: run_jail_pipeline invoked without HTTP
- **GIVEN** un reporte con una regla Jail que matchea y un status_code en `fail_codes`
- **WHEN** se llama directamente a `run_jail_pipeline(&app_state, &payload)`
- **THEN** se registra el audit log correspondiente con su `status_code`
- **AND** si se excede el umbral se crea un `PendingBan`
- **AND** sin reglas que matcheen se registra `cleared`

### Requirement: Configuration diagnostics

Cuando la configuración del transporte no sea utilizable, el lector SHALL emitir un
diagnóstico que identifique el valor rechazado y, cuando exista una corrección evidente,
nombre la variable que lo aceptaría.

El diagnóstico NO SHALL romper el fail-open: el fallo se registra como warning y se
reintenta cada `retry_delay`; el proceso y el pipeline WAF no se ven afectados.

#### Scenario: Absolute path passed as the gateway URL
- **GIVEN** `SHUUL_JOURNALD_URL=/run/systemd/journal/gatewayd.sock`
- **WHEN** el lector construye el transporte a partir del entorno
- **THEN** el error incluye el valor recibido
- **AND** menciona `SHUUL_JOURNALD_SOCKET` como la variable que acepta ese valor
- **AND** se registra como warning y se reintenta, sin abortar el arranque

#### Scenario: URL without a scheme that is not a path
- **GIVEN** `SHUUL_JOURNALD_URL=localhost:19531`
- **WHEN** el lector construye el transporte a partir del entorno
- **THEN** el error indica que falta el esquema
- **AND** NO menciona `SHUUL_JOURNALD_SOCKET`, porque la pista sería engañosa

#### Scenario: Unreadable socket
- **GIVEN** `SHUUL_JOURNALD_SOCKET` apunta a un socket existente sin permiso de conexión
- **WHEN** el lector intenta conectar
- **THEN** el warning nombra el path y el error del sistema
- **AND** el pipeline WAF sigue funcionando

### Requirement: No history replay

El lector SHALL alimentar el pipeline Jail únicamente con entradas posteriores
al inicio del intento de conexión en curso.

Para cada entrada el lector SHALL:

- descartarla si su `__REALTIME_TIMESTAMP` es anterior al instante en que se
  inició el intento de conexión;
- descartarla si `__REALTIME_TIMESTAMP` falta o no es un entero decimal; una
  entrada sin instante no se puede situar respecto al corte y se descarta por
  precaución;
- descartarla si su `__CURSOR` coincide con el último `__CURSOR` ya procesado,
  porque el stream puede reanudarse en esa misma entrada;
- alimentar el pipeline Jail en caso contrario.

El descarte SHALL ser silencioso a nivel `INFO` (a lo sumo `DEBUG`) y NO SHALL
contarse como error.

Este filtrado NO SHALL depender de la cabecera `Range`: gatewayd no garantiza
el `skip` de `entries=<cursor>:1` sin `num_entries`, ni aplica `realtime` cuando
la petición lleva `follow`.

#### Scenario: Entries older than the connection attempt are ignored
- **GIVEN** el journal contiene access logs de Traefik con fallos de días anteriores
- **WHEN** el reader arranca y abre la conexión
- **THEN** esas entradas se leen pero NO se entregan al pipeline Jail
- **AND** no se registra ningún fallo en el rate limiter por ellas
- **AND** no se crea ningún baneo a partir de ellas

#### Scenario: Live entries after startup are processed
- **GIVEN** el reader está conectado y ha fijado el corte del intento actual
- **WHEN** llega un access log con `__REALTIME_TIMESTAMP` posterior al corte
- **THEN** la entrada se entrega al pipeline Jail con normalidad

#### Scenario: Reconnect does not reprocess the last entry
- **GIVEN** el reader ya procesó una entrada y guardó su `__CURSOR`
- **WHEN** la conexión se corta y el reader reconecta
- **AND** el stream vuelve a entregar esa misma entrada
- **THEN** el lector la descarta por coincidir con el último `__CURSOR`
- **AND** el rate limiter no la cuenta dos veces

#### Scenario: Entry without a usable timestamp is discarded
- **GIVEN** el stream entrega una entrada sin `__REALTIME_TIMESTAMP`, o con un
  valor no numérico como `abc`
- **WHEN** el lector la evalúa contra el corte del intento de conexión en curso
- **THEN** la entrada NO se entrega al pipeline Jail
- **AND** el rate limiter no la cuenta ni puede banear por ella
- **AND** la siguiente entrada bien formada sí se procesa con normalidad

#### Scenario: A failed connection attempt moves the cutoff forward
- **GIVEN** gatewayd lleva 30 s caído y ha vuelto
- **WHEN** el reader reintenta
- **THEN** el corte es el instante de ese nuevo intento
- **AND** lo ocurrido durante la caída no se entrega al pipeline

### Requirement: JournalError preserves the source chain

`JournalError` MUST carry the underlying error, when there is one, in a field
marked `#[source]`, so that callers can walk the chain through
`std::error::Error::source()` and inspect the cause by type. Failures that are
context only MUST remain representable without a source, and `JournalError`
MUST stay `Send + Sync + 'static`.

Las cuatro variantes (`Socket`, `Parse`, `TraefikParse`, `Connection`) SHALL
llevar un segundo campo `#[source] Option<Box<dyn std::error::Error + Send + Sync>>`.
`source()` SHALL devolver la causa cuando el campo no es `None` y `None` cuando
lo es. SHALL existir una pareja de constructores de ayuda por variante —una sin
fuente y otra con fuente— para que los sitios de contexto puro no ganen ruido.

La firma pública no cambia: cada sitio SHALL seguir devolviendo la misma
variante que devolvía antes. Con la Opción 1 de `design.md` (§D3), el texto del
primer campo NO SHALL cambiar, de modo que los `Display`, los `debug!`/`warn!`
que los imprimen y los tests que afirman sobre ese texto se mantienen.

```rust
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("Socket error: {0}")]
    Socket(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),
    #[error("Journal entry parse error: {0}")]
    Parse(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),
    #[error("Traefik access log parse error: {0}")]
    TraefikParse(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),
    #[error("Connection error: {0}")]
    Connection(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),
}

impl JournalError {
    pub fn socket(ctx: impl Into<String>) -> Self;
    pub fn socket_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn parse(ctx: impl Into<String>) -> Self;
    pub fn parse_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn traefik_parse(ctx: impl Into<String>) -> Self;
    pub fn traefik_parse_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn connection(ctx: impl Into<String>) -> Self;
    pub fn connection_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
}
```

#### Scenario: Parse failure exposes the serde_json error

- **GIVEN** una línea NDJSON que no es JSON válido
- **WHEN** `parse_journal_entry()` falla
- **THEN** devuelve `Err(JournalError::Parse)`
- **AND** `err.source()` es el `serde_json::Error` de la causa
- **AND** se puede hacer `downcast_ref::<serde_json::Error>()` sobre `source()`
  y leer su `line()` y `column()`
- **AND** el `Display` sigue nombrando el contexto (`invalid journal JSON: …`)

#### Scenario: Traefik access log failure exposes the serde_json error

- **GIVEN** un `MESSAGE` de Traefik cuyo texto no es JSON válido
- **WHEN** `parse_traefik_access_log()` falla
- **THEN** devuelve `Err(JournalError::TraefikParse)`
- **AND** `err.source()` es el `serde_json::Error` de la causa
- **AND** se puede leer su `line()` y `column()`
- **AND** el `Display` sigue nombrando el contexto (`invalid JSON: …`)

#### Scenario: Missing Unix socket exposes an io::Error with kind NotFound

- **GIVEN** `JournaldTransport::UnixSocket` apuntando a un path que no existe
- **WHEN** el reader intenta conectar
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `std::io::Error` de la causa
- **AND** `err.source().downcast_ref::<std::io::Error>().kind() == NotFound`

#### Scenario: Closed TCP port exposes an io::Error with kind ConnectionRefused

- **GIVEN** `JournaldTransport::Tcp` apuntando a un puerto en el que nadie escucha
- **WHEN** el reader intenta conectar
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `std::io::Error` de la causa
- **AND** `err.source().downcast_ref::<std::io::Error>().kind() == ConnectionRefused`

#### Scenario: Invalid gateway URL exposes the InvalidUri

- **GIVEN** un `base_url` que no parsea como URI
- **WHEN** se construye el endpoint TCP
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `http::uri::InvalidUri` de la causa
- **AND** el `Display` sigue nombrando el valor rechazado y la pista

#### Scenario: A failure without a cause keeps source None

- **GIVEN** un `base_url` al que le falta el esquema (contexto puro, sin causa)
- **WHEN** se construye el endpoint TCP
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es `None`
- **AND** el `Display` es exactamente el de hoy

#### Scenario: The error stays usable across the async boundary

- **GIVEN** el reader corre dentro de un `tokio::spawn`
- **WHEN** un `JournalError` cruza el `.await` de la tarea de fondo
- **THEN** `JournalError` sigue siendo `Send + Sync + 'static`
- **AND** la aserción estática correspondiente compila

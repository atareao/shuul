# journald-reader Specification

## Purpose
Define el módulo `journald_reader`, que obtiene en streaming los access logs de Traefik desde `systemd-journal-gatewayd` —por socket Unix o TCP— los mapea a `ReportPayload` y los entrega al pipeline Jail, la misma función que alimenta el endpoint HTTP `POST /api/v1/report`. Cubre el transporte seleccionable por entorno, el filtrado por contenedor, los diagnósticos de configuración y el corte que descarta el backlog previo al inicio del intento de conexión, de modo que el rate limiter nunca cuenta tráfico anterior al arranque.

## Requirements

### Requirement: Journald reader transport

El módulo `journald_reader` SHALL obtener los access logs de Traefik en
streaming, mapearlos a `ReportPayload` y entregarlos a `run_jail_pipeline` desde
uno de **dos backends**:

- la **API de Docker**, sobre socket Unix `AF_UNIX` o sobre TCP;
- **`systemd-journal-gatewayd`**, sobre TCP o sobre socket Unix `AF_UNIX`.

Los transportes del backend de journald NO SHALL leer directamente
`/run/systemd/journal/socket`, porque ese socket es **write-only** (las
aplicaciones lo usan para *enviar* entradas al journal); el contenido sólo se
sirve a través de la API del journal, que gatewayd expone por HTTP.

El transporte SHALL seleccionarse con esta precedencia:

1. Si `SHUUL_DOCKER_SOCKET` está definida y no está en blanco → **API de Docker**
   por socket Unix, conectando al path indicado.
2. Si no, si `SHUUL_DOCKER_URL` está definida y no está en blanco → **API de
   Docker** por TCP contra esa URL base.
3. Si no, si `SHUUL_JOURNALD_SOCKET` está definida y no está en blanco →
   **gatewayd** por socket Unix, conectando al path indicado.
4. Si no, si `SHUUL_JOURNALD_URL` está definida y no está en blanco →
   **gatewayd** por TCP contra esa URL base.
5. Si no, **gatewayd** por TCP contra `http://localhost:19531`.

Un valor en blanco SHALL equivaler a una variable no definida. Cuando dos
variables del mismo backend estén definidas y no vacías SHALL usarse el socket y
SHALL registrarse un warning nombrando la variable de URL que se ignora. El
backend de más prioridad SHALL ganar sin que las variables de los otros backends
lleguen a evaluarse como transporte.

La petición del backend de **gatewayd** SHALL ser
`GET /entries?follow&CONTAINER_NAME={container_name}` con las cabeceras
`Accept: application/json` (Journal JSON Format, NDJSON: un objeto JSON por
línea) y `Host`. El filtrado por contenedor SHALL delegarse en el servidor
mediante el parámetro `KEY=match`. La misma ruta de petición SHALL usarse en
ambos transportes de gatewayd; sólo cambia el transporte subyacente.

La petición del backend de **Docker** SHALL ser
`GET /v1.44/containers/{container_name}/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={epoch}`
con las cabeceras `Accept: application/json` y `Host`. NO SHALL enviarse la
cabecera `Upgrade` ni `Connection: Upgrade`: si se piden, el motor responde
`101 UPGRADED` en lugar de servir el stream; sin ellas responde `200 OK` con el
cuerpo `application/vnd.docker.raw-stream`. El filtrado por contenedor en este
backend SHALL delegarse en el servidor mediante el segmento `{container_name}`
del path, de modo que las líneas SHALL NOT requerir el campo `CONTAINER_NAME`
para ser procesadas.

`timestamps=1` SHALL hacer que **cada carga** del frame llegue como
`<instante RFC3339Nano> <línea JSON>`. El lector SHALL extraer ese instante,
SHALL descartar el prefijo y SHALL entregar el resto a `parse_traefik_access_log`
sin cambiar el parseo de la línea.

`since` SHALL ser el corte del intento de conexión en curso en segundos desde
`UNIX_EPOCH` y SHALL actuar como **filtro grueso del servidor** —tiene
granularidad de segundo—, con el mismo papel que la cabecera `Range` en el
backend de journald: es sólo una **pista** que reduce lo que viaja, y la
corrección NO SHALL depender de él. El filtro exacto que garantiza que no se
reprocesa nada vive en el requisito *No history replay* y compara el instante de
cada carga con el corte del intento de conexión en curso.

Ambos backends y todos sus transportes SHALL compartir el mismo comportamiento
de streaming, reconexión, shutdown y fail-open.

```rust
pub enum JournaldTransport {
    /// HTTP sobre TCP contra una URL base de gatewayd.
    Tcp { base_url: String },
    /// HTTP sobre un socket AF_UNIX de gatewayd.
    UnixSocket { path: PathBuf },
    /// API de logs de Docker sobre TCP contra una URL base (proxy).
    DockerTcp { base_url: String },
    /// API de logs del motor de Docker sobre su socket AF_UNIX.
    DockerUnixSocket { path: PathBuf },
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
pub const DOCKER_API_VERSION: &str = "v1.44";

impl JournaldReader {
    pub fn new(base_url: &str, container_name: &str) -> Self;
    pub fn unix(path: impl Into<PathBuf>, container_name: &str) -> Self;
    /// Los cuatro parámetros de transporte van en el **orden de la precedencia**:
    /// primero los de Docker (socket, URL) y después los de journald (socket,
    /// URL). Los ausentes o en blanco se consideran no definidos.
    pub fn from_values(
        docker_socket: Option<String>,
        docker_url: Option<String>,
        journald_socket: Option<String>,
        journald_url: Option<String>,
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
- **AND** el comportamiento es el mismo en los dos backends

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

#### Scenario: Docker socket transport selected from the environment
- **GIVEN** `SHUUL_DOCKER_SOCKET=/var/run/docker.sock` definida y no vacía
- **AND** ninguna otra variable de transporte definida
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `DockerUnixSocket` con ese path
- **AND** el endpoint que aparece en los logs es `docker:unix:/var/run/docker.sock (traefik)`
- **AND** la URL por defecto de gatewayd no se usa

#### Scenario: Docker URL transport selected when there is no Docker socket
- **GIVEN** `SHUUL_DOCKER_URL=http://docker-socket-proxy:2375` definida y no vacía
- **AND** `SHUUL_DOCKER_SOCKET` sin definir o en blanco
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `DockerTcp` con esa URL base
- **AND** el endpoint que aparece en los logs es `docker:http://docker-socket-proxy:2375 (traefik)`

#### Scenario: Docker transport takes precedence over journald
- **GIVEN** `SHUUL_DOCKER_SOCKET`, `SHUUL_DOCKER_URL`, `SHUUL_JOURNALD_SOCKET` y `SHUUL_JOURNALD_URL` definidas y no vacías
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `DockerUnixSocket` con el path de `SHUUL_DOCKER_SOCKET`
- **AND** se registra un warning nombrando las variables ignoradas
- **AND** si se retira `SHUUL_DOCKER_SOCKET`, el transporte pasa a `DockerTcp`
- **AND** si se retiran además las dos variables de Docker, el transporte pasa a `UnixSocket` con el path de `SHUUL_JOURNALD_SOCKET`
- **AND** retirada también ésta, el transporte pasa a `Tcp` con la URL de `SHUUL_JOURNALD_URL`

#### Scenario: A blank Docker variable counts as undefined
- **GIVEN** `SHUUL_DOCKER_SOCKET=""` o con sólo espacios en blanco
- **AND** `SHUUL_JOURNALD_SOCKET` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** `SHUUL_DOCKER_SOCKET` no activa el backend de Docker
- **AND** el transporte es el de journald indicado por `SHUUL_JOURNALD_SOCKET`

#### Scenario: Docker log stream request
- **GIVEN** un lector con el backend de Docker y contenedor `traefik`
- **WHEN** el reader abre el stream
- **THEN** la petición es `GET /v1.44/containers/traefik/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={epoch}`
- **AND** envía las cabeceras `Accept: application/json` y `Host`
- **AND** NO envía `Upgrade` ni `Connection: Upgrade`
- **AND** en el transporte de socket Unix la cabecera `Host` vale `docker`
- **AND** en el transporte TCP la cabecera `Host` es la autoridad del endpoint configurado
- **AND** el path y la query son idénticos en los dos transportes

#### Scenario: A base path in the Docker URL is ignored
- **GIVEN** `SHUUL_DOCKER_URL=http://docker-socket-proxy:2375/base` definida y no vacía
- **AND** `SHUUL_DOCKER_SOCKET` sin definir o en blanco
- **WHEN** se construye el reader y abre el stream con el backend de Docker
- **THEN** la petición se envía en **forma origen** (`path?query`), sin el path base
- **AND** la cabecera `Host` es la autoridad del endpoint configurado, sin el path base
- **AND** el endpoint que aparece en los logs es el **efectivo**, `docker:http://docker-socket-proxy:2375 (traefik)`
- **AND** se registra un warning que nombra el path base que se ignora

#### Scenario: Container filtering is delegated to the Docker API
- **GIVEN** un lector con el backend de Docker y contenedor `traefik`
- **WHEN** el reader construye la petición
- **THEN** el nombre del contenedor viaja en el segmento `{container_name}` del path, URL-encoded
- **AND** el reader NO exige el campo `CONTAINER_NAME` en las líneas del stream para procesarlas

#### Scenario: Docker API not available
- **GIVEN** el socket de Docker no existe, o el proxy es inalcanzable, o el contenedor no existe
- **WHEN** `JournaldReader::run()` se ejecuta
- **THEN** se registra un warning `docker api not available` nombrando el endpoint y la causa
- **AND** se reintenta tras `retry_delay`
- **AND** `run()` no devuelve error ni se cae el proceso
- **AND** el pipeline WAF sigue funcionando sin enterarse

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

De los campos de la tabla, sólo los **cinco derivados de cabecera** —`user_agent`,
`referer`, `content_type`, `accept_language` y `x_request_id`— dependen de la
configuración del origen. Traefik sólo emite las claves `request_User-Agent`,
`request_Referer`, `request_Content-Type`, `request_Accept-Language` y
`request_X-Request-Id` si su bloque `accessLog.fields.headers.names` las declara;
sin ese bloque el `defaultMode` es `drop` y las claves **no aparecen** en el JSON.
Ni la API de Docker ni `systemd-journal-gatewayd` aportan esos campos: ambos
transportan el mismo access log JSON, de modo que la disponibilidad depende del
**origen** y no del **transporte**, y ningún backend puede inventarlos.

En consecuencia, cuando la clave de origen de un campo falte, el mapeo SHALL
resolver ese campo a `None` y NO SHALL descartar la línea por ello: un access log
sin claves `request_*` sigue siendo un access log válido, con su IP, su status,
su path y su método. La ausencia de esos campos es una **degradación del
filtrado**, no un error de parseo, y el lector SHALL NOT confundir las dos cosas.

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

#### Scenario: An access log without the request_ keys keeps the line and maps the header fields to None
- **GIVEN** un access log con `ClientHost`, `OriginStatus: 401`, `RequestPath: "/admin"`, `RequestMethod: "GET"`, `RequestHost` y **ninguna** clave que empiece por `request_`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` —la línea NO se descarta—
- **AND** `user_agent`, `referer`, `content_type`, `accept_language` y `x_request_id` son `None`
- **AND** el resto de campos mapeados (`ip_address`, `status_code`, `path`, `method`, `fqdn`) conservan su valor

#### Scenario: An access log with only a subset of the request_ keys maps the present ones
- **GIVEN** un access log con `OriginStatus: 200` y sólo las claves `request_User-Agent: "Mozilla/5.0"` y `request_X-Request-Id: "abc-123"`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `user_agent = "Mozilla/5.0"` y `x_request_id = "abc-123"`
- **AND** `referer`, `content_type` y `accept_language` son `None`
- **AND** la línea NO se descarta

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

El lector SHALL alimentar el pipeline Jail únicamente con entradas y cargas
posteriores al corte del intento de conexión **en curso**.

**Cada intento de conexión fija su propio corte**, y lo fija en el instante en
que el intento empieza —con precisión de microsegundos, la misma de
`now_micros()`, no de segundo—, con independencia de lo que haya procesado el
intento anterior. Un intento fallido SHALL fijar también su propio corte y
desplazarlo hacia delante.

Sólo SHALL entregarse al pipeline Jail lo que cada backend acepte al compararlo
con ese corte —la comparación exacta de cada uno se detalla abajo—; todo lo demás
SHALL descartarse.

El alcance de este requisito es la **entrega**: el lector garantiza que las
entradas y cargas posteriores al corte llegan al pipeline Jail. La cuantificación
de fallos y el baneo los hace `run_jail_pipeline` en `http/report.rs` —cubierto
por sus propios tests (`rate_limiting`, `report_per_rule_ban`)—, de modo que
cuando un escenario dice que el rate limiter «cuenta» o «no cuenta» una carga
describe la consecuencia observable de esa entrega, no una responsabilidad del
propio lector.

La garantía SHALL ser la misma en los dos backends, y cada uno la consigue por
medios distintos:

- **journald**: el corte por `__REALTIME_TIMESTAMP` más el dedupe por
  `__CURSOR`;
- **Docker**: el filtro grueso `since` del servidor más la comparación **exacta**
  con el instante que precede a cada carga.

`since` SHALL ser ese corte en segundos desde `UNIX_EPOCH`. SHALL ser sólo una
**pista** —su granularidad es de segundo— y nunca la fuente de corrección:
reduce lo que viaja, pero la corrección la da la comparación con el corte, que es
exacta.

La comparación exacta **asume que el instante de cada carga y el corte del intento
de conexión proceden de relojes sincronizados**: con el daemon en la misma máquina
—los modos A y B de este change, mismo-host por definición— emisor y lector
comparten reloj, igual que en journald con `__REALTIME_TIMESTAMP`, de modo que un
daemon de Docker en **otra** máquina queda fuera de este contrato.

Para cada **entrada** del backend de journald el lector SHALL:

- descartarla si su `__REALTIME_TIMESTAMP` es anterior al instante en que se
  inició el intento de conexión;
- descartarla si `__REALTIME_TIMESTAMP` falta o no es un entero decimal; una
  entrada sin instante no se puede situar respecto al corte y se descarta por
  precaución;
- descartarla si su `__CURSOR` coincide con el último `__CURSOR` ya procesado,
  porque el stream puede reanudarse en esa misma entrada;
- alimentar el pipeline Jail en caso contrario.

Para cada **carga** del backend de Docker el lector SHALL:

- extraer el instante `RFC3339Nano` que la precede y descartar el prefijo;
- descartarla si ese instante falta o no se puede parsear; una carga sin instante
  no se puede situar respecto al corte y se descarta por precaución;
- descartarla si su instante NO es **estrictamente posterior** al corte del
  intento de conexión en curso, porque el filtro `since` del servidor tiene
  granularidad de segundo y por sí solo dejaría pasar lo escrito dentro de ese
  segundo;
- alimentar el pipeline Jail en caso contrario.

El descarte del backend de Docker NO SHALL basarse en el **contenido** de la
línea: dos peticiones distintas pueden compartir IP, path, status y segundo —es
lo corriente en un escáner que reintenta— y ambas SHALL contarse.

El descarte SHALL ser silencioso a nivel `INFO` (a lo sumo `DEBUG`) y NO SHALL
contarse como error.

**Consecuencia asumida, sin adornos.** Lo que ocurra mientras el lector está
caído —o entre dos intentos, o durante un intento fallido— NO SHALL entregarse al
pipeline Jail y por tanto **no se cuenta** en el rate limiter. Es un fail-open
**deliberado**: se prefiere no contar ese tráfico a banear tráfico legítimo
reproduciéndolo de golpe contra una ventana de segundos. El pipeline WAF sigue
protegiendo durante esa ventana porque es síncrono y no depende de este
transporte. Contabilizar ese tráfico exigiría pasar el instante del log al rate
limiter para que la ventana se evalúe contra el tiempo real del evento, y eso
queda fuera de este cambio.

Este filtrado NO SHALL depender del filtro del servidor, que es sólo una pista:

- gatewayd no garantiza el `skip` de `entries=<cursor>:1` sin `num_entries`, ni
  aplica `realtime` cuando la petición lleva `follow`;
- la API de Docker filtra `since` con granularidad de segundo, así que por sí
  solo dejaría pasar lo escrito dentro del segundo del corte.

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

#### Scenario: A failed Docker connection attempt moves the cutoff forward
- **GIVEN** el socket de Docker o el proxy lleva 30 s inalcanzable y ha vuelto
- **WHEN** el reader reintenta y el intento fija su propio corte
- **THEN** `since` es el corte del intento nuevo, no el del intento fallido
- **AND** lo ocurrido durante la caída no se entrega al pipeline
- **AND** el rate limiter no cuenta ese tráfico ni puede banear por él

#### Scenario: Docker since spares the startup backlog from the rate limiter
- **GIVEN** el contenedor acumuló access logs de Traefik con fallos anteriores al arranque del reader
- **WHEN** el reader abre el stream de Docker con `since` igual al corte del intento de conexión en curso, en segundos
- **THEN** el motor no entrega ese histórico
- **AND** cualquier carga cuyo instante no sea estrictamente posterior al corte se descarta igualmente
- **AND** no se registra ningún fallo en el rate limiter por ellas
- **AND** no se crea ningún baneo a partir de ellas

#### Scenario: Docker reconnect fixes a new cutoff and does not reprocess the boundary line
- **GIVEN** el reader ya entregó cargas al pipeline Jail en un intento anterior
- **WHEN** la conexión se cierra y el reader reconecta
- **THEN** el intento nuevo fija su propio corte, en el instante en que empieza
- **AND** repite la petición con `since=<corte del intento nuevo, en segundos>`
- **AND** el motor vuelve a entregar las cargas de ese segundo, porque `since` tiene granularidad de segundo
- **AND** descarta toda carga cuyo instante no sea estrictamente posterior a ese corte, incluida la de la frontera
- **AND** las cargas escritas durante la desconexión no se entregan al pipeline
- **AND** el rate limiter no cuenta dos veces ninguna de esas cargas

#### Scenario: Two identical lines with different timestamps are both counted
- **GIVEN** dos cargas cuyo contenido es idéntico (misma IP, mismo path, mismo status) pero con instantes distintos, los dos posteriores al corte del intento en curso
- **WHEN** el reader las procesa
- **THEN** las dos se entregan al pipeline Jail
- **AND** el rate limiter las cuenta las dos
- **AND** el dedupe no las confunde por compartir el texto de la línea

#### Scenario: A Docker payload at the attempt cutoff is discarded
- **GIVEN** el corte del intento de conexión en curso, fijado en microsegundos
- **WHEN** el stream entrega una carga cuyo instante es exactamente ese corte
- **THEN** la carga NO se entrega al pipeline Jail
- **AND** el rate limiter no la cuenta

#### Scenario: A Docker payload without a usable timestamp is discarded
- **GIVEN** una carga sin el prefijo de instante, o con un prefijo que no se puede parsear
- **WHEN** el reader la evalúa
- **THEN** la carga NO se entrega al pipeline Jail
- **AND** el rate limiter no la cuenta ni puede banear por ella
- **AND** la siguiente carga bien formada sí se procesa con normalidad
- **AND** el stream no se aborta

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

### Requirement: Docker log stream frame decoding

El lector SHALL asumir un contenedor **sin TTY**: es entonces cuando el motor
multiplexa el stream y cada mensaje llega en un **frame** con cabecera de 8
bytes. Un contenedor con `tty: true` queda **fuera de este contrato**, porque su
stream es crudo y sin cabeceras; el despliegue NO SHALL activar `tty` (el
`compose.yaml` de Shuul no lo pone y no debe ponerse).

Formato de la cabecera:

| Offset | Tamaño | Contenido |
|---|---|---|
| 0 | 1 byte | identificador de stream: `0` = stdin, `1` = stdout, `2` = stderr |
| 1 | 3 bytes | relleno |
| 4 | 4 bytes | longitud de la carga, entero sin signo big-endian |

El decodificador SHALL ser **incremental**: SHALL mantener el estado entre los
chunks que llegan del socket, de modo que:

- un frame partido entre dos chunks se decodifique al completarse, sin depender
  de dónde caiga la frontera;
- un chunk que traiga varios frames los decodifique todos;
- un frame con carga vacía (longitud `0`) se acepte y no entregue ninguna línea.

El lector NO SHALL trocear el cuerpo por líneas antes de desmultiplexar.

Cada carga SHALL ser una línea del access log JSON de Traefik precedida por el
instante que pide `timestamps=1`, ya con el envoltorio del driver de logging
extraído por la API. El lector SHALL descartar el prefijo del instante y SHALL
entregar el resto a `parse_traefik_access_log`; el `ReportPayload` resultante
SHALL desembocar en la misma `run_jail_pipeline` que alimenta el endpoint
`POST /api/v1/report`, sin duplicar la lógica del pipeline. El uso del instante
para no reproducir histórico es responsabilidad del requisito *No history
replay*.

Sólo las cargas con identificador `stdout` SHALL entregarse al pipeline Jail: el
access log de Traefik va a stdout. Las de `stderr` SHALL descartarse a nivel
`debug`.

El byte de identificador de stream de una cabecera válida SHALL ser `0`, `1` o
`2`. Si un frame llega con cualquier otro valor, el lector SHALL **descartar la
conexión** con un warning que nombre la causa probable —un contenedor con TTY,
cuyo stream va crudo— en lugar de seguir intentando decodificar frames: un JSON
crudo empieza por `{` (`0x7B`), que nunca es un identificador de stream válido,
de modo que el síntoma deje de ser una avería muda.

Un frame cuya cabecera sea imposible (menos de 8 bytes disponibles y stream
cerrado) o cuya carga no sea una línea utilizable SHALL descartarse sin abortar
el stream ni caer el proceso.

#### Scenario: A frame split between two chunks is decoded
- **GIVEN** un stream cuyo primer chunk contiene la cabecera de 8 bytes y sólo la mitad de la carga
- **WHEN** llega el segundo chunk con el resto de la carga
- **THEN** el frame se decodifica al completarse
- **AND** la línea se entrega al pipeline Jail una sola vez
- **AND** no se descarta ni se duplica por el corte

#### Scenario: Two frames in one chunk are both decoded
- **GIVEN** un chunk que contiene dos frames completos consecutivos
- **WHEN** el lector lo procesa
- **THEN** las dos cargas se decodifican en el mismo chunk
- **AND** cada una llega al pipeline Jail con su propio `status_code`

#### Scenario: A frame with an empty payload is accepted
- **GIVEN** un frame cuya longitud de carga es `0`
- **WHEN** el lector lo procesa
- **THEN** no se entrega ninguna línea al pipeline Jail
- **AND** la decodificación del stream continúa con el frame siguiente

#### Scenario: stdout and stderr are told apart by the frame header
- **GIVEN** un frame con identificador de stream `1` (stdout) y otro con `2` (stderr)
- **WHEN** el lector procesa los dos
- **THEN** la carga de stdout se entrega al pipeline Jail
- **AND** la carga de stderr se descarta a nivel `debug`
- **AND** se registra el identificador de stream en el descarte

#### Scenario: The payload timestamp prefix is stripped before parsing
- **GIVEN** una carga con la forma `<instante RFC3339Nano> {…}` y `timestamps=1` en la petición
- **WHEN** el lector la procesa
- **THEN** extrae el instante, descarta el prefijo y parsea el resto como el JSON del access log
- **AND** el `ReportPayload` llega al pipeline Jail con los mismos campos que sin el prefijo

#### Scenario: A malformed frame is discarded without aborting the stream
- **GIVEN** un frame con menos de 8 bytes de cabecera antes del cierre del stream, o una carga que no es una línea de access log
- **WHEN** el lector lo procesa
- **THEN** el frame se descarta
- **AND** el stream sigue leyéndose y los frames siguientes se procesan con normalidad
- **AND** no se registra como error ni se interrumpe el bucle del reader

#### Scenario: A raw TTY stream is diagnosed instead of decoded in silence
- **GIVEN** un contenedor arrancado con `tty: true`, cuyo stream llega crudo y sin cabeceras de 8 bytes
- **WHEN** el lector lee el primer byte de un frame y no es `0`, `1` ni `2`
- **THEN** la conexión se descarta
- **AND** se registra un warning que nombra la causa probable: el contenedor tiene TTY
- **AND** el lector NO sigue intentando decodificar frames de esa conexión
- **AND** el bucle reconecta tras `retry_delay` y el WAF sigue funcionando

### Requirement: TCP transport URL scheme

Los transportes TCP se configuran con una **URL base** en `SHUUL_DOCKER_URL` (API de
Docker) y en `SHUUL_JOURNALD_URL` (gatewayd). Ambas SHALL aceptar `http://` y `tcp://`
como **sinónimos**, de modo que la sintaxis que Docker emplea para `DOCKER_HOST`
(`tcp://host:puerto`) SHALL valer sin traducción y SHALL producir exactamente el mismo
transporte, la misma autoridad y la misma petición que su equivalente en `http://`.

`https://` SHALL rechazarse con un diagnóstico que lo nombre, porque los transportes TCP
NO implementan TLS. Cualquier otro esquema SHALL rechazarse con un diagnóstico que
nombre el esquema recibido, y una URL sin esquema SHALL rechazarse nombrando la
variable que la contiene. En los tres casos el rechazo SHALL ser **recuperable**: un
warning y un reintento tras `retry_delay`, nunca la caída del proceso ni un error que
impida el arranque.

Cuando la URL traiga un **path base**, éste SHALL ignorarse al construir la petición y
SHALL registrarse un warning nombrándolo, como ya ocurre con los demás endpoints TCP.

`endpoint_display()` SHALL mostrar el **esquema efectivo**, no el escrito: una URL
configurada como `tcp://host:2375` SHALL mostrarse como `http://host:2375` —y como
`docker:http://host:2375 ({container_name})` en el backend de Docker—, de modo que los
logs no exhiban un esquema que la conexión no usa.

#### Scenario: tcp:// is accepted as an alias of http://
- **GIVEN** `SHUUL_DOCKER_URL=tcp://docker-socket-proxy:2375` definida y no vacía
- **AND** `SHUUL_DOCKER_SOCKET` sin definir
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `DockerTcp` con autoridad `docker-socket-proxy:2375`
- **AND** la petición se envía sobre HTTP/1.1 en claro, idéntica a la de `http://`

#### Scenario: The alias also works for the gatewayd URL
- **GIVEN** `SHUUL_JOURNALD_URL=tcp://journald.example:19531` definida y no vacía
- **AND** ninguna variable de mayor precedencia definida
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `Tcp` con autoridad `journald.example:19531`

#### Scenario: The effective scheme is what gets displayed
- **GIVEN** una URL base configurada como `tcp://docker-socket-proxy:2375`
- **WHEN** se llama a `endpoint_display()`
- **THEN** devuelve `docker:http://docker-socket-proxy:2375 (traefik)`
- **AND** el esquema `tcp` no aparece en la línea de arranque

#### Scenario: https is still rejected by name
- **GIVEN** `SHUUL_JOURNALD_URL=https://journald.example:19531` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el intento de conexión falla con un diagnóstico que nombra `https`
- **AND** el diagnóstico no se confunde con un esquema desconocido
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: An unknown scheme is rejected by name
- **GIVEN** `SHUUL_DOCKER_URL=ftp://host:2375` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el rechazo nombra el esquema `ftp`
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: A URL without a scheme is rejected naming the variable
- **GIVEN** `SHUUL_DOCKER_URL=host:2375` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el rechazo nombra la variable que contiene la URL
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: A base path is ignored with a warning
- **GIVEN** `SHUUL_DOCKER_URL=tcp://docker-socket-proxy:2375/docker` definida y no vacía
- **WHEN** se construye el reader
- **THEN** la URL efectiva es `http://docker-socket-proxy:2375`
- **AND** se registra un warning nombrando el path base ignorado

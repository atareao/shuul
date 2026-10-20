## MODIFIED Requirements

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

## ADDED Requirements

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

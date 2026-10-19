# Proposal: journal-error-source-chain

## Why

`JournalError` (backend/src/journald_reader.rs:69) tiene cuatro variantes —
`Socket`, `Parse`, `TraefikParse`, `Connection` — y **las cuatro llevan un
`String` como único campo**. El error de origen se aplana con `format!` en el
momento de construirlo, así que `std::error::Error::source()` devuelve siempre
`None` (thiserror no genera nada sin un campo `#[source]`).

En 9 de los 32 sitios donde se construye un `JournalError` hay una causa real
que hoy se descarta:

| Línea | Construcción | Tipo perdido |
|---|---|---|
| 471 | `frame.map_err(\|e\| JournalError::Socket(e.to_string()))` | `hyper::Error` (`body.frame()`) |
| 553 | `http1::handshake(...).map_err(...)` | `hyper::Error` |
| 559 | `sender.send_request(request).await.map_err(...)` | `hyper::Error` |
| 580 | `UnixStream::connect(path).await.map_err(...)` | `std::io::Error` |
| 592 | `TcpStream::connect(...).await.map_err(...)` | `std::io::Error` |
| 618 | `builder.body(...).map_err(...)` | `http::Error` |
| 690 | `base_url.parse().map_err(...)` | `http::uri::InvalidUri` |
| 771 | `serde_json::from_str(trimmed).map_err(...)` | `serde_json::Error` |
| 1117 | `serde_json::from_str(json_str).map_err(...)` | `serde_json::Error` |

Los otros 23 sitios (447 `"gateway returned HTTP {status}"`, 566 timeout,
696/701/706/714 esquema/host ausentes, 767/774/787 del parseo NDJSON, los tests
de 865/871/877/883 y los `matches!`/`Err(...)` de 942/1301/1442/2486/2502) **no
tienen causa que perder**: son contexto puro y deben poder seguir construyéndose
sin fuente.

Consecuencia: quien recibe un `JournalError` no puede recorrer la cadena ni
inspeccionar la causa **por tipo**. Se pierde justo lo que hace falta para
distinguir los diagnósticos que `SELF-HOSTING.md` §2.11 documenta como casos
distintos:

- el `kind()` de un `io::Error`: ¿`NotFound` —socket ausente o no montado— o
  `ConnectionRefused` —gatewayd caído—? Son dos averías con dos remedios
  distintos, y hoy sólo se distinguen leyendo el texto formateado.
- la `line()`/`column()` de un `serde_json::Error`: un access log mal formado se
  diagnostica con su posición exacta.
- el `is_incomplete_message()` de un `hyper::Error`: distingue una respuesta
  truncada de un fallo de protocolo.

Hoy el código no puede saberlo; sólo un humano leyendo el log. Esta deuda quedó
registrada en el archive de `journald-unix-socket`: *«M-7 (`JournalError` sin
`#[source]`) queda como deuda técnica documentada»*.

## What Changes

- Las cuatro variantes de `JournalError` ganan un segundo campo
  `#[source] Option<Box<dyn std::error::Error + Send + Sync>>`. `source()`
  devuelve la causa cuando existe y `None` cuando no.
- Nuevos constructores de ayuda —una pareja por variante, `socket(…)` /
  `socket_from("…", e)`, `connection(…)` / `connection_from(…)`, etc.— para que
  los 23 sitios sin causa no ganen ruido sintáctico.
- Los 9 puntos con causa real pasan a adjuntarla como `#[source]`.
- `JournalError` **sigue siendo `Send + Sync + 'static`**, que es lo que exige
  cruzar el `.await` dentro del `tokio::spawn` de la tarea de fondo del reader
  (backend/src/main.rs:379).

No cambia la firma pública de ninguna función, ni la variante que se devuelve,
ni los mensajes de log, ni el texto de `Display` (ver la decisión abierta en
`design.md`).

## Capabilities

**New Capabilities:**

None.

**Modified Capabilities:**

- `journald-reader` — nueva requirement *JournalError preserves the source
  chain*: el error lleva la causa subyacente cuando existe, de forma que la
  cadena se puede recorrer con `source()` y la causa se puede inspeccionar por
  tipo, sin dejar de ser `Send + Sync + 'static`.

## Impact

- `backend/src/journald_reader.rs`: cuatro variantes con `#[source]`, ocho
  constructores de ayuda, los 9 sitios con causa y los 23 sin causa reescritos a
  los constructores, más los tests que hoy construyen las variantes a mano
  (865/871/877/883 y los `matches!`/`Err(...)` de 942/1301/1442/2486/2502).
- Sin cambios de esquema, ni de variables de entorno, ni de `compose.yml`, ni de
  frontend.
- Efecto observable para el operador: los `warn!`/`debug!` que hoy imprimen
  `{e}` siguen imprimiendo exactamente lo mismo (Opción 1 de `design.md`), pero
  un futuro reporte de diagnóstico puede recorrer la cadena. El
  `is_incomplete_message()` de `hyper` y el `kind()` del `io::Error` quedan
  disponibles por primera vez.
- Sin cambios de rendimiento: el `String` ya se formateaba en el error; adjuntar
  la fuente es un `Box` por error.

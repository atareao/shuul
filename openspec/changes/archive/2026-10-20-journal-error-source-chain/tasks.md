# Tasks: journal-error-source-chain

## 1. Contrato

- [x] 1.1 RED: aserción estática de que `JournalError: Send + Sync + 'static`, y tests de que `source()` devuelve la causa correcta en los escenarios 1–5 del delta (JSON inválido → `serde_json::Error` con `line()`/`column()`; access log de Traefik no JSON → `serde_json::Error`; socket Unix inexistente → `io::Error` con `kind() == NotFound`; puerto TCP cerrado → `io::Error` con `kind() == ConnectionRefused`; URL inválida → `InvalidUri`). Verificación: `cargo test journald_reader` **no compila hoy**, porque las variantes todavía tienen un solo campo y `source()` devuelve siempre `None`.
  - Evidencia RED (literal): `error[E0599]: no variant, associated function, or constant named 'connection_from' found for enum 'JournalError'` y `error[E0023]: this pattern has 2 fields, but the corresponding tuple variant has 1 field`. Tras dar la nueva forma al enum, el resto de puntos de construcción rompen: `22 error[E0061]` + `6 error[E0023]` (28 errores en el target de test).
- [x] 1.2 GREEN: añadir a las cuatro variantes el segundo campo `#[source] Option<Box<dyn std::error::Error + Send + Sync>>` y los ocho constructores de ayuda (`socket`/`socket_from`, `parse`/`parse_from`, `traefik_parse`/`traefik_parse_from`, `connection`/`connection_from`). Verificación: el módulo compila y la aserción estática de 1.1 pasa.
  - `socket` y `traefik_parse` no tienen sitio de producción sin causa, así que sólo los usan los tests; llevan `#[allow(dead_code)]` (precedente: `with_retry_delay`) para que el gate `cargo clippy -- -D warnings` pase manteniendo la pareja simétrica que exige la spec.
- [x] 1.3 GREEN: reescribir los 23 sitios sin causa a los constructores sin fuente (447, 566, 696, 701, 706, 714, 767, 774, 787) y migrar los tests que construyen las variantes a mano (865, 871, 877, 883) y los `matches!`/`Err(...)` de 942, 1301, 1442, 2486, 2502 (más el de 2472, no listado pero con el mismo patrón). Verificación: `cargo test journald_reader` en verde y los tests de `source() == None` de la tarea 3.1 pasan.
- [x] 1.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.

## 2. Causa en los 9 puntos

- [x] 2.1 RED/GREEN 471 (`body.frame()`): test de que un frame de error de `hyper` produce un `JournalError::Socket` cuyo `source()` es un `hyper::Error`. Verificación: el test pasa. **Si forzar el fallo de `frame()` de forma limpia no es posible, decirlo explícitamente en esta tarea en vez de declarar una cobertura que no existe.**
  - Cubierto por `test_truncated_response_body_exposes_a_hyper_error`: un gateway `AF_UNIX` anuncia `Content-Length: 100`, escribe 10 bytes y cierra; `stream_once` devuelve `JournalError::Socket` con el `hyper::Error` como `source()`. No depende de reloj ni de timing (el EOF es determinista).
- [ ] 2.2 RED/GREEN 553 (handshake HTTP): test con un `TcpListener` de `tokio` que acepta la conexión y la cierra a media respuesta, de modo que `http1::handshake` falle; assertar que `source()` es el `hyper::Error`. Verificación: el test pasa. **Si no se puede cubrir de forma limpia, decirlo en vez de declarar cobertura inexistente.**
  - **NO CUBIERTO — no existe cobertura propia.** El `#[source]` del sitio 553 está puesto (`connection_from`), pero `hyper::client::conn::http1::handshake` **no realiza E/S**: en hyper 1.8.1 su implementación sólo construye el `Connection` y el `SendRequest` y devuelve `Ok(...)` de forma incondicional (verificado en `hyper-1.8.1/src/client/conn/http1.rs`, `Builder::handshake` y el `pub async fn handshake` que lo envuelve). Ningún input de shuul puede hacer que falle: la E/S empieza en `send_request` (sitio 559, sí cubierto). Un listener que corta la conexión hace fallar el 559, no el 553. Forzar el 553 exigiría mockear los internos de hyper; se descarta por no aportar.
- [x] 2.3 RED/GREEN 559 (`send_request`): test con un `TcpListener` que corta la conexión antes de completar la respuesta, de modo que `send_request` falle; assertar `source()` es el `hyper::Error`. Verificación: el test pasa.
  - Cubierto por `test_send_request_exposes_a_hyper_error_when_the_peer_closes_early`.
- [x] 2.4 RED/GREEN 580 (`UnixStream::connect`): test contra un path que no existe; assertar `source()` es el `io::Error` con `kind() == NotFound`. Verificación: el test pasa.
  - Cubierto por `test_missing_unix_socket_exposes_an_io_error_not_found`.
- [x] 2.5 RED/GREEN 592 (`TcpStream::connect`): test contra un puerto cerrado; assertar `source()` es el `io::Error` con `kind() == ConnectionRefused`. Verificación: el test pasa.
  - Cubierto por `test_closed_tcp_port_exposes_an_io_error_connection_refused`.
- [ ] 2.6 RED/GREEN 618 (`builder.body`): test de la construcción de la petición cuando falla; assertar `source()` es el `http::Error`. Verificación: el test pasa. **Si forzar el fallo de `body()` de forma limpia no es posible, decirlo en vez de declarar cobertura inexistente.**
  - **NO CUBIERTO — no existe cobertura propia.** El `#[source]` del sitio 618 está puesto (`connection_from`), pero `http::request::Builder::body` sólo falla si el builder acumuló un error previo (URI, método o cabecera inválidos). Las tres entradas que recibe `build_request` son: un `Method::GET` constante, el target de `request_target()` (el `container_name` va percent-encoded con `urlencoding::encode`) y el `Host` de `host_header()` —que sí deriva de `tcp_endpoint`, ya validado—; con esas tres entradas el builder no puede quedar en estado de error y `body()` no puede devolver `Err`. Un test aquí probaría la crate `http`, no shuul. Se descarta por no aportar.
- [x] 2.7 RED/GREEN 690 (`base_url.parse`): test con una URL que no parsea; assertar `source()` es el `http::uri::InvalidUri`. Verificación: el test pasa.
  - Cubierto por `test_invalid_gateway_url_exposes_the_invalid_uri` (determinista y trivial).
- [x] 2.8 RED/GREEN 771 (`parse_journal_entry`): test con línea no JSON; assertar `source()` es el `serde_json::Error` con `line()`/`column()`. Verificación: el test pasa.
  - Cubierto por `test_parse_failure_exposes_the_serde_json_error`.
- [x] 2.9 RED/GREEN 1117 (`parse_traefik_access_log`): test con `MESSAGE` no JSON; assertar `source()` es el `serde_json::Error`. Verificación: el test pasa.
  - Cubierto por `test_traefik_parse_failure_exposes_the_serde_json_error`.
- [x] 2.10 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 3. Sin regresión en los diagnósticos

- [x] 3.1 RED/GREEN: test de que un fallo sin causa (p. ej. URL sin esquema) sigue con `source() == None` y su `Display` es exactamente el de hoy. Verificación: el test pasa.
  - Cubierto por `test_context_only_errors_have_no_source_and_keep_their_display`: afirma el `Display` carácter a carácter en dos variantes (`Parse("empty journal entry")` → `"Journal entry parse error: empty journal entry"`; `Connection(…has no scheme)` → `"Connection error: gateway URL localhost:19531 has no scheme"`).
- [x] 3.2 RED/GREEN: test de que la cadena se recorre entera — `source()` del `source()` cuando el error de origen envuelve a otro (p. ej. el `io::Error` de un `hyper::Error`). Verificación: el test pasa.
  - Cubierto por `test_the_source_chain_is_walkable_end_to_end`: `source()` → `source()` del `source()` → `None` al final, con un error de test que envuelve un `io::Error`.
- [x] 3.3 RED/GREEN: test de que los sitios sin causa (`447`, `767`, `696`) siguen devolviendo la misma variante y el mismo texto que antes del cambio. Verificación: el test pasa.
  - `767` y `696` en `test_context_only_errors_have_no_source_and_keep_their_display`; `447` en `test_gateway_error_status_keeps_its_variant_and_text` (un gateway que responde 400 produce `Connection("gateway returned HTTP 400 Bad Request", None)`).
- [x] 3.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 4. Verificación

- [x] 4.1 `cargo test` en verde (hoy 273 tests: anotar exactamente cuántos nuevos se añaden con este cambio).
  - Evidencia: 273 → **284** tests, **+11** nuevos. Verificado por CLI, 0 fallos.
- [x] 4.2 `cargo clippy -- -D warnings` y `cargo fmt --check` sin warnings ni diferencias.
  - Evidencia: `clippy -- -D warnings` limpio y `fmt --check` sin diff.
- [x] 4.3 `openspec validate journal-error-source-chain`.
  - Evidencia: `Change 'journal-error-source-chain' is valid`.
- [x] 4.4 Auditoría con `rust-reviewer` sobre `backend/src/journald_reader.rs`; resolver o documentar lo que salga.
  - Evidencia: **APTO CON CAMBIOS**; los tres hallazgos aplicados. La auditoría confirmó además, sin hallazgo, que el texto de los 9 sitios con causa es byte a byte el de `8a16ef0` (Opción 1 respetada) y que las dos exenciones de `hyper` (553 y 618) son legítimas.
- [x] 4.5 Impacto operativo: confirmar que con la opción elegida en `design.md` §D3 los `debug!`/`warn!` de `:483`, `:638` y `:643` no pierden información respecto a hoy. Si se eligió la Opción 2, esto obliga a reescribir esos tres sitios y sus tests. Verificación: comparar el texto de los tres logs antes y después sobre una misma entrada.
  - Evidencia: satisfecha por construcción — los cuatro sitios de log siguen imprimiendo `{e}` con `Display` y, como el primer campo no cambia, no pierden un carácter. La auditoría lo verificó punto por punto.

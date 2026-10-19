# Tasks: journal-error-source-chain

## 1. Contrato

- [ ] 1.1 RED: aserción estática de que `JournalError: Send + Sync + 'static`, y tests de que `source()` devuelve la causa correcta en los escenarios 1–5 del delta (JSON inválido → `serde_json::Error` con `line()`/`column()`; access log de Traefik no JSON → `serde_json::Error`; socket Unix inexistente → `io::Error` con `kind() == NotFound`; puerto TCP cerrado → `io::Error` con `kind() == ConnectionRefused`; URL inválida → `InvalidUri`). Verificación: `cargo test journald_reader` **no compila hoy**, porque las variantes todavía tienen un solo campo y `source()` devuelve siempre `None`.
- [ ] 1.2 GREEN: añadir a las cuatro variantes el segundo campo `#[source] Option<Box<dyn std::error::Error + Send + Sync>>` y los ocho constructores de ayuda (`socket`/`socket_from`, `parse`/`parse_from`, `traefik_parse`/`traefik_parse_from`, `connection`/`connection_from`). Verificación: el módulo compila y la aserción estática de 1.1 pasa.
- [ ] 1.3 GREEN: reescribir los 23 sitios sin causa a los constructores sin fuente (447, 566, 696, 701, 706, 714, 767, 774, 787) y migrar los tests que construyen las variantes a mano (865, 871, 877, 883) y los `matches!`/`Err(...)` de 942, 1301, 1442, 2486, 2502. Verificación: `cargo test journald_reader` en verde y los tests de `source() == None` de la tarea 3.1 pasan.
- [ ] 1.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.

## 2. Causa en los 9 puntos

- [ ] 2.1 RED/GREEN 471 (`body.frame()`): test de que un frame de error de `hyper` produce un `JournalError::Socket` cuyo `source()` es un `hyper::Error`. Verificación: el test pasa. **Si forzar el fallo de `frame()` de forma limpia no es posible, decirlo explícitamente en esta tarea en vez de declarar una cobertura que no existe.**
- [ ] 2.2 RED/GREEN 553 (handshake HTTP): test con un `TcpListener` de `tokio` que acepta la conexión y la cierra a media respuesta, de modo que `http1::handshake` falle; assertar que `source()` es el `hyper::Error`. Verificación: el test pasa. **Si no se puede cubrir de forma limpia, decirlo en vez de declarar cobertura inexistente.**
- [ ] 2.3 RED/GREEN 559 (`send_request`): test con un `TcpListener` que corta la conexión antes de completar la respuesta, de modo que `send_request` falle; assertar `source()` es el `hyper::Error`. Verificación: el test pasa. **Si no se puede cubrir de forma limpia, decirlo en vez de declarar cobertura inexistente.**
- [ ] 2.4 RED/GREEN 580 (`UnixStream::connect`): test contra un path que no existe; assertar `source()` es el `io::Error` con `kind() == NotFound`. Verificación: el test pasa.
- [ ] 2.5 RED/GREEN 592 (`TcpStream::connect`): test contra un puerto cerrado; assertar `source()` es el `io::Error` con `kind() == ConnectionRefused`. Verificación: el test pasa.
- [ ] 2.6 RED/GREEN 618 (`builder.body`): test de la construcción de la petición cuando falla; assertar `source()` es el `http::Error`. Verificación: el test pasa. **Si forzar el fallo de `body()` de forma limpia no es posible, decirlo en vez de declarar cobertura inexistente.**
- [ ] 2.7 RED/GREEN 690 (`base_url.parse`): test con una URL que no parsea; assertar `source()` es el `http::uri::InvalidUri`. Verificación: el test pasa.
- [ ] 2.8 RED/GREEN 771 (`parse_journal_entry`): test con línea no JSON; assertar `source()` es el `serde_json::Error` con `line()`/`column()`. Verificación: el test pasa.
- [ ] 2.9 RED/GREEN 1117 (`parse_traefik_access_log`): test con `MESSAGE` no JSON; assertar `source()` es el `serde_json::Error`. Verificación: el test pasa.
- [ ] 2.10 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 3. Sin regresión en los diagnósticos

- [ ] 3.1 RED/GREEN: test de que un fallo sin causa (p. ej. URL sin esquema) sigue con `source() == None` y su `Display` es exactamente el de hoy. Verificación: el test pasa.
- [ ] 3.2 RED/GREEN: test de que la cadena se recorre entera — `source()` del `source()` cuando el error de origen envuelve a otro (p. ej. el `io::Error` de un `hyper::Error`). Verificación: el test pasa.
- [ ] 3.3 RED/GREEN: test de que los sitios sin causa (`447`, `767`, `696`) siguen devolviendo la misma variante y el mismo texto que antes del cambio. Verificación: el test pasa.
- [ ] 3.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 4. Verificación

- [ ] 4.1 `cargo test` en verde (hoy 273 tests: anotar exactamente cuántos nuevos se añaden con este cambio).
- [ ] 4.2 `cargo clippy -- -D warnings` sin warnings nuevos.
- [ ] 4.3 `cargo fmt --check` sin diferencias.
- [ ] 4.4 Auditoría con `rust-reviewer` sobre `backend/src/journald_reader.rs`; resolver o documentar lo que salga.
- [ ] 4.5 Impacto operativo: confirmar que con la opción elegida en `design.md` §D3 los `debug!`/`warn!` de `:483`, `:638` y `:643` no pierden información respecto a hoy. Si se eligió la Opción 2, esto obliga a reescribir esos tres sitios y sus tests. Verificación: comparar el texto de los tres logs antes y después sobre una misma entrada.

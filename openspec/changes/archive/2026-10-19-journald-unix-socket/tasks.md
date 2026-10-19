# Tasks: journald-unix-socket

> **Auditoría (rust-reviewer):** APTO tras aplicar M-1…M-6 y M-8…M-12.
> M-7 (`JournalError` sin `#[source]`) queda como deuda técnica documentada.

## 1. Transporte y configuración

- [x] 1.1 RED: tests de `from_values` con la precedencia socket → URL → default y de `from_env` con las tres variables. Verificación: `cargo test journald_reader` falla porque `JournaldTransport` todavía no existe.
- [x] 1.2 GREEN: añadir `hyper`, `hyper-util`, `http-body-util` y `bytes` a `backend/Cargo.toml`; implementar `JournaldTransport`, `new`, `unix`, `from_values`, `from_env` y el warning de configuración ambigua. Verificación: los tests de 1.1 pasan.
- [x] 1.3 GREEN: implementar `request_target()` (mismo path en ambos transportes, nombre de contenedor URL-encoded) y `endpoint_display()`. Verificación: tests de 1.1 que cubren ambas funciones pasan.
- [x] 1.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.

## 2. Capa de conexión con hyper

- [x] 2.1 RED: test del helper de conexión contra un `UnixListener` temporal y contra un `TcpListener`; un path inexistente debe dar `JournalError::Connection`. Verificación: el test falla por API inexistente.
- [x] 2.2 GREEN: implementar la conexión con `UnixStream::connect` / `TcpStream::connect`, el objeto `Box<dyn Io>` y el `CONNECT_TIMEOUT` de 10 s. Verificación: el test de 2.1 pasa.
- [x] 2.3 GREEN: sustituir `reqwest` por `hyper::client::conn::http1::handshake` + `TokioIo`, con `tokio::spawn` de la `Connection`, petición origin-form con `Host`, `Accept: application/json` y `Range` cuando haya cursor. Verificación: todos los tests de `run()` y los escenarios existentes en verde.
- [x] 2.4 RED/GREEN: test del camino "conexión aceptada pero handshake mudo" que debe abortar tras `CONNECT_TIMEOUT` con un `retry_delay` corto. Verificación: el test pasa sin dejar tareas colgadas.
- [x] 2.5 REFACTOR: eliminar `reqwest` del módulo; `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 3. Streaming del cuerpo

- [x] 3.1 RED: gateway Unix falso que envía el NDJSON en dos chunks partiendo una línea por la mitad; el lector debe reconstruirla y entregarla al pipeline Jail. Verificación: el test falla sin la lectura por frames.
- [x] 3.2 GREEN: consumir el cuerpo con `BodyExt::frame()` y alimentar `drain_ndjson_lines` conservando las líneas parciales. Verificación: el test de 3.1 pasa y no se rompe ningún test TCP.
- [x] 3.3 RED/GREEN: test de reanudación por socket Unix que asserta que el falso gateway recibe `Range: entries=<cursor>:1` y que no se reprocesa la entrada del cursor. Verificación: el test pasa.
- [x] 3.4 RED/GREEN: test de shutdown que interrumpe un `frame()` pendiente y asserta `Ok(())`. Verificación: el test pasa y termina en menos de un segundo.
- [x] 3.5 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 4. Despliegue y documentación

- [x] 4.1 `compose.yml`: montar `/run/systemd/journal:/run/systemd/journal:ro`, definir `SHUUL_JOURNALD_SOCKET` y eliminar `extra_hosts` y `SHUUL_JOURNALD_URL`. Verificación: `docker compose config` sin errores y el servicio conserva `SHUUL_JOURNALD_CONTAINER`.
- [x] 4.2 `SELF-HOSTING.md` §2: drop-in del socket, `SocketMode`, verificación con `ls -l` y `curl --unix-socket`, y el modo TCP como alternativa para gatewayd remoto. Verificación: los comandos son copiables y mencionan el path exacto.
- [x] 4.3 `AGENTS.md`: actualizar la descripción del transporte del reader y la tabla de variables de entorno. Verificación: `grep -n SHUUL_JOURNALD_SOCKET AGENTS.md SELF-HOSTING.md compose.yml` encuentra las tres apariciones.

## 5. Verificación de integración

- [x] 5.1 `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` en verde, sin tests ignorados nuevos.
- [x] 5.2 Contra un `systemd-journal-gatewayd` real del host: con el socket montado, confirmar en logs `streaming access logs from unix:/…` y que aparecen eventos en `GET /api/v1/logs`. (verificado en producción: 3487 eventos del pipeline `jail` y el socket `root:systemd-journal 0660` montado en el contenedor)
- [x] 5.3 Dejar escrito el rollback (quitar volumen, restaurar `SHUUL_JOURNALD_URL` y `extra_hosts`, recrear) en `SELF-HOSTING.md`. Verificación: la sección existe y coincide con el Migration Plan del design.

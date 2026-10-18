# Tasks: journald-reader

> **Transport revisado (2026-10-18):** `systemd-journal-gatewayd` por HTTP streaming
> (`Accept: application/json`, NDJSON) en lugar de leer el socket de journald,
> que es *write-only*. Las Tasks 1 y 2 se rehacen para reflejarlo.
>
> **Nota Task 9:** con este transporte no hacen falta volúmenes de socket. El
> contenedor sólo necesita red hacia el `:19531` del host, así que la entrega
> es `extra_hosts` + variables de entorno en `compose.yml` (además de la
> documentación en `SELF-HOSTING.md`).

## TDD Checklist

### Task 1: Módulo journald_reader — struct + errores
- [x] RED: Test del struct `JournaldReader` con campos esperados
- [x] GREEN: Crear `backend/src/journald_reader.rs` con struct, constructor y `JournalError`
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`
- [x] **REVISIÓN**: sustituir `socket_path` por `gateway_url` (default `http://localhost:19531`)
- [x] **REVISIÓN**: RED → actualizar tests del struct a `gateway_url`
- [x] **REVISIÓN**: GREEN → aplicar el cambio y `cargo test`
- [x] **REVISIÓN**: REFACTOR → `cargo clippy -- -D warnings` + `cargo fmt`

### Task 2: parse_journal_entry (Journal JSON Format / NDJSON)
- [x] RED: Tests de parseo del Journal Export Format (formato antiguo)
- [x] GREEN: Implementación del parser de export format
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`
- [x] **REVISIÓN**: RED → reescribir tests para NDJSON: objeto válido, multi-valor
      (primer elemento), binario/nulos ignorados, línea inválida y vacía
- [x] **REVISIÓN**: GREEN → reimplementar `parse_journal_entry()` con `serde_json`
- [x] **REVISIÓN**: REFACTOR → `cargo clippy -- -D warnings` + `cargo fmt`

### Task 3: Filtro por container_name
- [x] RED: Test con `CONTAINER_NAME=traefik` → pasa el filtro
- [x] RED: Test con `CONTAINER_NAME=nginx` → descartada
- [x] GREEN: Implementar `is_traefik_entry()`
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 4: parse_traefik_access_log
- [x] RED: Test con JSON completo de Traefik (todos los campos del `ReportPayload`)
- [x] RED: Test con campos opcionales ausentes → `None`
- [x] RED: Test con JSON inválido → error
- [x] RED: Test `OriginStatus: 0` → `Ok(None)` (nunca llegó al backend)
- [x] RED: Test fallback `ClientAddr` (IPv4 e IPv6) sin puerto
- [x] GREEN: Implementar `parse_traefik_access_log()` con `serde`
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 5: Refactorizar report_handler → run_jail_pipeline
- [x] RED: Test que llama a `run_jail_pipeline` directamente (sin HTTP)
- [x] RED: Test de `PendingBan` creado sin HTTP
- [x] RED: Test que confirma que `report_handler` delega
- [x] RED: Test de `cleared` sin reglas que matcheen
- [x] GREEN: Extraer la lógica a `run_jail_pipeline(app_state, payload)`
- [x] REFACTOR: Todos los tests de `report.rs` siguen pasando
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 6: extract_report_payload
- [x] RED: Test con entrada de Traefik + `MESSAGE` válido → `Ok(Some(payload))`
- [x] RED: Test con `CONTAINER_NAME` distinto → `Ok(None)`
- [x] RED: Test sin `MESSAGE` → `Ok(None)`
- [x] RED: Test con `MESSAGE` cuyo `OriginStatus` es 0 → `Ok(None)`
- [x] GREEN: Implementar `extract_report_payload()`
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 7: JournaldReader::run (HTTP streaming + reconexión)
- [x] RED: Test del helper de buffer NDJSON (líneas parciales entre chunks)
- [x] RED: Test de streaming contra un servidor HTTP de prueba (NDJSON + cierre)
- [x] RED: Test de graceful shutdown vía canal `watch`
- [x] RED: Test de gateway no disponible → warning + reintento, sin crash
- [x] GREEN: Implementar `JournaldReader::run()` con `reqwest`, `select!` y backoff
- [x] GREEN: Reanudar con `Range: entries=<cursor>:1:` usando el último `__CURSOR`
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 8: Integrar en Main como background task
- [x] RED: Test que verifica la integración del reader en el arranque
- [x] GREEN: Arrancar `JournaldReader::default().run(...)` en `main.rs` con shutdown
- [x] GREEN: Leer `SHUUL_JOURNALD_URL` / `SHUUL_JOURNALD_CONTAINER` (con defaults)
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 9: Docker + documentación
- [x] GREEN: Documentar en `SELF-HOSTING.md`: habilitar `systemd-journal-gatewayd.socket`,
      `Storage=volatile` y cómo dar acceso de red al contenedor
- [x] GREEN: Aviso de seguridad (gatewayd no autentica; exponerlo sólo en red privada)
- [x] GREEN: Documentar `SHUUL_JOURNALD_URL` y `SHUUL_JOURNALD_CONTAINER` en README/AGENTS
- [x] REFACTOR: Verificar que el contenedor arranca sin gatewayd (modo degradado)

### Final verification
- [x] `cargo test` all pass
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo fmt --check` clean
- [x] `cargo check` clean
- [x] Revisar que todos los escenarios de la spec están cubiertos por tests
- [x] Archivar el change proposal en `openspec/specs/`

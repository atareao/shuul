# Tasks: fix-whitelist-blacklist-response

## TDD Checklist

### RED
- [x] Escribir test de integración que verifique que `GET /api/v1/whitelist` devuelve `PagedResponse`
- [x] Escribir test de integración que verifique que `GET /api/v1/blacklist` devuelve `PagedResponse`
- [x] Verificar que los tests fallan (RED) con `cargo test`

### GREEN
- [x] Modificar `list_whitelist` en `backend/src/http/whitelist.rs` para devolver `PagedResponse`
- [x] Modificar `list_blacklist` en `backend/src/http/blacklist.rs` para devolver `PagedResponse`
- [x] Verificar que los tests pasan (GREEN) con `cargo test`

### REFACTOR
- [x] `cargo clippy -- -D warnings`
- [x] `cargo fmt --check`
- [x] `cargo test` (confirmar 100% green)

### ARCHIVE
- [x] `openspec archive fix-whitelist-blacklist-response`
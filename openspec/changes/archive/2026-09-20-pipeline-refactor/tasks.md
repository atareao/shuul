# Tasks: pipeline-refactor

## TDD Checklist

### RED
- [x] Escribir test de integración que verifique WAF rule allow antes que ban check
- [x] Escribir test que verifique que no hay whitelist/blacklist en should_log()
- [x] Verificar que los tests fallan (RED) con `cargo test --lib`

### GREEN — Backend
- [x] Eliminar `backend/src/models/whitelist.rs`
- [x] Eliminar `backend/src/models/blacklist.rs`
- [x] Eliminar referencias a whitelist/blacklist en `backend/src/models/mod.rs` (mod, pub use, AppState fields, reload methods)
- [x] Eliminar `backend/src/http/whitelist.rs`
- [x] Eliminar `backend/src/http/blacklist.rs`
- [x] Eliminar referencias en `backend/src/http/mod.rs` (mod, pub use)
- [x] Eliminar migraciones `20260920000000_add_whitelist_blacklist.up.sql` y `20260920000001_add_whitelist_blacklist_unique.up.sql`
- [x] Reordenar pipeline en `backend/src/http/shuul.rs`: WAF rules → Ban check
- [x] Eliminar categorías `whitelist`/`blacklist` de `should_log()`
- [x] Eliminar rutas whitelist/blacklist e imports en `backend/src/main.rs`
- [x] Eliminar `safe_paths`, `trusted_ips`, `trusted_user_agents` de Settings seed en migración inicial

### GREEN — Frontend
- [x] Eliminar `frontend/src/pages/admin/whitelist_page.tsx`
- [x] Eliminar `frontend/src/pages/admin/blacklist_page.tsx`
- [x] Eliminar `frontend/src/models/whitelist.ts`
- [x] Eliminar `frontend/src/models/blacklist.ts`
- [x] Eliminar lazy imports y rutas en `frontend/src/App.tsx`
- [x] Eliminar navegación en `frontend/src/layouts/admin_layout.tsx`

### GREEN — Verificación
- [x] `cargo check` (sin errores de compilación)
- [x] `cargo test --lib` (tests pasan)
- [x] `cargo clippy -- -D warnings`
- [x] `cargo fmt --check`

### REFACTOR
- [x] `cargo clippy -- -D warnings`
- [x] `cargo fmt --check`
- [x] `cargo test --lib` (confirmar 100% green)

### ARCHIVE
- [x] `openspec archive pipeline-refactor`
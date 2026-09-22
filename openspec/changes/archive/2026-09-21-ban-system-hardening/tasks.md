# Tasks: ban-system-hardening

## Task 1: BanManager::is_banned_for_rule

- [ ] RED: Escribir tests para `is_banned_for_rule` en `ban_manager.rs`
  - [ ] Test: IP baneada para rule_id específico retorna true
  - [ ] Test: IP baneada para rule_id diferente retorna false
  - [ ] Test: IP baneada sin rule_id (None) retorna true para None
  - [ ] Test: IP baneada sin rule_id retorna false para Some
  - [ ] Test: IP no baneada retorna false
  - [ ] Test: IP con ban expirado retorna false
- [ ] GREEN: Implementar `is_banned_for_rule` en `BanManager`
- [ ] REFACTOR: Clippy + cargo check

## Task 2: Fix is_banned per-rule en report.rs

- [ ] RED: Escribir test de integración para report_handler con múltiples reglas
  - [ ] Test: IP baneada por regla A permite ban por regla B
  - [ ] Test: IP baneada por regla A no bloquea regla A (misma regla)
  - [ ] Test: IP sin bans procesa todas las reglas
- [ ] GREEN: Cambiar `is_banned(&ip)` por `is_banned_for_rule(&ip, Some(*rule_id))` en report.rs
- [ ] REFACTOR: Clippy + cargo check

## Task 3: load_from_db usa multipliers del perfil

- [ ] RED: Escribir tests para load_from_db con perfil
  - [ ] Test: load_from_db usa multipliers del perfil Path Scanning
  - [ ] Test: load_from_db usa defaults cuando el perfil no existe
  - [ ] Test: load_from_db con múltiples perfiles distintos
- [ ] GREEN: Modificar load_from_db para leer perfil vía JOIN
  - [ ] Modificar query SQL para JOIN con rules y rate_limit_profiles
  - [ ] Extraer bantime_multipliers, bantime_maxtime, ban_count_decay_days del perfil
  - [ ] Fallback a defaults si el perfil no existe
- [ ] REFACTOR: Clippy + cargo check

## Task 4: load_from_db limpia duplicados legacy en DB

- [x] RED: Escribir tests para limpieza de duplicados
  - [x] Test: load_from_db marca duplicados como expired
  - [x] Test: load_from_db no afecta bans únicos
- [x] GREEN: Añadir UPDATE de duplicados en load_from_db
- [x] REFACTOR: Clippy + cargo check

## Task 5: Migración rate_limiter_state

- [ ] RED: Escribir test de migración (verificar que la tabla se crea)
- [ ] GREEN: Crear migración SQL `20260922000000_create_rate_limiter_state.up.sql`
  - [ ] CREATE TABLE rate_limiter_state
  - [ ] PRIMARY KEY (profile_id, ip_address)
- [ ] REFACTOR: Verificar que sqlx detecta la migración

## Task 6: RateLimiter::save y RateLimiter::load

- [x] RED: Escribir tests para save/load
  - [x] Test: RateLimiter se persiste y recupera tras reinicio simulado
  - [x] Test: RateLimiter vacío no persiste nada
  - [x] Test: load con profile_id sin datos retorna RateLimiter vacío
  - [x] Test: Timestamps expirados se filtran al cargar
- [x] GREEN: Implementar `RateLimiter::save(pool, profile_id)`
  - [x] Serializar CircularTimestamps a JSON
  - [x] UPSERT en rate_limiter_state
- [x] GREEN: Implementar `RateLimiter::load(pool, profile_id, max_retry, find_time)`
  - [x] Deserializar JSON a CircularTimestamps
  - [x] Filtrar timestamps expirados
- [x] REFACTOR: Clippy + cargo check

## Task 7: Background task de persistencia

- [ ] RED: Escribir test de integración para background task
  - [ ] Test: Background task persiste rate limiters periódicamente
- [ ] GREEN: Añadir background task en main.rs
  - [ ] tokio::spawn con interval de 60s
  - [ ] Itera rate_limiters y llama a save()
- [ ] REFACTOR: Clippy + cargo check

## Task 8: Carga de rate limiters al arrancar

- [x] RED: Escribir test de integración para carga al arrancar
  - [x] Test: Al arrancar se cargan rate limiters para todos los profile_ids activos
- [x] GREEN: Modificar main.rs para cargar rate limiters persistidos
  - [x] Colectar profile_ids activos de reglas Jail
  - [x] Llamar RateLimiter::load() para cada uno
- [x] REFACTOR: Clippy + cargo check

## Task 9: Multipliers más agresivos en templates

- [x] RED: Escribir tests para nuevos multipliers
  - [x] Test: Templates tienen multipliers [1,2,4,8,16,32,64,128]
  - [x] Test: Seed data se actualiza con nuevos multipliers
- [x] GREEN: Actualizar templates.rs
  - [x] Cambiar escalation_multipliers en todos los RateLimitProfileTemplate
- [x] GREEN: Actualizar migration seed data
  - [x] UPDATE rate_limit_profiles SET bantime_multipliers = '[1,2,4,8,16,32,64,128]'
- [x] REFACTOR: Clippy + cargo check

## Task 10: Consolidación

- [ ] Ejecutar `cargo test` completo (100% green)
- [ ] Ejecutar `cargo clippy -- -D warnings` (0 warnings)
- [ ] Ejecutar `cargo fmt --check`
- [ ] Marcar todas las tasks como completadas
- [ ] Ejecutar `openspec archive ban-system-hardening`
# PLAN.md — Optimización de CPU en pipeline WAF sin reglas activas

## Contexto / Problema
Shuul consume ~4% de CPU aunque no haya ninguna regla habilitada. El endpoint WAF `POST /api/v1/shuul` (backend/src/http/shuul.rs) ejecuta trabajo innecesario por cada petición:

1. `NewRequest::from_request()` (backend/src/models/new_request.rs:39) llama SIEMPRE a `IPData::complete()` (backend/src/models/ipdata.rs:20), que hace un lookup binario en el MaxMind GeoLite2-City.mmdb (~50-100MB) por cada request, aunque ninguna regla use campos GeoIP (city_name, country_name, country_code).
2. En shuul.rs se compila `Regex::new(safe_path)` y `Regex::new(trusted_ua)` en cada petición para cada patrón (líneas 58-71 y 92-106). Compilar regex es ~10-100x más caro que matchear.
3. `record_allowed()` (backend/src/models/stats.rs:196) adquiere 3 Mutex y hace escaneo lineal de buckets por cada request permitida. Secundario.

No hay busy-loop: las background tasks de main.rs (OIDC cada 30s, ban cleanup cada 60s, stats persist cada 1800s) son despreciables.

## Tareas

### T1 — GeoIP lookup lazy (mayor impacto)
- Modificar `NewRequest::from_request` en backend/src/models/new_request.rs para aceptar `maxmind_db: Option<&Reader<Vec<u8>>>`. Cuando sea `None`, saltar el lookup de MaxMind y dejar city_name/country_name/country_code a None.
- En backend/src/http/shuul.rs, antes de construir el request, comprobar si alguna regla cacheada usa filtros GeoIP (rule.city_name.is_some() || rule.country_name.is_some() || rule.country_code.is_some()). Si ninguna los usa, pasar None.
- El único call-site de from_request es shuul.rs:43. El endpoint util.rs usa IPData::complete directamente y NO se toca. report.rs construye NewRequest manualmente sin GeoIP y NO se toca.

### T2 — Precompilar regex de settings
- En backend/src/models/settings.rs, añadir campos `safe_paths_re: Vec<Regex>` y `trusted_user_agents_re: Vec<Regex>` a la struct Settings.
- Recompilar estos campos en `Settings::load()` y cuando el PUT de settings (backend/src/http/settings.rs, update_settings) actualice safe_paths/trusted_user_agents (crear helper `fn recompile(&mut self)` o similar).
- En shuul.rs, sustituir `Regex::new(safe_path)` por usar `settings.safe_paths_re` precompiladas, y `Regex::new(trusted_ua)` por `settings.trusted_user_agents_re`. Mantener la semántica: una regex inválida se ignora (log warn) pero ya no se compila en caliente.

### T3 — Verificación
- `cargo build` y `cargo test` en backend/ pasan.
- `cargo clippy -- -D warnings` y `cargo fmt --check` en backend/ pasan.
- No romper los tests existentes (tests/rule_test.rs, backend/tests/integration.rs, backend/tests/rate_limiting_test.rs).
- Revisión final del rust-reviewer.

## Notas de arquitectura
- Mantener el invariance de AGENTS.md: todos los MutexGuard se liberan antes de cualquier await; orden de locks rules → rate_limiter → ban_manager.
- CacheRule no tiene rate_limit field; read_all_active() = SELECT * FROM rules WHERE active = TRUE.
- El comportamiento WAF no cambia: safe paths, trusted IPs, trusted UAs → ALLOW inmediato; ban → 403; primera regla que matchea gana.
# Tasks: restore-escalation-on-startup

## TDD Checklist

### RED
- [ ] Escribir test: `test_load_from_db_restores_escalation_counts` — verifica que `escalation_counts` se restaura con el nivel máximo por IP
- [ ] Escribir test: `test_load_from_db_empty_db` — verifica que con BD vacía, `escalation_counts` está vacío
- [ ] Escribir test: `test_load_from_db_single_ban` — verifica que una IP con un solo ban restaura su nivel
- [ ] Escribir test: `test_load_from_db_multiple_ips` — verifica que múltiples IPs restauran sus niveles
- [ ] Ejecutar `cargo test` y confirmar que los nuevos tests fallan (RED)

### GREEN
- [ ] Modificar `load_from_db()` para restaurar `escalation_counts` con el nivel máximo por IP
- [ ] Ejecutar `cargo test` y confirmar que todos los tests pasan (GREEN)
- [ ] Ejecutar `cargo check` y confirmar que no hay errores de compilación

### REFACTOR
- [ ] Ejecutar `cargo clippy -- -D warnings` y corregir cualquier warning
- [ ] Ejecutar `cargo fmt --check` y corregir formato
- [ ] Ejecutar `cargo test` para confirmar que no hay regresiones

### ARCHIVE
- [ ] Ejecutar `openspec archive restore-escalation-on-startup`
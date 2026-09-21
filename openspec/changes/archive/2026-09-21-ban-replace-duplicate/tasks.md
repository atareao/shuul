# Tasks

## 1. Migración SQL

- [x] 1.1 Crear migración `20260921000000_add_unique_ip_rule_id.up.sql` que añada índice UNIQUE(ip_address, rule_id) y limpie duplicados legacy (marcar expired=1 en todos excepto el más reciente por par IP+rule_id). Verificar con `cargo test` que la migración se aplica sin errores.

## 2. Modificar BanManager::ban() — reemplazo en memoria

- [x] 2.1 Implementar lógica de reemplazo en `ban()`: buscar ban activo con mismo `rule_id` en el Vec y reemplazarlo, o push si no existe. Verificar con tests unitarios:
  - `test_ban_replaces_existing_active_same_rule_id`
  - `test_ban_adds_new_for_different_rule_id`
  - `test_ban_adds_new_for_none_rule_id`
  - `test_ban_adds_new_when_existing_expired`

## 3. Modificar persist_ban() — UPDATE+INSERT

- [x] 3.1 Implementar `persist_ban()` con UPDATE primero + INSERT condicional. Verificar con test de integración que no se crean duplicados en DB.

## 4. Modificar load_from_db() — deduplicación legacy

- [x] 4.1 Modificar `load_from_db()` para que, si hay múltiples filas para mismo IP+rule_id, cargue solo la más reciente (por `banned_at`). Verificar con test que carga el ban correcto.

## 5. Verificación final

- [x] 5.1 Ejecutar `cargo test` completo y verificar que todos los tests pasan (nuevos + existentes).
- [x] 5.2 Ejecutar `cargo clippy -- -D warnings` y verificar cero warnings.
- [x] 5.3 Ejecutar `cargo fmt --check` y verificar formato correcto.
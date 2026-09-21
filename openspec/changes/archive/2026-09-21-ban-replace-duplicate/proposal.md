# Proposal: Replace duplicate bans for same IP + rule_id

## Why

Actualmente `BanManager::ban()` siempre hace `push()` de un nuevo `BanInfo` al Vec de la IP, incluso si ya existe un ban activo con el mismo `rule_id`. Esto genera duplicados en memoria y en DB que no aportan valor: el escalado funciona por IP pero los bans antiguos quedan huérfanos hasta expirar. Cada infracción debería **reemplazar** el ban anterior con uno nuevo de mayor duración (escalado), no acumular duplicados.

## What Changes

- `BanManager::ban()`: si ya existe un `BanInfo` activo (no expirado) con el mismo `rule_id` para la IP, reemplazarlo en lugar de hacer push. Se actualizan `banned_at`, `ban_duration_seconds`, `escalation_level` y `reason`.
- `BanManager::persist_ban()`: usar `INSERT OR REPLACE` (UPSERT) para mantener una sola fila activa por IP+rule_id en la tabla `bans`.
- `BanManager::load_from_db()`: si hay múltiples filas para mismo IP+rule_id (legacy), conservar solo la más reciente.
- Se añade un índice `UNIQUE(ip_address, rule_id)` en la tabla `bans` para evitar duplicados a nivel DB.

## Capabilities

### Modified Capabilities

- `ban-manager`: El comportamiento de `ban()` cambia de "push siempre" a "reemplazar si existe activo". El endpoint `POST /api/v1/bans` ya no crea duplicados. El endpoint `DELETE /api/v1/bans` (unban) sigue funcionando igual: elimina el ban para esa IP+rule_id.

## Impact

- **backend/src/models/ban_manager.rs**: Modificar `ban()`, `persist_ban()`, `load_from_db()`. Añadir método auxiliar `find_active_ban()`.
- **backend/migrations/**: Nueva migración para añadir índice UNIQUE y limpiar duplicados legacy.
- **Tests**: Actualizar tests existentes en `ban_manager.rs` que verifican `push()` y añadir nuevos tests para el comportamiento de reemplazo.
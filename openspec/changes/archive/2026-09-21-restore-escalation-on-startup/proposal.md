# Restore escalation counters on startup

## Intent

`BanManager::load_from_db()` carga los bans activos desde la BD con su `escalation_level` histórico, pero **no restaura** el mapa `escalation_counts` en memoria. Esto provoca que tras un reinicio del servidor, el contador de escalada empiece desde 0, ignorando todo el historial de reincidencia de la IP.

## Scope

- `backend/src/models/ban_manager.rs` — solo `load_from_db()`

## Impact

- Tras reinicio, el `escalation_level` del próximo ban reflejará el nivel histórico acumulado
- Las IPs reincidentes no "empiezan de cero" tras un restart
- No hay cambios en la API, frontend, ni BD

## Spec delta

Ver `specs/ban-manager/spec.md`
# Proposal: Tor Network IP Filter

## Why

Los operadores de Shuul necesitan detectar y bloquear tráfico proveniente de la red Tor (The Onion Router), que frecuentemente se usa para eludir restricciones geográficas, realizar scraping malicioso, o ataques automatizados. Actualmente no existe un filtro específico para identificar IPs de Tor exit nodes.

## What Changes

- Nueva columna `is_tor` en la tabla `rules` (INTEGER, NOT NULL DEFAULT 0)
- Nuevo campo `is_tor: Option<bool>` en los modelos `Rule`, `NewRule`, `UpdateRule`, `CacheRule`
- Nuevo campo `is_tor: Option<bool>` en `NewRequest`, poblado por un servicio de detección Tor
- Nuevo servicio `TorService` que descarga periódicamente la lista de Tor exit nodes (`https://check.torproject.org/torbulkexitlist`) y mantiene un `HashSet<IpAddr>` en memoria
- Inicialización lazy: TorService arranca con lista vacía, primer refresh a los 30s
- Background task que refresca la lista cada 30 minutos (1800s)
- Lógica de matching: si `rule.is_tor = true`, la regla solo matchea si `request.is_tor = Some(true)`
- Frontend: `is_tor` como Switch en el tab Network del RuleDialog
- Nueva migración SQL para añadir la columna

## Capabilities

### New Capabilities
- `tor-filter`: Detección de IPs de Tor exit nodes mediante lista bulk descargada periódicamente. Incluye el servicio de refresh y el lookup durante la construcción de `NewRequest`.

### Modified Capabilities
- `waf-pipeline`: Nuevo requisito "Tor filter matching" que añade `is_tor` como filtro booleano en la lógica de matching de reglas WAF.

## Impact

- Backend: nuevo módulo `backend/src/services/tor_service.rs`, modificación de `rule.rs`, `new_request.rs`, `CacheRule`, `mod.rs` (AppState), `shuul.rs`, `report.rs`, `main.rs`
- Frontend: modificación de `rule.ts` (interface), `rule_dialog.tsx` (Switch en Network tab)
- Database: nueva migración `20260920000000_add_is_tor_column.up.sql`
- Templates: sin impacto (el filtro es opt-in)
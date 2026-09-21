## ADDED Requirements

### Requirement: BanManager::ban reemplaza duplicados (IP+rule_id)

Cuando `ban()` recibe una IP y `rule_id` para los que ya existe un ban activo (no expirado), debe **reemplazar** ese ban existente en lugar de añadir un duplicado. El ban reemplazado actualiza `banned_at`, `ban_duration_seconds`, `escalation_level` y `reason`.

#### Contract: Cambio en BanManager::ban
```rust
// Sin cambios en la firma. Cambia la implementación:
// Antes: self.bans.entry(ip).or_default().push(ban_info);
// Después: buscar ban activo con mismo rule_id y reemplazarlo, o push si no existe.
pub fn ban(
    &mut self,
    ip: IpAddr,
    rule_id: Option<i32>,
    reason: String,
    ban_duration_override: Option<i64>,
) -> &BanInfo
```

#### Scenario: ban reemplaza duplicado activo con mismo rule_id
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=Some(1)` con `escalation_level=0` y `ban_duration_seconds=3600`
- **WHEN** se llama `ban(ip, Some(1), "segunda infracción".to_string(), None)`
- **THEN** el Vec de bans para esa IP sigue teniendo 1 solo elemento
- **AND** el ban existente se ha reemplazado: `escalation_level=1`, `ban_duration_seconds=7200` (escalado 2×)
- **AND** `banned_at` se actualiza al momento actual
- **AND** `reason` se actualiza a "segunda infracción"

#### Scenario: ban con distinto rule_id añade nuevo ban (no reemplaza)
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=Some(1)`
- **WHEN** se llama `ban(ip, Some(2), "otra rule".to_string(), None)`
- **THEN** el Vec de bans para esa IP tiene 2 elementos
- **AND** el ban original para `rule_id=Some(1)` permanece intacto

#### Scenario: ban con rule_id=None siempre añade (no reemplaza)
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=None`
- **WHEN** se llama `ban(ip, None, "otro ban sin rule".to_string(), None)`
- **THEN** el Vec de bans para esa IP tiene 2 elementos
- **AND** ambos bans tienen `rule_id=None`

#### Scenario: ban reemplaza solo si el existente está activo
- **GIVEN** una IP `1.2.3.4` con un ban expirado para `rule_id=Some(1)`
- **WHEN** se llama `ban(ip, Some(1), "nuevo".to_string(), None)`
- **THEN** el Vec de bans para esa IP tiene 2 elementos
- **AND** el nuevo ban se añade (el expirado no se reemplaza)

#### Scenario: persist_ban usa UPSERT para evitar duplicados en DB
- **GIVEN** una IP `1.2.3.4` con `rule_id=Some(1)` ya persistida en la tabla `bans`
- **WHEN** se llama `persist_ban(pool, ip, Some(1), "nuevo", 7200, 1)`
- **THEN** la fila existente se actualiza (no se inserta una nueva)
- **AND** la tabla `bans` tiene 1 sola fila para `ip_address="1.2.3.4"` y `rule_id=1`

#### Scenario: load_from_db con duplicados legacy conserva el más reciente
- **GIVEN** la tabla `bans` tiene 2 filas para `ip_address="1.2.3.4"` y `rule_id=1`: una con `banned_at="2026-09-20T10:00:00Z"` y otra con `banned_at="2026-09-21T10:00:00Z"`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** solo se carga 1 ban para esa IP+rule_id
- **AND** es el de `banned_at="2026-09-21T10:00:00Z"` (el más reciente)
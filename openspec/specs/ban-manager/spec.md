# ban-manager Specification

## Purpose
Define el modelo BanManager, BanResponse, y los endpoints CRUD de baneos.

## Requirements

### Requirement: BanManager::unban con rule_id=None

`BanManager::unban()` debe eliminar todos los bans de una IP cuando `rule_id=None`.

#### Scenario: Unban sin rule_id elimina todos los bans de la IP
- **GIVEN** una IP `1.2.3.4` con dos bans activos: uno con `rule_id=Some(1)` y otro con `rule_id=Some(3)`
- **WHEN** se llama `unban(&ip, None)`
- **THEN** se eliminan AMBOS bans
- **AND** `unban` retorna `true`

#### Scenario: Unban con rule_id solo elimina el ban de esa rule
- **GIVEN** una IP `1.2.3.4` con dos bans activos: uno con `rule_id=Some(1)` y otro con `rule_id=Some(3)`
- **WHEN** se llama `unban(&ip, Some(1))`
- **THEN** solo se elimina el ban con `rule_id=Some(1)`
- **AND** el ban con `rule_id=Some(3)` permanece activo
- **AND** `unban` retorna `true`

#### Scenario: Unban de IP no baneada retorna false
- **GIVEN** una IP `1.2.3.4` sin bans activos
- **WHEN** se llama `unban(&ip, None)`
- **THEN** retorna `false`

### Requirement: BanResponse incluye rule_name

`BanResponse` debe incluir el nombre de la rule asociada a `rule_id`.

#### Contract: BanResponse
```rust
pub struct BanResponse {
    pub id: String,
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub rule_name: Option<String>,  // NUEVO: nombre de la rule
    pub reason: String,
    pub ban_duration_seconds: i64,
    pub escalation_level: u32,
    pub time_remaining_seconds: u64,
}
```

#### Scenario: BanResponse incluye rule_name cuando hay rule_id
- **GIVEN** un ban con `rule_id=Some(3)`
- **AND** existe una rule activa con `id=3` y `name="Path Scanning"`
- **WHEN** se listan los bans (`GET /api/v1/bans`)
- **THEN** el `BanResponse` incluye `rule_name="Path Scanning"`

#### Scenario: BanResponse incluye rule_name=null cuando rule_id es None
- **GIVEN** un ban con `rule_id=None`
- **WHEN** se listan los bans
- **THEN** el `BanResponse` incluye `rule_name=null`

#### Scenario: BanResponse incluye rule_name=null cuando rule_id no existe
- **GIVEN** un ban con `rule_id=Some(999)`
- **AND** no existe ninguna rule con `id=999`
- **WHEN** se listan los bans
- **THEN** el `BanResponse` incluye `rule_name=null`

### Requirement: DELETE /api/v1/bans requiere rule_id

El endpoint DELETE requiere `rule_id` para identificar qué ban específico eliminar. Una IP puede estar baneada por varias rules distintas.

#### Contract: UnbanParams
```rust
pub struct UnbanParams {
    pub id: Option<String>,
    pub ip_address: Option<String>,
    pub rule_id: Option<i32>,  // requerido, validado en handler
}
```

#### Scenario: DELETE sin rule_id retorna 400
- **GIVEN** `DELETE /api/v1/bans?id=1.2.3.4`
- **WHEN** el handler procesa la request
- **THEN** retorna 400 Bad Request con mensaje "rule_id is required"
- **AND** no se modifican los bans

#### Scenario: DELETE con rule_id desbanea esa rule específica
- **GIVEN** una IP `1.2.3.4` baneada por rule_id=1 y rule_id=3
- **WHEN** `DELETE /api/v1/bans?id=1.2.3.4&rule_id=1`
- **THEN** solo se elimina el ban de rule_id=1
- **AND** el ban de rule_id=3 permanece activo
- **AND** response status 200

#### Scenario: DELETE con rule_id inexistente retorna 404
- **GIVEN** una IP `1.2.3.4` sin bans activos, o sin ban para rule_id=999
- **WHEN** `DELETE /api/v1/bans?id=1.2.3.4&rule_id=999`
- **THEN** retorna 404 "IP not found or not banned"

### Requirement: La tabla de baneos muestra rule_name

La UI debe mostrar el nombre de la rule en lugar del ID numérico, con formato tag.

#### Contract: Interfaz Ban (frontend)
```typescript
export default interface Ban {
  id: string;
  ip_address: string;
  rule_id?: number;
  rule_name?: string;   // NUEVO
  reason: string;
  ban_duration_seconds: number;
  escalation_level: number;
  time_remaining_seconds: number;
}
```

#### Scenario: Columna rule_name visible en la tabla
- **GIVEN** la página de baneos
- **WHEN** se renderiza la tabla
- **THEN** se muestra la columna "Rule" con el nombre de la rule
- **AND** no se muestra "Ban IP" button (baneo manual eliminado)

### Requirement: DELETE desde frontend incluye rule_id

El diálogo de borrado debe enviar `rule_id` junto con `id` para desbanear solo esa rule específica.

#### Scenario: DELETE request envía rule_id
- **GIVEN** un ban con `id="1.2.3.4"` y `rule_id=3`
- **WHEN** se confirma el borrado en el diálogo
- **THEN** se envía `DELETE /api/v1/bans?id=1.2.3.4&rule_id=3`
- **AND** response status 200

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

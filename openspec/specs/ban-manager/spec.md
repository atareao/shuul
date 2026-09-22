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

### Requirement: load_from_db restaura escalation_counts

`BanManager::load_from_db()` debe restaurar el mapa `escalation_counts` con el máximo `escalation_level` por IP encontrado en los bans activos cargados de la BD.

#### Contract: Cambio en load_from_db

```rust
// Dentro de load_from_db(), después del loop que carga los bans:
// Para cada IP, calcular el escalation_level máximo y restaurarlo en escalation_counts
for row in &rows {
    let ip: IpAddr = row.get("ip_address").parse()?;
    let escalation_level: i32 = row.get("escalation_level");
    // ... existing BanInfo creation ...
    
    // NUEVO: restaurar escalation_counts con el nivel máximo histórico
    let entry = manager.escalation_counts.entry(ip).or_insert_with(|| (0, Instant::now()));
    if (escalation_level as u32) > entry.0 {
        entry.0 = escalation_level as u32;
    }
}
```

#### Scenario: load_from_db restaura escalation_counts al nivel máximo

- **GIVEN** la tabla `bans` tiene 2 filas activas para `ip_address="1.2.3.4"`: una con `rule_id=1` y `escalation_level=5`, otra con `rule_id=2` y `escalation_level=3`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene una entrada para IP `1.2.3.4` con nivel `5` (el máximo)
- **AND** el próximo `ban(ip, Some(3), ...)` crea un BanInfo con `escalation_level=5` (no 0)

#### Scenario: load_from_db con IP sin bans no crea entradas en escalation_counts

- **GIVEN** la tabla `bans` está vacía
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` está vacío

#### Scenario: load_from_db con IP baneada una sola vez

- **GIVEN** la tabla `bans` tiene 1 fila activa para `ip_address="1.2.3.4"` con `escalation_level=7`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene una entrada para IP `1.2.3.4` con nivel `7`

#### Scenario: load_from_db con múltiples IPs

- **GIVEN** la tabla `bans` tiene filas activas para IPs `1.2.3.4` (level 5) y `5.6.7.8` (level 3)
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene entradas para ambas IPs con sus respectivos niveles máximos

### Requirement: BanManager::is_banned_for_rule

`BanManager::is_banned_for_rule()` debe verificar si un IP tiene un ban activo para una **regla específica**, no para cualquier regla.

#### Contract: is_banned_for_rule

```rust
pub fn is_banned_for_rule(&self, ip: &IpAddr, rule_id: Option<i32>) -> bool {
    self.bans.get(ip).map_or(false, |ban_list| {
        ban_list.iter().any(|b| !b.is_expired() && b.rule_id == rule_id)
    })
}
```

#### Scenario: IP baneada para rule_id específico retorna true
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=Some(1)`
- **WHEN** se llama `is_banned_for_rule(&ip, Some(1))`
- **THEN** retorna `true`

#### Scenario: IP baneada para rule_id diferente retorna false
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=Some(1)`
- **WHEN** se llama `is_banned_for_rule(&ip, Some(2))`
- **THEN** retorna `false`

#### Scenario: IP baneada sin rule_id (None) retorna true para None
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=None`
- **WHEN** se llama `is_banned_for_rule(&ip, None)`
- **THEN** retorna `true`

#### Scenario: IP baneada sin rule_id retorna false para Some
- **GIVEN** una IP `1.2.3.4` con un ban activo para `rule_id=None`
- **WHEN** se llama `is_banned_for_rule(&ip, Some(1))`
- **THEN** retorna `false`

#### Scenario: IP no baneada retorna false
- **GIVEN** una IP `1.2.3.4` sin bans activos
- **WHEN** se llama `is_banned_for_rule(&ip, Some(1))`
- **THEN** retorna `false`

#### Scenario: IP con ban expirado retorna false
- **GIVEN** una IP `1.2.3.4` con un ban expirado para `rule_id=Some(1)`
- **WHEN** se llama `is_banned_for_rule(&ip, Some(1))`
- **THEN** retorna `false`

### Requirement: load_from_db usa multipliers del perfil

`BanManager::load_from_db()` debe leer los multipliers del perfil de rate limit asociado a cada ban, en lugar de usar valores hardcodeados.

#### Contract: Cambio en load_from_db

```rust
// Antes:
let mut manager = Self::new(
    3600,             // default_ban_duration
    true,             // bantime_increment
    vec![1, 2, 4, 8], // hardcodeado
    604_800,          // bantime_maxtime
    30,               // ban_count_decay_days
);

// Después:
// load_from_db hace JOIN con rate_limit_profiles en la query SQL
// Si el perfil no existe (eliminado), usa defaults [1, 2, 4, 8]
```

#### Scenario: load_from_db usa multipliers del perfil Path Scanning
- **GIVEN** la tabla `bans` tiene un ban activo para IP `1.2.3.4` con `rule_id=1`
- **AND** la rule con `id=1` tiene `rate_limit_profile_id=3` (Path Scanning: `bantime_multipliers=[1,2,4,8,16,32]`, `bantime_maxtime=86400`, `ban_count_decay_days=30`)
- **WHEN** se llama `load_from_db(pool)`
- **THEN** el `BanManager` se crea con `bantime_multipliers=[1,2,4,8,16,32]`, `bantime_maxtime=86400`, `ban_count_decay_days=30`

#### Scenario: load_from_db usa defaults cuando el perfil no existe
- **GIVEN** la tabla `bans` tiene un ban activo para IP `1.2.3.4` con `rule_id=999`
- **AND** no existe ninguna rule con `id=999`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** se usan los valores por defecto: `bantime_multipliers=[1,2,4,8]`, `bantime_maxtime=604800`, `ban_count_decay_days=30`

#### Scenario: load_from_db con múltiples perfiles distintos
- **GIVEN** la tabla `bans` tiene bans activos para IPs distintas con perfiles distintos (Path Scanning y Auth Brute Force)
- **WHEN** se llama `load_from_db(pool)`
- **THEN** se usa el perfil con `bantime_maxtime` más alto como referencia global
- **AND** los multipliers se toman del perfil más restrictivo

### Requirement: load_from_db limpia duplicados legacy en DB

Al cargar los bans, `load_from_db()` debe marcar como `expired=1` los duplicados legacy (misma IP+rule_id) que existan en DB, para que la limpieza periódica los elimine.

#### Contract: Limpieza de duplicados en load_from_db

```rust
// Después de cargar los bans, ejecutar:
sqlx::query(
    "UPDATE bans SET expired = 1 \
     WHERE expired = 0 \
       AND rowid NOT IN ( \
         SELECT MIN(rowid) FROM ( \
           SELECT rowid, ROW_NUMBER() OVER ( \
             PARTITION BY ip_address, COALESCE(rule_id, 0) \
             ORDER BY banned_at DESC \
           ) AS rn FROM bans WHERE expired = 0 \
         ) WHERE rn = 1 \
       )"
)
.execute(pool)
.await?;
```

#### Scenario: load_from_db marca duplicados como expired
- **GIVEN** la tabla `bans` tiene 3 filas activas para `ip_address="1.2.3.4"` y `rule_id=1`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** solo 1 ban se carga en memoria (el más reciente)
- **AND** las otras 2 filas se marcan como `expired=1` en DB

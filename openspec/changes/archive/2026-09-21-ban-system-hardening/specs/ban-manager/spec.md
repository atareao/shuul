## ADDED Requirements

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
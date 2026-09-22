## ADDED Requirements

### Requirement: RateLimiter persistente

El estado de los rate limiters (ring buffers por IP) debe persistirse en SQLite para que no se pierda al reiniciar el servidor.

#### Contract: Tabla rate_limiter_state

```sql
CREATE TABLE IF NOT EXISTS rate_limiter_state (
    profile_id INTEGER NOT NULL,
    ip_address TEXT NOT NULL,
    timestamps TEXT NOT NULL,       -- JSON array de epoch millis
    head INTEGER NOT NULL,
    count INTEGER NOT NULL,
    capacity INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (profile_id, ip_address)
);
```

#### Contract: RateLimiter::save

```rust
impl RateLimiter {
    /// Persiste todos los ring buffers de este RateLimiter a SQLite.
    /// Se llama desde un background task periódico (ej. cada 60s).
    pub async fn save(&self, pool: &SqlitePool, profile_id: i32) -> Result<(), sqlx::Error>
}
```

#### Contract: RateLimiter::load

```rust
impl RateLimiter {
    /// Carga el estado persistido de un RateLimiter desde SQLite.
    /// Se llama al arrancar el servidor para cada profile_id activo.
    pub async fn load(pool: &SqlitePool, profile_id: i32, max_retry: u32, find_time_seconds: i32) -> Result<Self, sqlx::Error>
}
```

#### Scenario: RateLimiter se persiste y recupera tras reinicio
- **GIVEN** un RateLimiter para profile_id=3 (Path Scanning: max_retry=20, find_time=60s)
- **AND** tiene registros para IP `1.2.3.4` con 15 timestamps en la ventana de 60s
- **WHEN** se llama `save(pool, 3)`
- **THEN** se inserta/actualiza una fila en `rate_limiter_state` con profile_id=3, ip_address="1.2.3.4", y los timestamps serializados

- **GIVEN** el servidor se reinicia
- **WHEN** se llama `load(pool, 3, 20, 60)`
- **THEN** el RateLimiter se crea con los 15 timestamps de `1.2.3.4` restaurados
- **AND** el IP `1.2.3.4` está a 15/20 requests (no empieza de cero)

#### Scenario: RateLimiter vacío no persiste nada
- **GIVEN** un RateLimiter nuevo sin registros
- **WHEN** se llama `save(pool, 3)`
- **THEN** no se inserta ninguna fila en `rate_limiter_state`

#### Scenario: load con profile_id sin datos retorna RateLimiter vacío
- **GIVEN** la tabla `rate_limiter_state` no tiene filas para profile_id=999
- **WHEN** se llama `load(pool, 999, 20, 60)`
- **THEN** retorna un RateLimiter vacío (HashMap sin entradas)

#### Scenario: Timestamps expirados se filtran al cargar
- **GIVEN** la tabla `rate_limiter_state` tiene timestamps para IP `1.2.3.4` con algunos fuera de la ventana de 60s
- **WHEN** se llama `load(pool, 3, 20, 60)`
- **THEN** solo se cargan los timestamps dentro de la ventana de 60s
- **AND** los timestamps expirados se descartan

### Requirement: Background task de persistencia

Un background task debe persistir los rate limiters periódicamente (cada 60s) para minimizar la pérdida de estado en caso de caída.

#### Contract: Background task en main.rs

```rust
// En main.rs, dentro del tokio::spawn loop:
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        // Persistir todos los rate limiters activos
        let rate_limiters = app_state.rate_limiter.lock().unwrap();
        for (profile_id, rl) in rate_limiters.iter() {
            if let Err(e) = rl.save(&pool, *profile_id).await {
                warn!("Failed to persist rate limiter {profile_id}: {e}");
            }
        }
    }
});
```

#### Scenario: Background task persiste cada 60s
- **GIVEN** el servidor lleva 120s en ejecución
- **WHEN** el background task se ejecuta 2 veces
- **THEN** los rate limiters se persisten 2 veces en la tabla `rate_limiter_state`
- **AND** los datos más recientes sobrescriben los anteriores (UPSERT)

### Requirement: Carga de rate limiters al arrancar

Al iniciar el servidor, se deben cargar los rate limiters persistidos para todos los profile_ids activos (referenciados por reglas Jail activas).

#### Contract: Carga en main.rs

```rust
// Después de cargar las rules:
let rules_guard = rules.lock().unwrap();
let active_profile_ids: Vec<i32> = rules_guard.iter()
    .filter(|r| r.rule.pipeline == "jail" && r.rule.active)
    .filter_map(|r| r.rule.rate_limit_profile_id)
    .collect();
drop(rules_guard);

let mut rate_limiter_map = HashMap::new();
for profile_id in active_profile_ids {
    if let Ok(profile) = RateLimitProfile::read(&pool, profile_id).await {
        let rl = RateLimiter::load(
            &pool,
            profile_id,
            profile.max_retry as u32,
            profile.find_time_seconds,
        ).await.unwrap_or_else(|_| RateLimiter::new(
            profile.max_retry as u32,
            profile.find_time_seconds,
        ));
        rate_limiter_map.insert(profile_id, rl);
    }
}
let rate_limiter = Mutex::new(rate_limiter_map);
```

#### Scenario: Al arrancar se cargan rate limiters para todos los profile_ids activos
- **GIVEN** hay reglas Jail activas que referencian profile_ids 3 y 5
- **AND** la tabla `rate_limiter_state` tiene datos para ambos profile_ids
- **WHEN** el servidor arranca
- **THEN** se cargan los rate limiters para profile_id=3 y profile_id=5
- **AND** el HashMap `rate_limiter` tiene entradas para ambos
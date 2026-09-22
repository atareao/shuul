# Change Proposal: Ban System Hardening

## Why

Los bans actuales muestran niveles de escalada de 162+ pero duraciones capped en 8h, lo que no disuade scanners persistentes. Además, bugs en la comprobación de bans (per-IP en lugar de per-rule) y falta de persistencia de rate limiters causan falsos negativos y pérdida de estado al reiniciar.

## What Changes

1. **BanManager::is_banned_for_rule()** — nuevo método que verifica bans por regla específica
2. **report.rs** — usar is_banned_for_rule en lugar de is_banned()
3. **load_from_db** — leer multipliers del perfil en lugar de hardcodear
4. **load_from_db** — limpiar duplicados legacy en DB
5. **Nueva tabla rate_limiter_state** — persistir ring buffers
6. **RateLimiter::save/load** — serialización/deserialización a SQLite
7. **Background task** — persistir rate limiters cada 60s
8. **Startup** — cargar rate limiters persistidos al arrancar
9. **Multipliers** — [1,2,4,8,16,32,64,128] en templates y seeds

## Intent

Endurecer el sistema de bans de Shuul para que los scanners persistentes (como los de GCP vistos en producción) sean efectivamente disuadidos, y corregir bugs de concurrencia y persistencia que permiten bans duplicados y pérdida de estado al reiniciar.

## Scope

| Módulo | Cambio |
|---|---|
| `backend/src/http/report.rs` | Fix `is_banned()` per-rule en lugar de per-IP |
| `backend/src/models/ban_manager.rs` | Multipliers desde perfil en `load_from_db`, limpieza de duplicados |
| `backend/src/models/rate_limiter.rs` | Persistencia de ring buffers a SQLite |
| `backend/src/templates.rs` | Multipliers más agresivos en templates |
| `backend/migrations/` | Nueva migración para tabla `rate_limiter_state` |
| `backend/src/main.rs` | Carga de rate limiters persistidos al arrancar |

## Problemas Detectados

### 1. `is_banned()` per-IP en lugar de per-rule (BUG)

En `report.rs:298`:
```rust
if ban_manager.is_banned(&ip).is_some() {
    continue;  // Salta TODAS las reglas del reporte
}
```

`is_banned()` busca cualquier ban activo para el IP. Si el IP está baneado por regla A, se salta el ban para regla B. Además, el `continue` sale del `for` loop entero, saltándose todas las reglas restantes.

**Impacto:** Falsos negativos — IPs que deberían ser baneadas por una regla no lo son porque ya están baneadas por otra.

### 2. Multipliers de escalada demasiado débiles

Todos los perfiles usan `[1, 2, 4, 8]`. Con `default_ban_duration=3600`:
- Level 0: 1h
- Level 1: 2h
- Level 2: 4h
- Level 3+: 8h (capped)

Un scanner persistente espera 8h y vuelve. El nivel de escalada sube a 162 sin beneficio real porque la duración nunca pasa de 8h.

**Impacto:** La escalada no disuade scanners persistentes. El `bantime_maxtime` de 7 días nunca se alcanza.

### 3. `load_from_db` hardcodea multipliers

```rust
BanManager::new(3600, true, vec![1, 2, 4, 8], 604_800, 30)
```

Ignora los multipliers configurados en el perfil de rate limit. Si un perfil tiene `[1, 2, 4, 8, 16]`, al reiniciar se pierde.

**Impacto:** La configuración de escalada del perfil no se respeta tras reinicio.

### 4. Rate limiters no persistidos

```rust
let rate_limiter: Mutex<HashMap<i32, RateLimiter>> = Mutex::new(HashMap::new());
```

Los ring buffers por IP se pierden al reiniciar. Un IP a 19/20 requests en la ventana de 60s empieza de cero.

**Impacto:** Ventana de falso negativo tras cada reinicio.

## Soluciones Propuestas

### A. Fix `is_banned()` per-rule

Añadir método `is_banned_for_rule(ip, rule_id)` que verifica si hay un ban activo para el IP **y** la regla específica:

```rust
pub fn is_banned_for_rule(&self, ip: &IpAddr, rule_id: Option<i32>) -> bool {
    self.bans.get(ip).map_or(false, |ban_list| {
        ban_list.iter().any(|b| !b.is_expired() && b.rule_id == rule_id)
    })
}
```

En `report.rs`, cambiar:
```rust
if ban_manager.is_banned(&ip).is_some() {
    continue;
}
```
por:
```rust
if ban_manager.is_banned_for_rule(&ip, Some(*rule_id)) {
    continue;
}
```

### B. Multipliers más agresivos

Actualizar todos los templates y seeds a `[1, 2, 4, 8, 16, 32, 64, 128]` para que la escalada alcance el `bantime_maxtime`.

Para Path Scanning (`default_ban_duration=300`, `max_ban=86400`):
- Level 0: 5min
- Level 1: 10min
- Level 2: 20min
- Level 3: 40min
- Level 4: 1h20min
- Level 5: 2h40min
- Level 6: 5h20min
- Level 7+: 10h40min (capped en 24h por maxtime)

Para Auth Brute Force (`default_ban_duration=900`, `max_ban=604800`):
- Level 7+: 900*128 = 115200s (32h, capped en 7 días)

### C. `load_from_db` usa multipliers del perfil

En lugar de hardcodear, `load_from_db` debe leer el perfil asociado a cada ban y usar sus multipliers. Si el perfil no existe (eliminado), usar defaults.

### D. Persistir rate limiters

Nueva tabla `rate_limiter_state`:
```sql
CREATE TABLE rate_limiter_state (
    profile_id INTEGER NOT NULL,
    ip_address TEXT NOT NULL,
    timestamps TEXT NOT NULL,  -- JSON array de Instant (como epoch millis)
    head INTEGER NOT NULL,
    count INTEGER NOT NULL,
    capacity INTEGER NOT NULL,
    PRIMARY KEY (profile_id, ip_address)
);
```

- `RateLimiter::save(pool)` — persiste todos los ring buffers
- `RateLimiter::load(pool, profile_id)` — carga ring buffers desde DB
- Background task cada 60s para persistir (o al hacer un ban)

## Impacto

- **Backwards compatibility:** La migración es additive (nueva tabla). Los bans existentes en DB se cargan correctamente.
- **Rendimiento:** La persistencia de rate limiters es fire-and-forget cada 60s. Impacto mínimo.
- **Seguridad:** Scanners persistentes serán baneados por días/semanas en lugar de horas.
# Design: Replace duplicate bans for same IP + rule_id

## Context

Actualmente `BanManager::ban()` hace `push()` incondicional al `Vec<BanInfo>` de la IP, incluso si ya existe un ban activo con el mismo `rule_id`. Esto produce duplicados en memoria y en DB. El escalado (`escalation_counts`) funciona por IP, pero los bans duplicados no aportan valor y complican la depuración.

Ver proposal.md para motivación completa.

## Goals / Non-Goals

**Goals:**
- `ban()` reemplaza el ban activo existente para la misma IP+rule_id en lugar de duplicar
- `persist_ban()` usa UPSERT para mantener una fila por IP+rule_id en DB
- `load_from_db()` maneja duplicados legacy (carga solo el más reciente)
- Migración SQL que añade UNIQUE(ip_address, rule_id) y limpia duplicados

**Non-Goals:**
- No se cambia la semántica de `unban()` (sigue eliminando por IP+rule_id)
- No se cambia el sistema de escalado (sigue siendo por IP)
- No se toca el frontend ni la API pública (los endpoints siguen igual)

## Decisions

### Decisión 1: Reemplazo in-place en el Vec, no push condicional

**Opción elegida:** Buscar el índice del ban activo con mismo `rule_id` en el Vec y reemplazarlo con `swap()` + `pop()` o modificación directa.

**Alternativa considerada:** `retain()` + push. Descartada porque requiere reconstruir el Vec entero.

**Alternativa considerada:** Usar `HashMap<(IpAddr, Option<i32>), BanInfo>` en lugar de `HashMap<IpAddr, Vec<BanInfo>>`. Descartada porque rompe la relación 1:N (una IP puede estar baneada por varias rules distintas) y requeriría refactor mayor.

**Implementación:**
```rust
pub fn ban(&mut self, ip: IpAddr, rule_id: Option<i32>, reason: String, ban_duration_override: Option<i64>) -> &BanInfo {
    let escalation_level = self.get_escalation_level(&ip);
    let duration = ban_duration_override.unwrap_or_else(|| self.calculate_ban_duration(escalation_level));

    let ban_info = BanInfo {
        banned_at: Instant::now(),
        ban_duration_seconds: duration,
        escalation_level,
        rule_id,
        reason,
    };

    let bans = self.bans.entry(ip).or_default();

    // Replace existing active ban with same rule_id, or push new one
    if let Some(rule_id) = rule_id {
        if let Some(pos) = bans.iter().position(|b| b.rule_id == Some(rule_id) && !b.is_expired()) {
            bans[pos] = ban_info;
        } else {
            bans.push(ban_info);
        }
    } else {
        bans.push(ban_info);
    }

    self.increment_escalation(&ip);
    bans.last().unwrap()
}
```

### Decisión 2: UPSERT con INSERT OR REPLACE en SQLite

**Opción elegida:** Usar `INSERT OR REPLACE` con un índice UNIQUE en `(ip_address, rule_id)`.

**Justificación:** SQLite soporta `INSERT OR REPLACE` nativamente. Es más simple que un `SELECT` + `UPDATE` + `INSERT` en dos pasos.

**Nota:** `INSERT OR REPLACE` requiere que `rule_id` sea NOT NULL para el UNIQUE, pero actualmente es NULLable. Para `rule_id=NULL`, se sigue usando INSERT normal (sin UPSERT), ya que no hay restricción UNIQUE que aplicar.

**Implementación:**
```rust
// Para rule_id = Some:
sqlx::query(
    "INSERT OR REPLACE INTO bans (ip_address, rule_id, reason, banned_at, ban_duration_seconds, escalation_level, expired, created_at) \
     VALUES (?, ?, ?, ?, ?, ?, 0, ?) \
     WHERE EXISTS (SELECT 1 FROM bans WHERE ip_address = ? AND rule_id = ? AND expired = 0)"
)
// Para rule_id = None: INSERT normal (sin restricción UNIQUE)
```

**Corrección:** En SQLite, `INSERT OR REPLACE` no lleva WHERE. Mejor usar `INSERT OR REPLACE` directamente, que requiere UNIQUE constraint. Alternativa: `UPDATE` + `INSERT` condicional.

**Decisión final:** Usar `UPDATE` primero + `INSERT` si no afectó filas (más seguro y no requiere UNIQUE en la tabla):

```rust
let affected = sqlx::query(
    "UPDATE bans SET reason = ?, banned_at = ?, ban_duration_seconds = ?, escalation_level = ?, expired = 0 \
     WHERE ip_address = ? AND rule_id = ? AND expired = 0"
)
.bind(...)
.execute(pool)
.await?
.rows_affected();

if affected == 0 {
    // No existing active ban, insert new row
    sqlx::query("INSERT INTO bans (...) VALUES (...)")
        .bind(...)
        .execute(pool)
        .await?;
}
```

### Decisión 3: Migración para limpiar duplicados legacy

Se añade una migración que:
1. Elimina duplicados: para cada par (ip_address, rule_id) con múltiples filas no expiradas, marca `expired=1` en todas excepto la más reciente.
2. Añade índice UNIQUE(ip_address, rule_id) para prevenir futuros duplicados.

**Riesgo:** Si hay duplicados con `rule_id=NULL`, el UNIQUE no los cubre (NULL != NULL en SQL). Se acepta como trade-off: los bans sin rule_id son poco comunes (solo bans manuales sin rule).

## Risks / Trade-offs

- **[Riesgo] Reemplazo en memoria vs DB out-of-sync**: Si `ban()` reemplaza en memoria pero `persist_ban()` falla, el estado en memoria queda inconsistente con DB. → **Mitigación**: Ya existe este riesgo hoy con el push. No empeora.
- **[Riesgo] rule_id=NULL no tiene UNIQUE**: Pueden seguir creándose duplicados para bans sin rule_id. → **Mitigación**: Los bans manuales sin rule_id son raros. Si ocurre, `unban(None)` los limpia todos.
- **[Riesgo] Cambio sutil en semántica**: Código que dependía de que `ban()` siempre añadiera un nuevo ban (ej. para trackear histórico) se rompería. → **Mitigación**: No hay tal código. El sistema siempre ha tratado los bans como estado actual, no como histórico.

## Migration Plan

1. Añadir migración SQL (`20260921000000_add_unique_ip_rule_id.up.sql`)
2. Modificar `BanManager::ban()` con lógica de reemplazo
3. Modificar `persist_ban()` con UPDATE+INSERT
4. Modificar `load_from_db()` para cargar solo el más reciente por IP+rule_id
5. Ejecutar tests existentes para verificar que no se rompen
6. Ejecutar nuevos tests del spec delta
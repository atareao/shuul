## MODIFIED Requirements

### Requirement: load_from_db limpia duplicados legacy en DB

Al cargar los bans, `load_from_db()` SHALL marcar como `expired = 1` **únicamente**
los duplicados legacy (misma IP+rule_id) que existan en DB, para que la limpieza
periódica los elimine.

La unidad de duplicidad SHALL ser la partición
`(ip_address, COALESCE(rule_id, 0))`. Por cada partición SHALL sobrevivir
**exactamente una** fila activa —la de `banned_at` más reciente— y las filas de
particiones distintas NO SHALL tocarse. Un `rule_id` nulo y un `rule_id` numérico
SHALL ser particiones distintas, según la semántica de `COALESCE(rule_id, 0)`.

La limpieza SHALL evaluar `ROW_NUMBER()` por partición y marcar sólo las filas con
`rn > 1`. NO SHALL aplicarse ningún agregado global —`MIN(rowid)`, `MAX(rowid)` ni
equivalente— sobre el conjunto de todas las particiones, porque colapsaría la
tabla entera a **una sola** fila activa y daría por caducados baneos que no son
duplicados de nada, de modo que un baneo no sobreviviría a un reinicio.

#### Contract: Limpieza de duplicados en load_from_db

```sql
-- Después de cargar los bans, ejecutar:
UPDATE bans SET expired = 1
WHERE expired = 0
  AND rowid IN (
    SELECT rowid FROM (
      SELECT rowid, ROW_NUMBER() OVER (
        PARTITION BY ip_address, COALESCE(rule_id, 0)
        ORDER BY banned_at DESC
      ) AS rn FROM bans WHERE expired = 0
    ) WHERE rn > 1
  )
```

El subconjunto de `rowid` que se marca es el de las filas `rn > 1` de cada
partición: los ganadores (`rn = 1`) NO se tocan y las particiones no se mezclan
entre sí.

#### Scenario: load_from_db marca duplicados como expired

- **GIVEN** la tabla `bans` tiene 3 filas activas para `ip_address="1.2.3.4"` y `rule_id=1`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** solo 1 ban se carga en memoria (el más reciente)
- **AND** las otras 2 filas se marcan como `expired=1` en DB

#### Scenario: Dos IPs distintas conservan su baneo activo tras load_from_db

- **GIVEN** la tabla `bans` tiene una fila activa para `ip_address="1.2.3.4"` con `rule_id=1` y otra fila activa para `ip_address="5.6.7.8"` con `rule_id=2`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** **ambas** filas siguen con `expired=0` en DB
- **AND** `SELECT COUNT(*) FROM bans WHERE expired = 0` es 2
- **AND** las dos IPs se cargan en memoria

#### Scenario: Una IP con dos rule_id distintos conserva ambos baneos activos

- **GIVEN** la tabla `bans` tiene una fila activa para `ip_address="1.2.3.4"` con `rule_id=1` y otra fila activa para la misma IP con `rule_id=3`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** **ambas** filas siguen con `expired=0` en DB
- **AND** las dos son particiones distintas (`(1.2.3.4, 1)` y `(1.2.3.4, 3)`) y ninguna se marca como duplicado

#### Scenario: Mezcla de baneos únicos y duplicados

- **GIVEN** la tabla `bans` tiene N filas activas que son cada una la única de su partición, más M filas activas que son duplicados legacy de una de esas particiones
- **WHEN** se llama `load_from_db(pool)`
- **THEN** sobreviven exactamente **N + 1** filas activas en DB (una por partición con duplicados y una por partición única)
- **AND** las M filas duplicadas se marcan como `expired=1`
- **AND** ninguna de las N filas únicas se marca como `expired=1`

#### Scenario: rule_id NULL y rule_id 1 son particiones distintas

- **GIVEN** la tabla `bans` tiene una fila activa para `ip_address="1.2.3.4"` con `rule_id=NULL` y otra fila activa para la misma IP con `rule_id=1`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** **ambas** filas siguen con `expired=0` en DB
- **AND** la semántica de `COALESCE(rule_id, 0)` se preserva: `NULL` se agrupa como `0` y `1` como `1`, de modo que son particiones distintas

## ADDED Requirements

### Requirement: Limpieza periódica de baneos caducados en DB

> **Marcador de la propuesta — decisión pendiente de aprobación.** Este requisito
> se implementa sólo si el usuario aprueba la opción B2 de `design.md` §D3; si la
> rechaza, se retira este requirement del delta antes del archivado. Conectar la
> limpieza cambia la semántica de la restauración del escalado: sólo los baneos
> vivos en el momento de la carga restaurarían su nivel. Este párrafo se elimina
> al aprobar la opción.

El task de fondo de limpieza —el que corre cada 60 s en `backend/src/main.rs`—
SHALL invocar `BanManager::cleanup_expired_db(pool)` además de
`BanManager::cleanup_expired()` (limpieza en memoria), de modo que las filas cuyo
`banned_at + ban_duration_seconds` ya pasó SHALL marcarse como `expired = 1` en DB
en lugar de quedarse activas indefinidamente.

`cleanup_expired_db` SHALL dejar de estar marcada `#[allow(dead_code)]` y NO SHALL
marcar ninguna fila que todavía esté viva (`banned_at + ban_duration_seconds >
now`). La limpieza NO SHALL afectar a los baneos servidos por la API, que se
resuelven desde memoria.

#### Contract: cleanup_expired_db en el task periódico

```rust
// En el task de 60 s de backend/src/main.rs, junto a ban_manager.cleanup_expired():
if let Err(e) = BanManager::cleanup_expired_db(&cleanup_state.pool).await {
    warn!("DB ban cleanup failed: {e}");
}
```

```sql
-- En BanManager::cleanup_expired_db (ya existente, hoy código muerto):
UPDATE bans SET expired = 1
WHERE expired = 0
  AND CAST(strftime('%s', banned_at) AS INTEGER) + ban_duration_seconds
      < CAST(strftime('%s', 'now') AS INTEGER)
```

#### Scenario: El task periódico marca como expired las filas temporalmente muertas

- **GIVEN** la tabla `bans` tiene una fila con `expired=0` cuyo `banned_at + ban_duration_seconds` es anterior al instante actual
- **WHEN** el task de limpieza de 60 s ejecuta `cleanup_expired_db(pool)`
- **THEN** esa fila pasa a tener `expired=1` en DB

#### Scenario: Las filas todavía vivas no se marcan

- **GIVEN** la tabla `bans` tiene una fila con `expired=0` cuyo `banned_at + ban_duration_seconds` es posterior al instante actual
- **WHEN** el task de limpieza de 60 s ejecuta `cleanup_expired_db(pool)`
- **THEN** esa fila sigue con `expired=0` en DB

#### Scenario: La restauración de escalation_counts sólo considera los baneos vivos en la carga

- **GIVEN** la tabla `bans` tiene un ban caducado con `escalation_level=5` ya marcado `expired=1` por el task periódico, y un ban vivo para la misma IP con `escalation_level=1`
- **WHEN** se llama `load_from_db(pool)` tras el reinicio
- **THEN** `manager.escalation_counts` contiene esa IP con nivel `1` (no `5`)
- **AND** el próximo `ban(ip, Some(n), ...)` crea un `BanInfo` con `escalation_level=1`

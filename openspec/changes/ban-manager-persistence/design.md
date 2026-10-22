# Design

## Context

Ver `proposal.md` — Why y Evidencia medida. El estado actual relevante, con su
ubicación:

- `BanManager::load_from_db` (`backend/src/models/ban_manager.rs:300-438`) carga
  los bans activos (`expired = 0`) en memoria, restaura `escalation_counts` con
  el máximo `escalation_level` por IP y, **después** de la carga, ejecuta el
  `UPDATE` que marca duplicados (líneas **419-435**).
- El `UPDATE` roto:

  ```sql
  UPDATE bans SET expired = 1
  WHERE expired = 0
    AND rowid NOT IN (
      SELECT MIN(rowid) FROM (
        SELECT rowid, ROW_NUMBER() OVER (
          PARTITION BY ip_address, COALESCE(rule_id, 0)
          ORDER BY banned_at DESC
        ) AS rn FROM bans WHERE expired = 0
      ) WHERE rn = 1
    )
  ```

  El `PARTITION BY` interno identifica un ganador (`rn = 1`) por partición, pero
  el `MIN(rowid)` **de fuera** se aplica al conjunto entero: sobrevive **una sola
  fila de toda la tabla** —la de `rowid` más bajo— y todo lo demás se marca
  `expired = 1`.
- `BanManager::cleanup_expired_db` (`ban_manager.rs:554-570`) está marcado
  `#[allow(dead_code)]` y **nunca se llama** (grep en todo `backend/src/`: sólo
  aparece su definición y un doc-comment).
- El task de fondo de 60 s (`backend/src/main.rs:292-313`) llama a
  `ban_manager.cleanup_expired()` (limpieza **en memoria**) y a
  `rl.cleanup_expired()` (rate limiter), pero **no** al de la DB.
- `list_handler` (`backend/src/http/ban.rs:77-96`) sirve desde memoria con
  `ban_manager.active_bans()`, no desde la DB: **B2 no tiene impacto en la UI**.
- Esquema: `bans.id INTEGER PRIMARY KEY AUTOINCREMENT`
  (`backend/migrations/20260902000000_initial_schema.up.sql:58-68`), así que
  `rowid` es alias de `id`.
- Specs: `openspec/specs/ban-manager/spec.md:316-344` —el requirement
  `load_from_db limpia duplicados legacy en DB` **consagra el SQL roto** en su
  `#### Contract`.

## Goals / Non-Goals

**Goals:**

- Que un baneo **sobreviva a un reinicio**: la limpieza de duplicados no puede
  marcar como caducado un ban que no es duplicado de nada.
- Que la semántica quede fijada por partición
  `(ip_address, COALESCE(rule_id, 0))`: una fila activa por partición, las demás
  intactas.
- Que los tests **asertan el estado de la DB**, que es el agujero que dejó pasar
  el bug.
- Corregir el `#### Contract` de la spec que hoy documenta el SQL equivocado.

**Non-Goals:**

- Recuperar los baneos ya barridos en producción. Ver §D4.
- Tocar la API HTTP, el frontend, el pipeline WAF/Jail o el esquema SQLite.
- Cambiar el intervalo del task de limpieza (60 s) o crear uno nuevo.

## Decisions

### D1. Quitar el agregado global del `UPDATE`

La limpieza marca sólo las filas **que son duplicados de verdad**:

```sql
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

La semántica es la que fija el delta de spec: **por cada partición
`(ip_address, COALESCE(rule_id, 0))` sobrevive exactamente una fila activa —la de
`banned_at` más reciente— y las filas de particiones distintas NO se tocan.**
Ningún agregado global —`MIN(rowid)`, `MAX(rowid)` ni equivalente— puede volver a
aparecer: reduciría la tabla entera a una sola fila activa.

**Rationale.** El `ROW_NUMBER()` por partición ya contiene toda la información
necesaria: la fila `rn = 1` es el ganador de su partición y las `rn > 1` son sus
duplicados. El `MIN(rowid)` era un intento de colapsar «los ganadores» a un
conjunto de `rowid` que sobreviven, pero `MIN` es un agregado **global**: colapsa
los ganadores de **todas** las particiones a uno solo. La forma `rowid IN (… WHERE
rn > 1)` elimina el agregado por completo: cada `rowid` del subconjunto se evalúa
contra sí mismo y las particiones no se mezclan.

**Alternativa descartada: `MIN(rowid)` por partición con `GROUP BY`.** Equivale a
lo mismo —`WHERE rowid NOT IN (SELECT MIN(rowid) FROM (…) GROUP BY ip_address,
COALESCE(rule_id, 0))`— pero es más frágil de leer: reintroduce un agregado y un
`GROUP BY` cuya corrección depende de que la clave de agrupación coincida
**exactamente** con la del `PARTITION BY`; si divergen, vuelve el bug. Marcar
`rn > 1` expresa la intención de forma directa y sin clave que duplicar.

**Alternativa descartada: hacer la limpieza en Rust.** Cargar todas las filas
activas, agruparlas en un `HashMap` y emitir un `UPDATE` por duplicado. Descartada
por N+1: con 1410 filas y 31 particiones activas, convierte un `UPDATE` en un
número de round-trips proporcional a los duplicados, y añade a Rust una lógica que
SQLite ya resuelve en una sola sentencia. Además, la limpieza en memoria **ya
existe** (`cleanup_expired()`), así que el trabajo de la DB es sólo el marcado
masivo.

### D2. Los tests asertan el estado de la DB

Los tests nuevos consultan **las filas de la tabla `bans`** —`SELECT COUNT(*) …
WHERE expired = 0` / `WHERE expired = 1`, y el `reason`/`banned_at` de la fila
superviviente—, no sólo el estado en memoria. Además hay que **corregir** el test
engañosamente llamado `test_load_from_db_does_not_affect_unique_bans`: hoy inserta
**una sola fila en toda la tabla**, con lo que `MIN(rowid)` no marca nada y el
test pasa sin cubrir lo que su nombre promete. Debe insertar **varias particiones
distintas** y comprobar que **ninguna** se marca como expirada.

**Rationale.** El bug no era visible en memoria: `load_from_db` carga los bans
**antes** de correr el `UPDATE`, así que el `BanManager` en memoria queda correcto
y sólo la DB se corrompe. Un test que sólo mira `manager.bans` o
`manager.escalation_counts` pasa en verde con la DB destruida —es exactamente lo
que hace hoy `test_load_from_db_multiple_ips`—. El punto de observación tiene que
ser la DB.

### D3. B2 entra en este change (decidido por el usuario el 2026-10-22)

**B2 está aprobado: la conexión de `cleanup_expired_db` al task de 60 s entra en
este change.** Decisión explícita del usuario (2026-10-22); ya no es una opción
abierta.

El problema es real y medido: `cleanup_expired_db` es código muerto y **22 filas**
con `expired = 0` ya están caducadas (la más antigua, el `rowid = 1` del 22 de
septiembre). La tabla crece sin límite: 1410 filas, de las que 1379 ya están
marcadas. **No hay impacto en la UI** (`list_handler` sirve desde memoria y
`cleanup_expired()` descarta las caducadas cada 60 s).

La consecuencia que se asume con esta decisión es de **semántica del escalado**:
hoy las filas caducadas pero no marcadas **restauran `escalation_counts` al
arrancar** (`load_from_db` restaura el nivel máximo histórico por IP). Con el task
de 60 s marcándolas `expired = 1`, sólo los baneos **vivos en el momento de la
carga** restaurarán su nivel. Es un cambio observable y **asumido**: un IP con
historial de bans ya caducados vuelve a empezar en nivel 0 tras el reinicio.

**Rationale de la decisión.** El bug y el código muerto son la misma historia: el
`UPDATE` de `load_from_db` existe precisamente «para que la limpieza periódica los
elimine» (doc-comment), y esa limpieza periódica **nunca se conectó**. Arreglar B1
sin B2 dejaría la tabla creciendo sin límite y la promesa del contrato a medias. El
cambio de semántica del escalado es **el comportamiento correcto**: el escalado
debe decaer con los baneos, no perpetuarse por filas que ya no están vivas.
Aprovecha que ya se toca el fichero y el mismo requisito de persistencia. El
scenario correspondiente del delta —la restauración de `escalation_counts` sólo
considera los baneos vivos en la carga— es **normativo**.

**Alternativas descartadas.**

- **B2 fuera, en un change propio.** Mantendría este change mínimo y con un solo
  efecto (el `UPDATE`), lo que hace la regresión más fácil de atribuir. Descartada:
  deja el código muerto en pie y la tabla creciendo, y obliga a un segundo
  archivado para algo que comparte causa raíz con B1.
- **B2 nunca.** Descartada: el crecimiento de la tabla es ilimitado y el
  `#[allow(dead_code)]` señala una intención incumplida desde su creación.

### D4. Alcance: sin frontend, sin migración, y sin rescate de lo barrido

El change es **sólo backend**. No hay cambio de esquema ni migración: el `UPDATE`
es lógica, y la columna `expired` ya existe. El frontend no cambia: `list_handler`
sirve desde memoria.

**Los baneos ya barridos en `co1` no se recuperan: decisión explícita del usuario
(2026-10-22), no un supuesto.** El `UPDATE` los marcó `expired = 1` y no hay forma
fiable de distinguirlos de los caducados de verdad —el marcado no guarda la
causa—. La única vía de rescate sería recalcular `banned_at +
ban_duration_seconds` sobre las filas `expired = 1`, que hoy daría **7 filas**
todavía válidas; pero ese cálculo no distingue una fila barrida por el bug de una
caducada de verdad que el task ya hubiera tocado, ni de una **desbaneada a mano**
—resucitaría un baneo que un operador levantó—. Por eso **no se prepara ningún
script de rescate**. Este change arregla el futuro, no el pasado.

## Risks / Trade-offs

- **Cambio de comportamiento de la limpieza**: hoy se marca casi todo; después se
  marcará sólo lo que es duplicado. Es el arreglo, pero es observable en la DB:
  el recuento de filas activas sube y deja de colapsar a una sola.
- **La restauración del escalado cambia** (§D3, decidido): sólo los baneos vivos
  al cargar restauran su nivel. Es intencionado —el escalado decae con los baneos—
  y es el precio asumido de conectar la limpieza periódica.
- **Los baneos ya perdidos en producción no se recuperan** (§D4, decidido por el
  usuario): no se prepara script de rescate. Este change arregla el futuro, no el
  pasado.
- **`MIN(rowid)` podría reaparecer** en un futuro refactor. Mitigación: el
  `#### Contract` del delta lo prohíbe por escrito y el enunciado fija la
  semántica por partición; el test de «varias particiones intactas» (D2) lo
  detectaría.

## Migration Plan

No hay migración de datos ni de esquema.

1. Desplegar la imagen nueva. En el siguiente arranque, `load_from_db` marcará
   como expiradas **sólo** las filas duplicadas de verdad; los baneos únicos
   conservan `expired = 0` y sobreviven al reinicio.
2. Verificar en la DB de producción que el recuento de filas activas ya no colapsa
   a 1: `SELECT COUNT(*) FROM bans WHERE expired = 0` debe ser ≥ el número de
   particiones activas.
3. El task de 60 s marca las filas caducadas: comprobar que
   `expired = 0 AND banned_at + ban_duration_seconds < now` devuelve 0 filas.
4. Rollback: revertir la imagen. No hay estado que deshacer —el único efecto es
   sobre la columna `expired`, y el comportamiento anterior no requiere
   restaurarla.

## Open Questions

Ninguna abierta. Las dos preguntas que planteaba este change quedaron **resueltas
por el usuario el 2026-10-22**, y se dejan aquí como memoria de la decisión:

- **¿Entra B2 en este change?** **Sí, entra.** Resuelto en §D3: la conexión de
  `cleanup_expired_db` al task de 60 s es parte del alcance aprobado, con la
  consecuencia asumida sobre la restauración del escalado.
- **¿Merece la pena un script de rescate de los baneos ya barridos en `co1`?**
  **No.** Decisión explícita del usuario: no se rescata y no se prepara script. El
  cálculo `banned_at + ban_duration_seconds` daría **7 filas** todavía válidas,
  pero no distingue las barridas por el bug de las caducadas de verdad ni de las
  desbaneadas a mano —resucitaría un baneo levantado por un operador—. Ver §D4.

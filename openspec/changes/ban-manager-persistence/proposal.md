# Proposal: ban-manager-persistence

## Why

`BanManager::load_from_db` ejecuta, tras cargar los baneos en memoria, un
`UPDATE` que debería marcar como `expired = 1` **sólo** los duplicados legacy. Su
`PARTITION BY ip_address, COALESCE(rule_id, 0)` es correcto, pero el `MIN(rowid)`
exterior se aplica al **conjunto entero** en vez de a cada partición: sobrevive
**una sola fila de toda la tabla** —la de `rowid` más bajo— y **todo lo demás se
marca expirado**, incluidos baneos que no son duplicados de nada. Como el
`UPDATE` corre después de la carga en memoria, el baneo entra al arrancar pero la
DB lo da por caducado para el arranque siguiente: **un baneo no sobrevive a un
reinicio**. Es un fallo de seguridad de disponibilidad del baneo, no cosmético:
una IP baneada por el Jail —o a mano— vuelve a estar libre tras un `restart`. Y la
spec **consagra el SQL roto**: su `#### Contract` incluye ese mismo `MIN(rowid)`.
No está incompleta: está equivocada.

## What Changes

- **A. Semántica de la limpieza de duplicados.** El `UPDATE` de
  `load_from_db` deja de usar un agregado global sobre todas las particiones y
  pasa a marcar **sólo** las filas con `rn > 1` dentro de su partición
  `(ip_address, COALESCE(rule_id, 0))`. La semántica que fija el delta es:
  **por cada partición sobrevive exactamente una fila activa —la de `banned_at`
  más reciente— y las filas de particiones distintas NO se tocan.** Es un
  **MODIFIED** del requirement `load_from_db limpia duplicados legacy en DB`,
  incluido su `#### Contract`, que hoy consagra el SQL roto.

- **B. Limpieza periódica de la DB.** `BanManager::cleanup_expired_db(pool)`
  existe, está marcado `#[allow(dead_code)]` y **nunca se llama**. El task de
  fondo de 60 s limpia memoria (`cleanup_expired()`) y rate limiters, pero no la
  tabla: las filas temporalmente muertas se quedan con `expired = 0` para siempre.
  Marcarlas **no** acota el tamaño de la tabla —nada borra filas de `bans`—, pero
  evita que se carguen al arrancar y **resuciten el nivel de escalado** como si
  siguieran vivas. Conectar la llamada tiene una **consecuencia semántica
  asumida** (aprobada el 2026-10-22): sólo los baneos vivos en el
  momento de la carga restauran su nivel de escalado entre reinicios. El
  requirement nuevo (`cleanup_expired_db` en el task periódico) es normativo.

- **C. Tests que asertan la DB.** Los tests nuevos consultan **las filas de la
  tabla `bans`** (`expired`), no sólo el estado en memoria. El agujero que dejó
  pasar el bug es exactamente ése: los tests actuales asertan memoria y, en el
  caso de dos IPs, nunca miran la DB.

- **Documentación.** El contrato de `load_from_db` y de `cleanup_expired_db`
  queda documentado en su doc-comment.

No hay **BREAKING changes** de API, ni de esquema de datos, ni de variables de
entorno.

## Capabilities

### New Capabilities

Ninguna. El change no introduce ninguna capacidad nueva: modifica y amplía el
contrato de una ya existente (`ban-manager`).

### Modified Capabilities

- `ban-manager` — **dos** requisitos entran en el delta:
  - *load_from_db limpia duplicados legacy en DB* (**MODIFIED**): su
    `#### Contract` sustituye el `MIN(rowid)` global por la marcación de
    `rn > 1` por partición, y su semántica queda fijada en el enunciado.
  - *Limpieza periódica de baneos caducados en DB* (**ADDED**): el task de fondo
    de 60 s invoca `cleanup_expired_db(pool)`.

## Impact

- `backend/src/models/ban_manager.rs`:
  - el `UPDATE` de `load_from_db` (líneas **419-435**): se elimina el agregado
    global y se marca sólo `rn > 1` por partición. Es el **único** punto donde se
    decide qué es un duplicado.
  - `cleanup_expired_db` (líneas **554-570**): pasa de código muerto
    (`#[allow(dead_code)]`, sin llamadas en todo `backend/src/`) a invocarse desde
    el task periódico.
  - el módulo `tests` (≈1446-1560): los tests de duplicados asertan el estado de
    la DB.
- `backend/src/main.rs` (líneas **292-313**): el task de limpieza de 60 s añade
  la llamada a `cleanup_expired_db(&pool)`.
- `openspec/specs/ban-manager/spec.md`: el contrato que este change corrige.
- **Beneficio, no regresión.** Un baneo deja de desaparecer tras un reinicio; el
  WAF y el Jail no cambian su comportamiento en caliente, y la UI no cambia
  porque sirve desde memoria (`ban_manager.active_bans()`).
- **NO se toca**: el pipeline WAF, el pipeline Jail, `run_jail_pipeline`, el
  esquema SQLite, las migraciones, el frontend ni la API HTTP.

## Evidencia medida

Medido en producción (servidor `co1`, SQLite en modo `-readonly`, contenedor
arrancado el `2026-10-21T18:04:18Z`):

- `rowid = 1` → `ip 34.64.198.185`, `rule_id 6`, `banned_at 2026-09-22T05:03:25`,
  `expired = 0`. Es el **rowid más bajo de toda la tabla** y la **única**
  superviviente del barrido. No es coincidencia: es exactamente lo que hace
  `MIN(rowid)` global.
- **7 filas marcadas `expired = 1` que todavía son válidas** (`banned_at +
  ban_duration_seconds > now`), todas **anteriores al arranque del contenedor**:
  `34.84.248.173` reglas 6 y 9 (15:50, duración 86400), `185.55.7.213` reglas 9 y
  6 (12:35), `34.39.50.88` regla 6 (08:09). Firma inequívoca: el `UPDATE` corrió
  al arrancar y las barrió.
- Estado de la tabla: **1410 filas**, **31** con `expired = 0`, **31**
  particiones distintas (`ip_address, COALESCE(rule_id, 0)`). Los 31 activos son
  posteriores al arranque; el único anterior es el `rowid = 1`.
- El esquema es `id INTEGER PRIMARY KEY AUTOINCREMENT`
  (`backend/migrations/20260902000000_initial_schema.up.sql:58-68`), así que
  `rowid` es alias de `id`: el superviviente es el baneo **más antiguo**.
- **B2, medido:** **22 filas** con `expired = 0` pero ya caducadas (la más antigua
  es el `rowid = 1` del 22 de septiembre) y **1379** filas ya marcadas
  `expired = 1` de 1410 totales.

**Por qué nadie lo detectó.** La spec **consagra el SQL roto** (`#### Contract`
con ese mismo `MIN(rowid)`) y los tests no cubren el caso que rompe:

- `test_load_from_db_does_not_affect_unique_bans` (≈1514) inserta **una sola fila
  en toda la tabla**. Con una fila, `MIN(rowid)` es esa fila y no marca nada:
  pasa en verde sin cubrir nada. El nombre promete más de lo que prueba.
- `test_load_from_db_multiple_ips` (≈1565) sí inserta **dos IPs** —que con el bug
  deberían corromper la DB—, pero **sólo aserta `escalation_counts` en memoria**;
  nunca consulta las filas de la DB. Pasa en verde con la DB corrupta.
- `test_load_from_db_marks_duplicates_expired` (≈1447) usa una sola partición, el
  único caso que el bug respeta.
- Los 28 tests de `ban_manager::` pasan en verde
  (`cargo test --lib ban_manager::` → 28 passed; 0 failed).

## Non-goals

- **No se recuperan los baneos ya barridos en `co1`** — **decisión explícita del
  usuario (2026-10-22), no una duda pendiente**. El `UPDATE` los marcó y no hay
  forma fiable de distinguirlos de los caducados de verdad, salvo por el cálculo
  `banned_at + ban_duration_seconds` —que hoy da 7 filas válidas—, que además
  resucitaría baneos levantados a mano. **No se prepara ningún script de rescate**
  (§D4).
- **No se cambia** la API HTTP, ni el `list_handler`, ni el frontend: la UI sirve
  desde memoria y muestra exactamente lo mismo.
- **No se añade** ninguna migración: el esquema no cambia.
- **No se cambia** la semántica de `escalation_counts` más allá de la consecuencia
  inevitable de B (§D3): la restauración sigue tomando el máximo `escalation_level`
  de los bans activos que se cargan.
- **No se cambia** el intervalo del task de limpieza (60 s) ni se crea uno nuevo.

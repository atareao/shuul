# Tasks: ban-manager-persistence

> **Alcance:** el `UPDATE` de limpieza de duplicados de `load_from_db` y la
> conexión de `cleanup_expired_db` al task de 60 s (§D3, **aprobado el
> 2026-10-22**). **No** se toca la API HTTP, el frontend, el pipeline WAF/Jail ni
> el esquema SQLite.
> **Baseline:** `cargo test` en verde con **337 passed; 0 failed; 1 ignored**,
> medido en el árbol limpio sobre `develop`. Exigir que no baje.
> **Regla de los tests nuevos:** deben **asertar el estado de la DB**
> (`SELECT COUNT(*) … WHERE expired = 0` / `= 1`, `reason`/`banned_at` de la fila
> superviviente), no sólo el estado en memoria. El bug no era visible en memoria:
> `load_from_db` carga los bans **antes** de correr el `UPDATE`.

## 1. Preparación

- [x] 1.1 Anotar el baseline: `cargo test` en verde y su recuento, en el árbol limpio sobre `develop`. Verificación: el recuento queda escrito aquí y los tests pasan (exit 0). **Hecho:** baseline = **337 passed; 0 failed; 1 ignored** (doc-test), desglose: `lib` 100, `bin` 230, `tests/integration.rs` 0, `tests/rate_limiting_test.rs` 4, `tests/report_per_rule_ban_test.rs` 3. Medido en el árbol limpio sobre `develop`; coincide con el baseline que dejó el change anterior (`missing-header-fields`).
- [x] 1.2 Confirmar que el header del delta coincide **literalmente** con el de la spec destino: `### Requirement: load_from_db limpia duplicados legacy en DB` en `openspec/specs/ban-manager/spec.md:316`. Verificación: `grep -n '^### Requirement:' openspec/specs/ban-manager/spec.md` y el delta muestra el mismo título, carácter a carácter. **Hecho:** el título aparece idéntico en `openspec/specs/ban-manager/spec.md:316` y en el delta (`openspec/changes/ban-manager-persistence/specs/ban-manager/spec.md:3`), carácter a carácter.
- [x] 1.3 `design.md` escrito con las decisiones D1–D4, cada una con rationale y alternativa descartada. Verificación: `openspec status --change ban-manager-persistence` marca `4/4 artifacts complete`. **Hecho:** `openspec status --change ban-manager-persistence` → `Progress: 4/4 artifacts complete`.
- [x] 1.4 **Decisión del usuario sobre §D3 (B2).** Preguntar explícitamente si B2 entra en este change. Verificación: la respuesta queda registrada. **Hecho (2026-10-22): B2 entra** — el usuario aprobó el change **con B2** (y decidió **no** rescatar los baneos ya barridos, §D4). Por tanto las tareas 2.6, 2.7, 3.3, 4.2 y 6.5 se quedan y dejan de ser condicionales; el `deltaCount` esperado es **2**.

## 2. Tests (RED)

- [ ] 2.1 RED: test de que **dos IPs distintas conservan su baneo activo** tras `load_from_db`. Insertar una fila activa para `1.2.3.4` (`rule_id=1`) y otra para `5.6.7.8` (`rule_id=2`); asertar `SELECT COUNT(*) FROM bans WHERE expired = 0` es 2 y que ninguna fila se marca `expired=1`. Verificación: `cargo test` falla en aserción (hoy `MIN(rowid)` global deja 1 activa y marca la otra); los tests heredados siguen en verde.
- [ ] 2.2 RED: test de que **una IP con dos `rule_id` distintos conserva ambos baneos**. Insertar `(1.2.3.4, 1)` y `(1.2.3.4, 3)` activos; asertar 2 filas con `expired=0`. Verificación: el test falla en aserción (hoy sobrevive una sola).
- [ ] 2.3 RED: test de **mezcla de N únicos + M duplicados**. Insertar varias particiones únicas y duplicados de una de ellas; asertar que sobreviven exactamente **N + 1** filas activas y que las M duplicadas quedan `expired=1`. Verificación: el test falla en aserción.
- [ ] 2.4 RED: test de que **`rule_id=NULL` y `rule_id=1` son particiones distintas**. Insertar ambas activas para la misma IP; asertar 2 filas con `expired=0`. Verificación: el test falla en aserción.
- [ ] 2.5 RED: **corregir** el test `test_load_from_db_does_not_affect_unique_bans` (`ban_manager.rs` ≈1514) para que cubra de verdad lo que su nombre promete: insertar **varias particiones distintas** (no una sola fila en toda la tabla) y asertar que **ninguna** se marca `expired=1`. Verificación: el test corregido **falla en aserción antes del GREEN** —con el `MIN(rowid)` global las varias particiones colapsan a una sola fila activa, de modo que `SELECT COUNT(*) FROM bans WHERE expired = 0` da 1 en vez de N y al menos una partición única queda marcada `expired=1`— y **pasa después del GREEN**; el guardián `test_load_from_db_marks_duplicates_expired` queda en verde en ambos momentos.
- [ ] 2.6 RED: test de que el task periódico marca `expired=1` las filas temporalmente muertas (`banned_at + ban_duration_seconds < now`) y **no** las vivas. Verificación: el test falla (hoy `cleanup_expired_db` no se invoca desde el task); los heredados siguen en verde.
- [ ] 2.7 RED: test de que tras el reinicio la restauración de `escalation_counts` sólo considera los baneos vivos en la carga. Verificación: el test falla en aserción (hoy una fila caducada pero con `expired=0` restaura el nivel histórico).

> **Guardián de caracterización.** `test_load_from_db_marks_duplicates_expired`
> (`ban_manager.rs` ≈1447) usa una sola partición —el único caso que el bug
> respeta— y **debe seguir en verde** antes y después del GREEN. No se modifica;
> se anota como guardián de la semántica conservada.

## 3. Implementación (GREEN)

- [ ] 3.1 GREEN: sustituir el `UPDATE` de `load_from_db` (`ban_manager.rs:419-435`) por la forma sin agregado global —`rowid IN (… WHERE rn > 1)`—, marcando sólo los duplicados por partición. Verificación: 2.1, 2.2, 2.3, 2.4 y 2.5 pasan; el guardián de la nota sigue en verde.
- [ ] 3.2 GREEN: **verificar** (no editar) que el `#### Contract: Limpieza de duplicados en load_from_db` del **delta** (`openspec/changes/ban-manager-persistence/specs/ban-manager/spec.md`) es la forma corregida —`rowid IN (… WHERE rn > 1)`, sin ningún agregado global— y que su header `### Requirement: load_from_db limpia duplicados legacy en DB` coincide **literalmente** con el de `openspec/specs/ban-manager/spec.md:316`. Verificación: `grep -n '^### Requirement: load_from_db limpia duplicados legacy en DB$'` sobre el delta y sobre el destino devuelve títulos idénticos carácter a carácter, y el `Contract` del delta muestra `rn > 1`. **PROHIBIDO editar `openspec/specs/ban-manager/spec.md` a mano**: la spec destino la actualiza `openspec archive` al cerrar el change; si se reescribe directamente, el proposal queda huérfano, los headers dejan de coincidir y el `archive` falla (AGENTS.md §II, *Practical Lessons Learned*).
- [ ] 3.3 GREEN: quitar `#[allow(dead_code)]` de `cleanup_expired_db` (`ban_manager.rs:554`) e invocarla desde el task de 60 s (`main.rs:292-313`) junto a `cleanup_expired()`. Verificación: 2.6 y 2.7 pasan.
- [ ] 3.4 GREEN: documentar en el doc-comment de `load_from_db` la semántica por partición —una fila activa por `(ip_address, COALESCE(rule_id, 0))`, las demás intactas— y en el de `cleanup_expired_db` que se invoca periódicamente. Verificación: el doc-comment lo dice y `cargo check` no rompe.

## 4. REFACTOR

- [ ] 4.1 Revisar que no queda ningún agregado global (`MIN(rowid)` / `MAX(rowid)`) ni clave de agrupación duplicada en la limpieza. Verificación: `grep -n 'MIN(rowid)\|MAX(rowid)' backend/src/models/ban_manager.rs` no muestra coincidencias en la ruta de producción.
- [ ] 4.2 Verificar que `cleanup_expired_db` no conserva el `#[allow(dead_code)]` y que tiene exactamente un punto de llamada. Verificación: `grep -rn 'cleanup_expired_db' backend/src/` muestra la definición y la llamada del task.
- [ ] 4.3 `cargo fmt --all` y `cargo clippy -- -D warnings` en `backend/`. Verificación: ambos exit 0 y sin warnings nuevos.
- [ ] 4.4 Re-ejecutar `cargo test`. Verificación: exit 0 y recuento ≥ baseline de 1.1, sin tests ignorados nuevos.

## 5. Documentación

- [ ] 5.1 Anotar en `AGENTS.md` §V el contrato de persistencia del `BanManager`: la unidad de duplicidad es la partición `(ip_address, COALESCE(rule_id, 0))`, la limpieza periódica de la DB corre cada 60 s y la consecuencia sobre `escalation_counts`. Verificación: el texto nombra la partición, el intervalo y la consecuencia del escalado.
- [ ] 5.2 Registrar en `design.md` §Open Questions el cierre de las dos decisiones (§D3: B2 entra; §D4: sin rescate), con la fecha y el motivo. Verificación: §Open Questions no deja ninguna pregunta abierta y nombra ambas decisiones como del usuario.

## 6. Gates

- [ ] 6.1 `cargo fmt --all -- --check` en `backend/`. Verificación: exit 0.
- [ ] 6.2 `cargo clippy -- -D warnings` en `backend/`. Verificación: exit 0, sin warnings nuevos.
- [ ] 6.3 `openspec validate ban-manager-persistence --strict`. Verificación: exit 0 (`Change 'ban-manager-persistence' is valid`) y `deltaCount` de **2** (un MODIFIED + un ADDED sobre `ban-manager`).
- [ ] 6.4 Confirmar que el cambio **no** tocó nada fuera de alcance. Verificación: `git status` no muestra cambios en el frontend, en `migrations/`, en el pipeline WAF/Jail ni en la API HTTP.
- [ ] 6.5 Comprobar en una DB de prueba que tras el arranque no quedan filas `expired = 0` ya caducadas: `SELECT COUNT(*) FROM bans WHERE expired = 0 AND CAST(strftime('%s', banned_at) AS INTEGER) + ban_duration_seconds < CAST(strftime('%s','now') AS INTEGER)` → 0.

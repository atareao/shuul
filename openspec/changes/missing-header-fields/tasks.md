# Tasks: missing-header-fields

> **Alcance:** matcheo de reglas, mapeo documentado y diagnóstico. **No** se toca
> el transporte, ni el pipeline WAF/Jail, ni `run_jail_pipeline`, ni el
> `traefik.yml`.
> **Baseline:** anotar el recuento de `cargo test` vigente **antes** de empezar y
> exigir que no baje.

## 1. Preparación

- [ ] 1.1 Anotar el baseline: `cargo test` en verde y su recuento, en el árbol limpio sobre `develop`. Verificación: el recuento queda escrito aquí y los tests pasan (exit 0).
- [ ] 1.2 Confirmar que el header del delta coincide **literalmente** con el de la spec destino: `### Requirement: Traefik access log mapping` en `openspec/specs/journald-reader/spec.md`. Verificación: `grep -n '^### Requirement:' openspec/specs/journald-reader/spec.md` y el delta muestra el mismo título, carácter a carácter.
- [ ] 1.3 Confirmar que **no existe** capacidad previa `rule-matching`: el requisito nuevo no duplica ninguno existente. Verificación: `openspec list --specs` no lista `rule-matching` antes de este change.
- [ ] 1.4 `design.md` escrito con las decisiones D1–D5, cada una con rationale y alternativa descartada. Verificación: `openspec status --change missing-header-fields` marca `4/4 artifacts complete`.

## 2. Matcher (RED)

- [ ] 2.1 RED: test de que **un campo restringido y ausente hace que la regla NO case**: regla con regex de `user_agent` y petición sin `user_agent` → `matches()` es `false`. Verificación: `cargo test` falla porque hoy `check_match` devuelve `true` en `(Some(_), None)`; los tests heredados siguen en verde.
- [ ] 2.2 RED: test de que **un campo restringido y ausente descarta la regla aunque los demás campos casen**: regla con regex de `path` (que casa) y de `x_request_id` (ausente) → `matches()` es `false`. Verificación: el test falla.
- [ ] 2.3 RED: test de **neutralidad intacta**: regla que sólo restringe `path`, petición sin `user_agent`/`referer`/`x_request_id` y con `path` que casa → `matches()` es `true`. Verificación: el test **pasa ya** en rojo-por-construcción (guardián de 3.x); se anota como tal para que el GREEN no lo rompa.
- [ ] 2.4 RED: test de que **una cabecera presente pero vacía cuenta como ausente**: regla con regex de `referer` y petición con `Referer: ""` → `matches()` es `false`. Verificación: el test falla.
- [ ] 2.5 RED: test del **diagnóstico** del matcher: un match descartado por campo ausente emite un `debug!` que nombra el campo y la regla; un no-match por regex con el valor presente **no** lo emite. Verificación: el test falla por API inexistente (no hay punto de observación del log todavía).

## 3. Lector (guardianes de caracterización)

- [ ] 3.1 RED: test de que un access log **sin** claves `request_*` se mapea con los cinco campos a `None` **y** la línea no se descarta. Verificación: el test **pasa ya** —es el comportamiento actual— y queda como caracterización explícita del requisito nuevo; se anota como guardián.
- [ ] 3.2 RED: test de que un access log con **un subconjunto** de las claves `request_*` mapea las presentes y deja las ausentes a `None`. Verificación: guardián, pasa ya; se anota.

> **Nota.** El lector **no** se modifica: su mapeo ya devolvía `None` cuando la
> clave de origen falta, y ese comportamiento se conserva. Estas dos tareas sólo
> fijan esa garantía, sin implementación asociada.

## 4. Implementación (GREEN)

- [ ] 4.1 GREEN: `check_match` (`backend/src/models/rule.rs`) pasa a tres brazos explícitos con `(Some(_), None) => false`; la neutralidad de `(None, _)` no se toca. Verificación: 2.1, 2.2 y 2.4 pasan y 2.3 sigue en verde; los tests heredados de `matches()` siguen en verde.
- [ ] 4.2 GREEN: el `debug!` del match descartado, emitido **sólo** por el brazo de campo ausente, con el nombre del campo y el identificador de la regla. Verificación: 2.5 pasa.
- [ ] 4.3 GREEN: documentar en `backend/src/models/new_request.rs` que los cinco campos se resuelven a `None` cuando la cabecera falta o está vacía, y en `check_match` el contrato de los tres brazos. Verificación: el doc-comment lo dice y `cargo doc`/`cargo check` no rompen.

## 5. REFACTOR

- [ ] 5.1 Revisar si el brazo de campo ausente se repite en más de un sitio —el closure y su llamada desde `matches()`— y consolidarlo en `check_match` para que siga siendo el **único** punto de decisión. Verificación: `grep -n 'check_match' backend/src/models/rule.rs` muestra una sola definición y ninguna comparación duplicada de `None`.
- [ ] 5.2 `cargo fmt --all` y `cargo clippy -- -D warnings` en `backend/`. Verificación: ambos exit 0 y sin warnings nuevos.
- [ ] 5.3 Re-ejecutar `cargo test`. Verificación: exit 0 y recuento ≥ baseline de 1.1, sin tests ignorados nuevos.

## 6. Documentación

- [ ] 6.1 Documentar los campos que el origen no puede aportar y la clave que los recupera (`accessLog.fields.headers.names`), dejando claro que el `traefik.yml` vive fuera del repositorio. Verificación: el texto nombra las cinco claves y la opción de Traefik.
- [ ] 6.2 **Verificación de §D4:** confirmar que la clave que emite Traefik al declarar un nombre en `fields.headers.names` coincide **literalmente** con la que espera el parser (`request_<Nombre>`). Verificación: se anota el resultado medido —clave emitida observada—; si no coincide, se declara y se abre un change propio para el parser. **No** se implementan alias de capitalización aquí.
- [ ] 6.3 `design.md` §Open Questions actualizado con el resultado de 6.2. Verificación: la pregunta queda cerrada con la medición o marcada como abierta con su cambio pendiente.

## 7. Gates

- [ ] 7.1 `cargo fmt --all -- --check` en `backend/`. Verificación: exit 0.
- [ ] 7.2 `cargo clippy -- -D warnings` en `backend/`. Verificación: exit 0, sin warnings nuevos.
- [ ] 7.3 `openspec validate missing-header-fields --strict`. Verificación: exit 0 (`Change 'missing-header-fields' is valid`) y `deltaCount` de **2** (un MODIFIED sobre `journald-reader` y un ADDED sobre `rule-matching`).
- [ ] 7.4 Confirmar que el cambio **no** tocó nada fuera de alcance. Verificación: `git status` no muestra cambios en `backend/src/journald_reader.rs`, ni en el transporte, ni en `traefik.yml`, ni en `openspec/changes/docker-logs-transport/`.
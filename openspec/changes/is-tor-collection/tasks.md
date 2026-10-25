# Tasks: is-tor-collection

## 0. Contexto y dependencias

- [ ] 0.1 Confirmar el estado de partida: rama `feature/waf-is-tor-collection` sobre F2b-2b, sin changes activos huérfanos y árbol de trabajo limpio. **Verificación:** `openspec list` sin changes activos y `git status --short` limpio.
- [ ] 0.2 Medir la base antes de tocar nada: suite backend y frontend, clippy lib+bins y all-targets, y fmt. **Verificación:** los conteos pegados en la tarea (backend X/0/1, clippy 0, all-targets N, fmt 0, frontend M).

## 1. RED (tests que fallan)

- [ ] 1.1 Vocabulario `is_tor`: `field_name()`, `from_field_name("is_tor")`, `requires_key()` falso y que NO está en `Collection::ALL`. **Verificación:** fallan por la variante inexistente.
- [ ] 1.2 Operador `@truthy`: variante booleana de `Operator`, su serialización y su contrato (cumple si y solo si el booleano es exactamente `true`). **Verificación:** fallan por la variante inexistente.
- [ ] 1.3 Proyección desde la columna: `is_tor = Some(true)` proyecta **una** condición `is_tor`/`@truthy` al final; `Some(false)` y `None` **no** proyectan nada (caso `Some(false)` explícito). **Verificación:** fallan.
- [ ] 1.4 Matcheo: la colección `is_tor` resuelve el booleano de la petición; `Some(true)` casa, `Some(false)` y `None` no casan y emiten el `debug!`; sin anotación de evidencia. **Verificación:** fallan.
- [ ] 1.5 Persistencia/round-trip: la columna `operator` escribe y lee `@rx` y `@truthy`; una fila incoherente se descarta con diagnóstico; la proyección desde la tabla es equivalente a la de la columna. **Verificación:** fallan.
- [ ] 1.6 Validación 400: `@truthy` fuera de `is_tor` se rechaza; `@rx` sobre `is_tor` se rechaza; una condición de `is_tor` con `@truthy` se acepta. **Verificación:** fallan.
- [ ] 1.7 Migración de backfill: una base con reglas `is_tor = 1` anteriores recibe su fila `is_tor`/`@truthy`; idempotencia. **Verificación:** fallan.
- [ ] 1.8 Frontend: `projectConditions` incluye `is_tor` cuando `is_tor === true`; la inferencia de modo no degrada una regla simple con `is_tor`; el editor ofrece `@truthy` fijo y oculta patrón/clave/transformaciones para `is_tor`. **Verificación:** fallan.
- [ ] 1.9 Confirmar que la suite existente sigue verde salvo lo que la nueva forma rompe por compilación (retirada de `check_tor` y del campo `CacheRule.is_tor`) y que esos arreglos no cambian aserciones. **Verificación:** la suite compila con los tests nuevos en rojo y los previos intactos.

## 2. GREEN (implementación mínima)

- [ ] 2.1 Vocabulario: la variante `IsTor` con su nombre persistido y su inverso, sin clave y fuera de `Collection::ALL`. **Verificación:** 1.1 verde.
- [ ] 2.2 Operador: la variante booleana de `Operator`, su serialización `@truthy` y su aplicación en el matcheo. **Verificación:** 1.2 verde.
- [ ] 2.3 Proyección: `project()` añade la condición `is_tor`/`@truthy` al final cuando `is_tor == Some(true)`; nada para `Some(false)` ni `None`. **Verificación:** 1.3 verde.
- [ ] 2.4 Matcheo: resolución de `is_tor` desde el booleano de la petición, el `debug!` de ausencia y la ausencia de evidencia; retirada de `check_tor` y del campo `CacheRule.is_tor`. **Verificación:** 1.4 verde y la suite de F2a/F2b intacta.
- [ ] 2.5 Persistencia: la escritura y la lectura de la columna `operator` (`@rx`/`@truthy`) y el descarte de la fila incoherente con diagnóstico. **Verificación:** 1.5 verde.
- [ ] 2.6 Validación: `ConditionInput::compile` acepta `@truthy` solo para `is_tor` con patrón vacío, sin clave y sin transformaciones; rechaza las combinaciones inválidas con 400. **Verificación:** 1.6 verde.
- [ ] 2.7 Migración: `backend/migrations/20261026000000_project_is_tor_into_rule_conditions.up.sql`, backfill idempotente (`WHERE NOT EXISTS`). **Verificación:** 1.7 verde.
- [ ] 2.8 Frontend: `is_tor` en el catálogo, `Condition.operator` con `"@truthy"`, el editor con el operador fijo y `projectConditions` con la condición de `is_tor`. **Verificación:** 1.8 verde.
- [ ] 2.9 Cierre de GREEN: suite completa verde. **Verificación:** agregado de `cargo test --no-fail-fast` y `npx vitest run`.

## 3. REFACTOR y auditoría

- [ ] 3.1 `cargo fmt` aplicado; `cargo clippy --lib --bins -- -D warnings` en 0 y `cargo clippy --all-targets` en la base. **Verificación:** los tres comandos.
- [ ] 3.2 Auditoría con `rust-reviewer` y `react-reviewer` y remediación de los hallazgos (ALTOS y MEDIOS). **Verificación:** veredictos y cierre de hallazgos.
- [ ] 3.3 Revisar la coherencia de la fila irresoluble y la documentación interna de las decisiones (evidencia no anotada, `Some(false)` neutral). **Verificación:** lectura del código y clippy sin avisos nuevos.
- [ ] 3.4 Suite completa verde tras el refactor. **Verificación:** agregado de `cargo test --no-fail-fast`.

## 4. Documentación

- [ ] 4.1 `AGENTS.md`: `is_tor` como colección booleana, el operador `@truthy`, la proyección desde la columna, la retirada de `check_tor` y la ausencia de evidencia. **Verificación:** la sección describe el comportamiento implementado.
- [ ] 4.2 `WAF-ROADMAP.md`: marcar F2b-3 entregada y ajustar el desglose de F2b. **Verificación:** el nodo F2b y su desglose quedan al día.
- [ ] 4.3 `USER-GUIDE.md`: el filtro Tor como colección y su operador en la autoría avanzada. **Verificación:** el documento describe lo entregado.

## 5. Verificación y archivo

- [ ] 5.1 Suite backend completa. **Verificación:** agregado de `cargo test --no-fail-fast`.
- [ ] 5.2 Frontend verde. **Verificación:** `npx vitest run` y `npx tsc -b`.
- [ ] 5.3 Clippy lib+bins en 0, all-targets en la base, fmt en 0. **Verificación:** los tres comandos.
- [ ] 5.4 Cero cambio observable: la equivalencia sobre las 42 plantillas (incluida «Tor exit nodes») sigue verde y la forma del evento no cambia. **Verificación:** los tests de equivalencia pasan.
- [ ] 5.5 Archivar el change con `openspec archive is-tor-collection --yes` y comprobar que las dos specs destino reciben sus requisitos. **Verificación:** `openspec list` sin changes activos y los requisitos presentes.

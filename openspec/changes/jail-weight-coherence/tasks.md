# Tasks: jail-weight-coherence

## 1. RED (tests que fallan primero)

- [x] 1.1 Backend: test que fija que una regla creada con pipeline `jail` y un peso distinto de 0 queda almacenada con peso 0.
- [x] 1.2 Backend: test que fija que una regla de Jail editada con un peso distinto de 0 sigue con peso 0.
- [x] 1.3 Backend: test que fija que la migración de normalización pone a 0 las reglas de Jail existentes y respeta el peso de las del WAF.
- [x] 1.4 Backend: test que fija que la lista de reglas con pesos repetidos devuelve el mismo orden en dos consultas consecutivas.
- [x] 1.5 Backend: test que fija que recorrer las páginas de un conjunto de reglas empatadas no repite ni pierde ninguna.
- [x] 1.6 Backend: test que fija que las 18 plantillas de Jail del catálogo declaran peso 0 y que las del WAF conservan los suyos.
- [x] 1.7 Frontend: test que fija que el campo Weight no se ofrece al aplicar una plantilla de Jail.
- [x] 1.8 Frontend: test que fija que el campo Weight no se ofrece en el diálogo de regla cuando el pipeline es Jail.
- [x] 1.9 Ejecutar las dos suites: las nuevas fallan y **todas** las existentes siguen en verde.

## 2. GREEN (implementación mínima)

- [x] 2.1 `backend/src/templates.rs`: peso 0 en las 18 plantillas de Jail.
- [x] 2.2 `backend`: peso 0 impuesto a las reglas con pipeline `jail` en la creación y en la edición, sin confiar en el cliente.
- [x] 2.3 `backend/migrations/20261023000000_normalize_jail_rule_weight.up.sql`: normalización de las reglas de Jail existentes, idempotente y sin tocar las del WAF.
- [x] 2.4 `backend/src/models/rule.rs`: desempate determinista en el `ORDER BY` de la lista de reglas (no en el orden de evaluación del WAF).
- [x] 2.5 Frontend: `templates_page.tsx` deja de ofrecer el campo Weight para Jail y el peso que viaja para una regla de Jail pasa a ser 0 en los dos diálogos (en `rule_dialog.tsx` el campo ya estaba oculto).
- [x] 2.6 Ambas suites en verde y `cargo check` limpio.

## 3. REFACTOR

- [x] 3.1 `cargo clippy -- -D warnings`, `cargo fmt --check`, `npm run lint` y `tsc -b --force` limpios.
- [x] 3.2 Documentar en `AGENTS.md`, junto a la descripción de los pipelines, que el peso de una regla de Jail es siempre 0 y por qué (el pipeline de Jail no ordena).

## 4. Verificación manual en producción (la hace el usuario)

- [ ] 4.1 Con la versión nueva desplegada, la tabla de Rules ordenada por Weight muestra las reglas de Jail agrupadas y con el mismo orden al reordenar.
- [ ] 4.2 Recorrer las páginas de la tabla ordenada por Weight no repite ni pierde ninguna regla.
- [ ] 4.3 El diálogo de una plantilla de Jail ya no ofrece Weight; el de una regla de Jail, tampoco.

## 5. Notas del RED (evidencia medida)

- Backend: `cargo test --no-fail-fast` → 6 tests nuevos en rojo y 372 preexistentes verdes. Commit `2cef053`.
- Frontend: `npx vitest run` → 2 tests nuevos en rojo y 39 preexistentes verdes; `tsc -b --force` limpio. Commit `20adc89`.
- Los tests 1.4 y 1.5 perturban a propósito el plan de consulta (índice creado entre dos listados) porque SQLite, sin esa perturbación, devuelve los empates en orden de rowid y el defecto quedaría invisible.
- El test 1.1 cubre también la vía de importación: si la coerción no alcanza la importación, 1.1 falla por ese bloque.
- `rules.is_tor` es `NOT NULL` y el PATCH lo bindea tal cual: el payload debe traerlo. Comportamiento preexistente, ajeno a este change.

## 6. REFACTOR tras la auditoría (hallazgos de los revisores)

- [ ] 6.1 M1: el pipeline se valida en la escritura (creación, edición e importación) y se rechaza lo no admitido; la migración alcanza además el legado con otra capitalización (`COLLATE NOCASE`).
- [x] 6.2 m1: test de la rama `ON CONFLICT DO UPDATE` de la importación (reimportar el mismo nombre con un peso distinto de 0).
- [x] 6.3 m2: el test de la migración comprueba la idempotencia ejecutando el SQL dos veces.
- [x] 6.4 m3: el test de creación se parte para no arrastrar `clippy::too_many_lines`.
- [x] 6.5 m5: la migración documenta por qué no toca `updated_at`.
- [x] 6.6 Frontend: test de UPDATE en `rule_dialog.test.tsx` y helper único para la comparación con el pipeline `jail`.
- [ ] 6.7 Aplazado a su propio change: m4 (un `sort_by` fuera de la lista blanca desactiva todo el `ORDER BY`, comportamiento preexistente) y la explicación al usuario de por qué Jail no tiene peso.

- [ ] 6.8 El rechazo no se queda en la API: la UI del import deja de decir «Imported N rules» cuando hubo entradas rechazadas y las nombra.
- [ ] 6.9 Aplazado a su propio change: una fila del WAF con la etiqueta en mayúsculas (`WAF`) no la salta el pipeline de Jail —la comparación literal de `report.rs` es `waf`—, así que participa de un pipeline que no le toca. Es preexistente y ajeno a la invariante del peso de Jail.

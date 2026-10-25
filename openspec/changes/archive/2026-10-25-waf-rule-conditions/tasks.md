# Tasks: waf-rule-conditions

## 0. Contexto y dependencias

- [x] 0.1 Confirmar que `openspec/specs/rules/` existe y que ningún header del delta coincide con un header ya existente de la spec destino (los existentes son: «El peso de una regla de Jail es siempre 0», «La ordenación de la lista de reglas es estable», «El pipeline de una regla se valida en la escritura», «La especificación de transformaciones se valida en la escritura», «Una regla con una especificación irresoluble no entra en la caché»).
- [x] 0.2 Confirmar que `openspec list` no muestra ningún otro change activo.
- [x] 0.3 Medir la base **antes** de tocar nada: `cargo test --no-fail-fast`, `cargo clippy --lib --bins -- -D warnings` (0) y `cargo clippy --all-targets`. **Medido:** backend base 537 verdes / 0 rojos / 1 ignorado antes de esta sub-entrega.
- [x] 0.4 Confirmar las tres vías de escritura (`Rule::create`, `Rule::update`, `Rule::import_handler`) y la de lectura (`CacheRule::read_all_active`), y que las 14 columnas de `rules` son la única fuente del `Vec<Condition>` actual.

## 1. RED (tests que fallan)

- [x] 1.1 Test: una regla con `path` restringido, tras crearse, tiene una fila en `rule_conditions` con `collection = 'path'` y el patrón de la regla.
- [x] 1.2 Test: una regla con `path` restringido y `query` en blanco genera solo la fila de `path`.
- [x] 1.3 Test: al editar una regla, el conjunto persistido es exactamente el de la nueva versión, sin filas de la anterior.
- [x] 1.4 Test: al eliminar una regla, sus filas de `rule_conditions` desaparecen.
- [x] 1.5 Test: la migración rellena las reglas existentes y la proyección desde las filas coincide con la de los 14 campos.
- [x] 1.6 Test de equivalencia: sobre las 42 plantillas del catálogo, el conjunto leído desde la tabla es el mismo que proyecta `project()` desde los 14 campos.
- [x] 1.7 Test: el ciclo export/import deja las condiciones persistidas igual que antes.
- [x] 1.8 Ejecutar la suite y comprobar que los tests nuevos fallan y el resto sigue verde. **Medido:** los 8 tests nuevos en rojo (tabla `rule_conditions` inexistente) y los 537 previos intactos.

## 2. GREEN (implementación mínima)

- [x] 2.1 Migración `.up.sql`: `CREATE TABLE rule_conditions` con `UNIQUE(rule_id, position)` y `ON DELETE CASCADE`, más el backfill desde las 14 columnas.
- [x] 2.2 Helper de persistencia: proyectar los 14 campos a filas y leer las filas de vuelta a `Condition` (con el parseo `Collection` ↔ `TEXT` y `position` contigua 0-based).
- [x] 2.3 `read_all_active` lee el conjunto desde `rule_conditions`.
- [x] 2.4 `Rule::create` persiste las condiciones tras escribir la fila.
- [x] 2.5 `Rule::update` reemplaza el conjunto de condiciones.
- [x] 2.6 `import_handler` persiste las condiciones de cada regla importada.
- [x] 2.7 `Rule::delete` deja que la cascada borre las condiciones.
- [x] 2.8 Ejecutar la suite (todo verde) y los type checks. **Medido:** 545 verdes / 0 rojos / 1 ignorado (537 + los 8 nuevos).

## 3. REFACTOR

- [x] 3.1 Consolidar el helper de persistencia y la lectura en un único punto, sin duplicar la proyección.
- [x] 3.2 `cargo fmt`, `cargo clippy --lib --bins -- -D warnings` en 0 y `cargo clippy --all-targets` sin nuevos avisos. **Verificado:** `cargo fmt --check` 0, `cargo clippy --lib --bins -- -D warnings` 0 y `cargo clippy --all-targets` 83 (la base).
- [x] 3.3 Confirmar que ningún test existente se ha debilitado ni borrado.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: actualizar la sección del esquema y la persistencia (tabla `rule_conditions`, la tabla como fuente del matcheo, las 14 columnas como representación simple).
- [x] 4.2 `WAF-ROADMAP.md`: marcar F2b-1 como entregada y ajustar el diagrama/§F2 si procede.
- [x] 4.3 Archivar el change con `openspec archive waf-rule-conditions --yes` y comprobar que la spec `rules` recibe los requisitos. **Verificado:** archivado como `2026-10-25-waf-rule-conditions`; `openspec/specs/rules/spec.md` pasó de 5 a 8 requisitos (+3).

## 5. Verificación

- [x] 5.1 `cargo test --no-fail-fast` en verde (pegar el total passed/failed/ignored). **Medido:** 547 verdes / 0 rojos / 1 ignorado (545 + el test unitario de `from_field_name`, contado en lib y bin).
- [x] 5.2 `cargo clippy --lib --bins -- -D warnings` en 0 y `cargo clippy --all-targets` sin subir la base.
- [x] 5.3 `npx vitest run` en verde (el frontend no cambia, pero se confirma que sigue verde). **Medido:** frontend 96 verdes (sin cambios en el cambio).
- [x] 5.4 Confirmar **cero cambio observable**: mismo `matched`, misma evidencia y misma oferta de API para el mismo juego de reglas. **Verificado:** mismo `matched`, misma evidencia y misma oferta de API; el 8.º test del catálogo prueba la equivalencia tabla↔14 campos sobre las 42 plantillas, y la suite heredada de F2a sigue verde.

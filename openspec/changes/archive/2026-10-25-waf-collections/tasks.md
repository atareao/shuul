# Tasks: waf-collections

## 0. Contexto y dependencias

- [x] 0.1 Confirmar el estado de partida: rama `develop` con F2b-1 integrado (la tabla de condiciones es la fuente del matcheo), `openspec list` sin changes activos y árbol de trabajo limpio. **Verificación:** `openspec list` sin changes activos y `git status --short` vacío. ✅ Confirmado.
- [x] 0.2 Medir la base antes de tocar nada: suite backend y frontend, clippy lib+bins y all-targets, y fmt. **Verificación:** los conteos pegados en la tarea (backend X/0/1, clippy 0, all-targets 83, fmt 0, frontend N). ✅ Base medida: backend **547/0/1**; `cargo clippy --lib --bins` **0**; `--all-targets` **83**; `cargo fmt --check` **0**; frontend **96**.

## 1. RED (tests que fallan)

- [x] 1.1 Tests de extracción: cookies desde `Cookie`, cabeceras con nombre en minúsculas, argumentos del query uno a uno y **crudos**, basename y extensión, y el caso del Jail que sólo deriva lo que su payload trae. **Verificación:** fallan por los campos inexistentes del modelo de la petición. ✅ Cinco tests en `models::new_request::collection_extraction_tests`.
- [x] 1.2 Tests del vocabulario: nombre persistido y su inverso para las cinco colecciones, qué colecciones exigen clave, y la resolución del valor por clave (presente y ausente). **Verificación:** fallan por las variantes inexistentes. ✅ Tests en `models::condition::tests`.
- [x] 1.3 Tests del matcheo: una condición de cookie casa por su nombre; una cookie ausente no casa y diagnostica; la evidencia usa la clave compuesta y distingue dos condiciones de la misma colección; la condición lleva su clave. **Verificación:** fallan. ✅ `test_cookie_condition_matches_by_name` y `test_evidence_uses_composite_key`.
- [x] 1.4 Tests de persistencia: la clave sobrevive el ciclo escritura-lectura; una condición sin clave la conserva nula; el modo simple queda intacto (equivalencia sobre las 42 plantillas). **Verificación:** fallan. ✅ Tests de la sección `waf-collections (F2b-2a)`.
- [x] 1.5 Tests de integración de la autoría avanzada: crear con lista de condiciones y persistirlas en orden; rechazo con 400 de colección desconocida, de clave faltante en colección con clave, de clave sobrante en colección sin clave, y de patrón que no compila; el ciclo de lectura reenviando las condiciones; exportación/importación que conservan una regla avanzada. **Verificación:** fallan. ✅ Diez tests de integración.
- [x] 1.6 Confirmar que la suite existente sigue verde salvo lo que la nueva forma rompe por compilación (los constructores de la petición y de la condición) y que esos arreglos no cambian aserciones. **Verificación:** la suite compila con los tests nuevos en rojo y los previos intactos. ✅ RED verificado: **556 passed / 24 failed / 1 ignored** (doce tests nuevos × dos targets).

## 2. GREEN (implementación mínima)

- [x] 2.1 Extracción de las colecciones nuevas en el modelo de la petición, con el helper compartido para lo derivable del path y del query, y el poblado del literal del Jail. **Verificación:** 1.1 verde. ✅ Tanda B: `from_request` y `derive_path_collections()`, compartido con el Jail.
- [x] 2.2 Las cinco variantes del vocabulario con su nombre persistido y su inverso, la declaración de qué colecciones exigen clave, y la resolución del valor por clave. **Verificación:** 1.2 verde. ✅ `Collection::from_field_name`, `Collection::requires_key` y `request_value(request, key)`.
- [x] 2.3 La clave en la condición, su resolución en el matcheo y la clave compuesta de la evidencia. **Verificación:** 1.3 verde y la suite de F2a/F2b-1 intacta. ✅ Tanda B.
- [x] 2.4 La columna `key` en la escritura y en la lectura de las condiciones persistidas. **Verificación:** 1.4 verde. ✅ Tanda A: `write_conditions`, `read_conditions` y `read_conditions_batch`.
- [x] 2.5 El campo `conditions` en los modelos de escritura, el validador único, su uso en crear/editar/importar, y las condiciones en la representación de la regla (por lote en el listado y la exportación). **Verificación:** 1.5 verde. ✅ Tanda A: `ConditionInput::compile`, `RuleResponse` y `responses_for_rules`.
- [x] 2.6 Cierre de GREEN: suite completa verde. **Verificación:** agregado de `cargo test --no-fail-fast`. ✅ **580 passed / 0 failed / 1 ignored** e integración **19/19**.
- [x] 2.7 Auditoría con `rust-reviewer` y remediación de los hallazgos. **Verificación:** veredicto y cierre de ALTOS y MEDIOS. ✅ Veredicto **APROBADO CON OBSERVACIONES** (sin CRÍTICOS). Remediados: ALTO #1 (`condition_from_row` descarta transformaciones irresolubles con diagnóstico), ALTO #2 (`is_tor` en la importación, 26 binds), MEDIO #3 (descarte de colección con clave sin clave), MEDIO #4 (tests de rechazo en `PATCH` e `import`), MEDIO #5 (inverso de `from_field_name` para las cinco), MEDIO #6 (tests de duplicados, «último gana») y los BAJOS. Suite final **587/0/1**; integración **22/22**.

## 3. REFACTOR

- [x] 3.1 `cargo fmt` aplicado; `cargo clippy --lib --bins -- -D warnings` en 0 y `cargo clippy --all-targets` en la base (83). **Verificación:** los tres comandos. ✅ fmt **0**, clippy lib+bins **0**, all-targets **83**.
- [x] 3.2 Revisar la duplicación del parseo del query y de las cookies (un helper único) y la documentación interna de las decisiones. **Verificación:** lectura del código y clippy sin avisos nuevos. ✅ Sin duplicación: `from_request` delega en `derive_path_collections`; los cuatro `trivial_regex` se resolvieron con `#[allow]` justificado; se retiraron los `#[allow(dead_code)]` provisionales y se limpió la documentación de fases RED/GREEN.
- [x] 3.3 Suite completa verde. **Verificación:** agregado de `cargo test --no-fail-fast`. ✅ **587/0/1**.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: el vocabulario de colecciones nuevas, la clave y su evidencia compuesta, el modo simple frente al avanzado, la lista de condiciones en la API y la representación con condiciones. **Verificación:** la sección describe el comportamiento implementado. ✅ Sección `CacheRule` actualizada.
- [x] 4.2 `WAF-ROADMAP.md`: marcar F2b-2a entregada y ajustar el desglose de F2b-2 en F2b-2a/F2b-2b. **Verificación:** el nodo F2b y su desglose quedan al día. ✅
- [x] 4.3 `USER-GUIDE.md` y `SELF-HOSTING.md`: la autoría avanzada por API y la limitación de que las colecciones con clave son del WAF. **Verificación:** ambos documentos describen lo entregado. ✅

## 5. Verificación

- [x] 5.1 Suite backend completa. **Verificación:** agregado de `cargo test --no-fail-fast`. ✅ **587 passed / 0 failed / 1 ignored** (547 base + 40 nuevos).
- [x] 5.2 Frontend intacto (no se toca en esta sub-entrega). **Verificación:** `npx vitest run` verde. ✅ **96/96** (12 ficheros).
- [x] 5.3 Clippy lib+bins en 0, all-targets en la base, fmt en 0. **Verificación:** los tres comandos. ✅ clippy lib+bins **0**, all-targets **83**, fmt **0**.
- [x] 5.4 Cero cambio observable en modo simple: test de caracterización sobre las 42 plantillas y la equivalencia tabla↔14 campos de F2b-1 sigue verde. **Verificación:** los tests de equivalencia pasan. ✅ `test_projection_from_table_is_equivalent_over_the_catalog` y `test_evaluate_is_equivalent_over_the_fourteen_fields`, ambos verdes.
- [x] 5.5 Archivar el change con `openspec archive waf-collections --yes` y comprobar que las tres specs destino reciben sus requisitos. **Verificación:** `openspec list` sin changes activos y los requisitos presentes. ✅ Acción final de la sub-entrega.

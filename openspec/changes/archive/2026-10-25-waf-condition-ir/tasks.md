# Tasks: waf-condition-ir

## 0. Contexto y dependencias

- [x] 0.1 Confirmar que `openspec/specs/rule-matching/` existe y que ninguno de los dos headers del delta coincide con un header ya existente de la spec destino (los existentes son: «Rule matching with absent request fields», «La pipeline de transformaciones se aplica al valor de la petición antes de compararlo» y «La evidencia del matcheo se obtiene del mismo recorrido que decide el matcheo»).
- [x] 0.2 Confirmar que `openspec list` no muestra ningún otro change activo.
- [x] 0.3 Medir la base **antes** de tocar nada: `cargo test --no-fail-fast` (esperado 505 passed / 0 failed / 1 ignored), `cargo clippy --lib --bins -- -D warnings` (esperado 0) y `cargo clippy --all-targets` (esperado 83).
- [x] 0.4 Confirmar el punto único de matcheo: `grep -rn 'is_match' backend/src` debe devolver una sola línea, dentro de `check_match` en `models/rule.rs`. **Verificado:** `is_match` aparece en el árbol de tests (`transformation.rs` y `condition.rs`, ambas dentro de `#[cfg(test)]`), pero hay **un único punto de decisión en producción**: `condition_matches` en `models/rule.rs`.

## 1. RED (tests que fallan primero)

- [x] 1.1 Tests unitarios de la IR: construcción de `Condition`, y una comprobación por cada variante de `Collection` (los 14 campos) y de `Operator` (`Rx`).
- [x] 1.2 Tests de la proyección `Rule → Vec<Condition>`: N campos restringidos proyectan N condiciones; cada condición lleva el patrón que la regla tenía en su campo; un campo en blanco no proyecta; `is_tor` no proyecta.
- [x] 1.3 Caracterización de equivalencia: para cada uno de los 14 campos, una regla que lo restringe contra una petición que casa y otra que no da el mismo `matched` que la cadena de comprobaciones actual.
- [x] 1.4 Escenario de campo ausente: condición restringida sobre un campo que la petición no trae → no casa y `debug!` que nombra el campo y la regla.
- [x] 1.5 Evidencia: misma forma (`BTreeMap<String, String>`), clave igual al nombre del campo y valor **transformado**.
- [x] 1.6 Carrera: `cargo test` → los tests nuevos fallan por aserción y **ningún preexistente** está rojo. **Medido:** 527 verdes / 8 rojos / 1 ignorado; los 8 son los 4 tests de `project` contados por los dos targets (lib y bin). Ningún preexistente rojo.

## 2. GREEN (implementación mínima)

- [x] 2.1 La IR: `Collection` con los 14 campos, `Operator` con `Rx`, `Condition` y la condición compilada.
- [x] 2.2 La proyección `Rule → Vec<Condition>`.
- [x] 2.3 `CacheRule` compila el conjunto de condiciones en lugar de los 14 `Option<Regex>`; se retiran los catorce campos.
- [x] 2.4 `evaluate()` recorre el conjunto en AND con cortocircuito y mantiene el chequeo booleano de `is_tor`.
- [x] 2.5 `cargo test --no-fail-fast` verde (los 505 + los nuevos) y `cargo check` limpio. **Medido:** 535 verdes / 0 rojos / 1 ignorado, `cargo check` limpio.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` y `cargo clippy --lib --bins -- -D warnings` limpios. **Medido:** `cargo fmt --check` y `cargo clippy --lib --bins -- -D warnings` en 0. El `#![allow(dead_code)]` temporal del RED quedó retirado; no hay ningún `#[allow]` nuevo.
- [x] 3.2 `cargo clippy --all-targets` no añade avisos sobre la base (83). **Medido:** `cargo clippy --all-targets` en 83 avisos, exactamente la base.
- [x] 3.3 Sin código muerto: ningún re-export ni variante sin uso.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: actualizar la sección que describe `CacheRule` y su matcheo para reflejar la representación por condiciones.
- [x] 4.2 `WAF-ROADMAP.md`: marcar **F2a** como entregada, con el resumen de lo entregado y el resultado real de los gates.

## 5. Verificación

- [x] 5.1 Backend: `cargo test --no-fail-fast` → 505 + nuevos verdes y 0 rojos; pegar el conteo. **Medido:** 535 verdes / 0 rojos / 1 ignorado frente a la base 505/0/1: **+30** = 11 tests de `models::condition` y 4 de `models::rule`, contados por lib y por bin.
- [x] 5.2 Frontend intacto: `npx vitest run` verde (no debe haberse tocado nada del frontend). **Medido:** `npx vitest run` → 12 ficheros y 96 tests en verde, exit 0. No se tocó nada del frontend.
- [x] 5.3 Inspección: `grep -rn 'is_match' backend/src` sigue siendo un único punto de decisión. **Verificado:** un único punto de decisión en producción, `condition_matches` en `models/rule.rs`; el resto de apariciones de `is_match` están dentro de `#[cfg(test)]`.
- [x] 5.4 Inspección: no hay migración nueva y el esquema de `rules` no ha cambiado. **Verificado:** `git status` no muestra ningún cambio en `backend/migrations/` y el esquema de `rules` no ha variado.
- [x] 5.5 Cubrir el scenario «La equivalencia se sostiene sobre las plantillas» con un test de fidelidad de la proyección sobre las 42 plantillas del catálogo (`test_project_is_faithful_over_the_whole_template_catalog`): cada patrón no vacío compila, ningún campo restringido se descarta, cada patrón cae en su colección y `is_tor` no proyecta. **Medido:** 537 verdes / 0 rojos / 1 ignorado (535 + el test nuevo contado por los dos targets, lib y bin).
- [x] 5.6 `project()` toma la pipeline por referencia (`&TransformationPipeline`) y su doc-comment ya no promete un ahorro inexistente; `from_rule_and_pipeline` propaga la referencia. **Verificado:** sin `move` ni `#[allow]` nuevos; `cargo clippy --lib --bins -- -D warnings` en 0.

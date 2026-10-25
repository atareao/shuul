# Tasks: waf-string-operators

## 0. Contexto y dependencias

- [ ] 0.1 Confirmar el estado de partida: rama `feature/waf-operators` sobre F2b-3 integrada (`is_tor`/`@truthy` ya son una condición más y `Operator` tiene dos variantes), `openspec list` sin changes activos huérfanos y árbol de trabajo limpio. **Verificación:** `openspec list` sin changes activos y `git status --short` con lo único pendiente de este change.
- [ ] 0.2 Medir la base antes de tocar nada: suite backend y frontend, clippy lib+bins y all-targets, y fmt. **Verificación:** los conteos pegados en la tarea —referencia de partida medida en F2b-3: backend **620** verdes / **0** rojos / 1 ignorado, `cargo clippy --lib --bins -- -D warnings` **0**, `cargo clippy --all-targets` **82**, `cargo fmt --check` **0**, frontend **118** verdes, `npx tsc -b` **0**, `npx eslint .` **0**—.
- [ ] 0.3 Confirmar los puntos de anclaje del cambio en el código real: `Operator` en `backend/src/models/condition.rs`, `condition_matches`, `From<&Condition> for ConditionInput`, `ConditionInput::compile`, `write_conditions` y `condition_from_row` en `backend/src/models/rule.rs`, y el editor en `frontend/src/components/dialogs/condition_editor*.tsx|ts` y `frontend/src/models/rule.ts`. **Verificación:** cada punto de anclaje localizado por `grep` y anotado en la tarea.

## 1. RED (tests que fallan)

- [ ] 1.1 Vocabulario de operadores: las seis variantes nuevas de `Operator` con su `name()` canónico —`@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within`, `@unconditionalMatch`— y su `pattern()` canónico (prestado, construido para `@within`, vacío para `@unconditionalMatch`). **Verificación:** los tests fallan por las variantes inexistentes.
- [ ] 1.2 Semántica de `@contains`: casa cuando el valor contiene el patrón, **con distinción de mayúsculas**, y no casa cuando no lo contiene. **Verificación:** el test falla por la variante inexistente.
- [ ] 1.3 Semántica de `@streq`: casa con la igualdad exacta y no casa con una diferencia de un carácter ni con distinta capitalización. **Verificación:** el test falla.
- [ ] 1.4 Semántica de `@beginsWith`: casa por el principio y no casa cuando el patrón aparece más adentro. **Verificación:** el test falla.
- [ ] 1.5 Semántica de `@endsWith`: casa por el final y no casa cuando el patrón aparece antes. **Verificación:** el test falla.
- [ ] 1.6 Semántica de `@within`: casa cuando el valor es uno de los elementos de la lista, **recortando espacios** de cada elemento; no casa cuando el valor no está en la lista; es el inverso de `@contains` sobre el mismo patrón; y un elemento **vacío** del patrón no casa ningún valor —no actúa como comodín—. **Verificación:** los tests fallan.
- [ ] 1.7 Semántica de `@unconditionalMatch`: casa con cualquier valor de la colección presente; **no** casa cuando la colección está ausente y emite el `debug!` que la nombra. **Verificación:** los tests fallan.
- [ ] 1.8 Contrato compartido de los operadores de cadena: la pipeline se aplica **al valor y nunca al patrón** (patrón `get` con `t:lowercase` casa `GET`, patrón `GET` no casa); un operador que no casa detiene la conjunción por cortocircuito; un operador de cadena sobre `is_tor` es incoherente y no casa, con diagnóstico. **Verificación:** los tests fallan.
- [ ] 1.9 Evidencia: un operador de cadena con pipeline **no vacía** anota el valor **transformado** bajo la clave de la colección (y bajo la clave compuesta `colección[clave]` en las colecciones con clave); con pipeline **vacía** no anota nada; `@unconditionalMatch` sigue el mismo contrato. **Verificación:** los tests fallan.
- [ ] 1.10 Validación de la autoría avanzada (400): operador de cadena con patrón no vacío se acepta; operador de cadena con patrón vacío → 400; `@unconditionalMatch` con patrón vacío se acepta y con patrón no vacío → 400; operador desconocido (`@ipMatch`, aún fuera de alcance) → 400; operador de cadena sobre `is_tor` → 400; `@truthy` fuera de `is_tor` y `@rx` sobre `is_tor` siguen siendo 400. **Verificación:** los tests fallan y los previos de F2b-3 siguen verdes.
- [ ] 1.11 Persistencia y round-trip: cada operador de cadena se escribe con su nombre y su patrón canónicos y se relee igual; el patrón de `@within` se canoniza a elementos recortados unidos por coma sin espacios y el round-trip es estable; una fila con operador **desconocido** se descarta con diagnóstico que la nombra; una fila incoherente (`@contains` con patrón vacío, `@unconditionalMatch` con patrón, operador de cadena sobre `is_tor`) se descarta con diagnóstico; la proyección desde la tabla sigue siendo equivalente a la del modo simple. **Verificación:** los tests fallan.
- [ ] 1.12 El caso peligroso: una regla existente en **modo simple** —los 14 campos y la columna `is_tor`— **no** cambia de comportamiento: `project()` sigue produciendo `@rx`/`@truthy`, ninguna fila nueva lleva un operador de cadena y las 42 plantillas deciden y evidencian igual que en F2b-3. **Verificación:** los tests fallan por la ausencia de la garantía explícita y los de equivalencia de F2b-3 siguen verdes.
- [ ] 1.13 Frontend: el tipo del operador y el catálogo de los siete; el selector por fila con `@rx` por defecto; `@unconditionalMatch` oculta el patrón y lo envía vacío; `is_tor` conserva `@truthy` fijo y sin selector; la validación de fila impide guardar con patrón vacío salvo en `@unconditionalMatch`; la inferencia de modo no degrada una regla simple y una avanzada con operadores de cadena abre en avanzado con su lista y sus operadores. **Verificación:** los tests fallan.
- [ ] 1.14 Confirmar que la suite existente sigue verde salvo lo que la nueva forma rompe por compilación (el `match` sobre `Operator` en los consumidores del patrón) y que esos arreglos no cambian aserciones. **Verificación:** la suite compila con los tests nuevos en rojo y los previos intactos.

## 2. GREEN (implementación mínima)

- [ ] 2.1 `Operator`: las seis variantes con su nombre canónico y su `pattern()` canónico; `name()` cubre las ocho. **Verificación:** 1.1 verde.
- [ ] 2.2 Matcheo: `condition_matches` decide por operador sobre el valor **transformado**, con la semántica de cada uno —`@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` con elementos recortados y elemento vacío neutralizado, `@unconditionalMatch`— y mantiene el brazo de la colección ausente por delante del operador. **Verificación:** 1.2 a 1.8 verdes.
- [ ] 2.3 Evidencia: el contrato único —casa y pipeline no vacía— con el valor transformado bajo la clave de la colección o la clave compuesta. **Verificación:** 1.9 verde.
- [ ] 2.4 Validación: `ConditionInput::compile` resuelve el operador con la tabla por operador y colección, con patrón no vacío para los de cadena, patrón vacío para `@unconditionalMatch` y los rechazos existentes intactos. **Verificación:** 1.10 verde.
- [ ] 2.5 Persistencia: `write_conditions` escribe `operator.name()` y `operator.pattern()`; `condition_from_row` reconstruye el operador desde el nombre y el patrón y descarta la fila irresoluble con diagnóstico; `From<&Condition> for ConditionInput` lee el patrón del operador. **Verificación:** 1.11 verde.
- [ ] 2.6 Cero cambio en el modo simple: `project()` sigue compilando `Operator::Rx` y la columna `is_tor` sigue proyectando `@truthy`, sin ninguna ruta que produzca un operador de cadena desde los 14 campos. **Verificación:** 1.12 verde y las 42 plantillas equivalentes.
- [ ] 2.7 Frontend: el tipo del operador, el catálogo de los siete, el selector por fila, el ocultado del patrón en `@unconditionalMatch`, la validación de fila por operador y la serialización del operador elegido. **Verificación:** 1.13 verde.
- [ ] 2.8 Cierre de GREEN: suite completa verde en backend y frontend. **Verificación:** agregado de `cargo test --no-fail-fast` y `npx vitest run`, con los conteos pegados en la tarea.

## 3. REFACTOR + auditoría

- [ ] 3.1 Retirar la duplicación del `match` sobre `Operator`: los consumidores del patrón (representación, escritura, evidencia) preguntan al operador por su patrón canónico en vez de ramificar por variante. **Verificación:** `grep` sin `match` sobre `Operator` fuera de `name()`, `pattern()` y el punto de decisión del matcheo.
- [ ] 3.2 `cargo fmt` aplicado y `cargo fmt --check` limpio. **Verificación:** `cargo fmt --check` en **0**.
- [ ] 3.3 `cargo clippy --lib --bins -- -D warnings` en 0 y `cargo clippy --all-targets` en la base, sin avisos nuevos. **Verificación:** los dos comandos, con el conteo de all-targets pegado.
- [ ] 3.4 Frontend: `npx tsc -b` en 0 y `npx eslint .` en 0, sin `eslint-disable`, sin `any` y sin `@ts-ignore`. **Verificación:** los dos comandos.
- [ ] 3.5 Auditoría con `rust-reviewer` y `react-reviewer` y remediación de los hallazgos ALTOS y MEDIOS. **Verificación:** veredictos y cierre de hallazgos pegados en la tarea.
- [ ] 3.6 Revisar la coherencia de la fila irresoluble (operador desconocido, patrón incoherente con el operador) y la documentación interna de las decisiones: elemento vacío de `@within`, `@unconditionalMatch` con la colección ausente y evidencia solo con pipeline no vacía. **Verificación:** lectura del código y clippy sin avisos nuevos.
- [ ] 3.7 Suite completa verde tras el refactor. **Verificación:** agregado de `cargo test --no-fail-fast` y `npx vitest run`.

## 4. Documentación

- [ ] 4.1 `AGENTS.md`: las ocho variantes de `Operator` con sus nombres canónicos, el patrón como dato del operador, la semántica de los seis operadores de cadena, la validación por operador, el round-trip y el descarte con diagnóstico, y el selector del editor. **Verificación:** la sección describe el comportamiento implementado.
- [ ] 4.2 `USER-GUIDE.md`: el operador como elección de la autoría avanzada, con la semántica de cada uno y el ejemplo de `@within` frente a `@contains`. **Verificación:** el documento describe lo entregado.
- [ ] 4.3 `WAF-ROADMAP.md`: marcar F2c-1 entregada y ajustar el desglose de F2c (lo que queda en F2c-2 y F2c-3). **Verificación:** el nodo F2c y su desglose quedan al día.

## 5. Verificación final y archivo

- [ ] 5.1 Suite backend completa. **Verificación:** agregado de `cargo test --no-fail-fast`, con el conteo pegado.
- [ ] 5.2 Frontend verde. **Verificación:** `npx vitest run` y `npx tsc -b`.
- [ ] 5.3 Clippy lib+bins en 0, all-targets en la base y fmt en 0. **Verificación:** `cargo clippy --lib --bins -- -D warnings`, `cargo clippy --all-targets` y `cargo fmt --check`.
- [ ] 5.4 Cero cambio observable en modo simple: la equivalencia sobre las 42 plantillas sigue verde, ninguna fila proyectada lleva un operador de cadena y la forma del evento de auditoría no cambia. **Verificación:** los tests de equivalencia de F2b-3 y el de la proyección simple pasan.
- [ ] 5.5 Rollback: una fila con un operador que el matcheo no conoce se descarta con diagnóstico y no se aplica mal. **Verificación:** el test del descarte con operador desconocido pasa.
- [ ] 5.6 Archivar el change con `openspec archive waf-string-operators --yes` y comprobar que las dos specs destino reciben sus requisitos. **Verificación:** `openspec list` sin changes activos y los requisitos presentes en `openspec/specs/rule-matching/spec.md` y `openspec/specs/rules/spec.md`.

# Proposal: is-tor-collection

## Why

El matcheo sabe recorrer un conjunto ordenado de condiciones (F2a), sabe persistirlo en `rule_conditions` (F2b-1) y su vocabulario incluye ya las colecciones nuevas (F2b-2a) con su editor (F2b-2b). Pero `IS_TOR` sigue siendo **la última cosa que el motor no sabe expresar como condición**: un predicado booleano cableado al margen del conjunto (`check_tor` en `CacheRule::evaluate()`, combinado al final de la conjunción). Con `Operator` reducido a `@rx` no había forma de comparar un booleano que no fuese inventarle una representación de cadena, que sería andamiaje muerto. Por eso F2a lo dejó fuera y la reentrada se escalonó (`WAF-ROADMAP.md` § «`is_tor`: el caso especial que entra en esta fase»).

**F2b-3** cierra ese peldaño: `IS_TOR` entra en el vocabulario como **colección booleana**, su operador booleano `@truthy` la compara, la columna `is_tor` de `rules` la proyecta, el matcheo la evalúa como una condición más y el editor la ofrece. No es andamiaje: entra en funcionamiento y sustituye al predicado `check_tor`.

**No hay cambio observable en modo simple.** El comportamiento del filtro Tor es **equivalente** al de hoy —las 42 plantillas, incluida «Tor exit nodes», deciden igual—, la forma del evento de auditoría no cambia (no se anota evidencia para `is_tor`) y el modo simple se comporta byte a byte como antes. En **modo avanzado**, en cambio, la columna `is_tor` deja de participar fuera del conjunto —el conjunto se evalúa como exactamente la lista persistida—, conforme al requisito de `rule-matching`.

## What Changes

1. **`IS_TOR` como colección booleana.** El vocabulario de colecciones gana `is_tor`, con nombre persistido `is_tor`, sin clave, y resolución desde el booleano `is_tor` de la petición (`Option<bool>`), no desde `request_value`. `Collection::ALL` sigue siendo las **14** de campo: `is_tor` no se proyecta desde ellas.

2. **Operador booleano `@truthy`.** `Operator` gana una variante booleana además de `Rx(Regex)`, serializada como `@truthy`. La condición se cumple **si y solo si** el booleano de la petición es exactamente `true`; `Some(false)` y `None` no cumplen. Se descarta `@streq true` sobre una cadena `"true"`/`"false"`: inventar esa representación de cadena es lo que la fase entera evita. No se introduce `@bool false` ni negación: «sólo Tor» (no-Tor) es F2d (`negate`).

3. **Proyección desde la columna `is_tor` y retirada de `check_tor`.** `project()` añade al final del conjunto una condición `is_tor`/`@truthy` cuando `is_tor == Some(true)`; `Some(false)` y `None` no proyectan nada (neutrales). `CacheRule::evaluate` **retira** el predicado `check_tor` y el campo `CacheRule.is_tor`, en cascada: el chequeo de Tor pasa a ser una condición más, evaluada en AND con cortocircuito.

4. **Round-trip por `rule_conditions`.** La columna `operator` deja de estar fijada a `@rx`: se escribe `@rx` (con su patrón) o `@truthy` (con patrón vacío), y la lectura la reconstruye. Una fila incoherente se **descarta** con un diagnóstico que la nombra.

5. **Validación de la autoría avanzada.** `ConditionInput::compile` acepta `@truthy` **únicamente** para `is_tor` (y exige patrón vacío, sin clave, sin transformaciones); para el resto de colecciones el operador es ausente o `@rx`. `@truthy` fuera de `is_tor` → `400`; `@rx` sobre `is_tor` → `400`.

6. **Migración de backfill.** Una nueva migración inserta en `rule_conditions` la fila `is_tor`/`@truthy` de las reglas con `is_tor = 1` ya almacenadas. Es imprescindible: `rule_conditions` es la fuente **única** del matcheo, así que sin ella esas reglas perderían su condición Tor al arrancar.

7. **Frontend.** `is_tor` entra en el catálogo de colecciones; el editor ofrece, para `is_tor`, el operador fijo `@truthy` y no patrón, ni clave, ni transformaciones. `projectConditions` —que siembra y sirve a la inferencia de modo— pasa a incluir la condición de `is_tor` cuando `is_tor === true`, para que la inferencia **no** degrade a avanzado una regla simple con `is_tor`.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `rules`: `is_tor` como colección booleana en el vocabulario, el operador `@truthy`, la proyección desde la columna `is_tor`, el round-trip del operador en `rule_conditions`, la validación de la autoría avanzada y el editor con `@truthy`.
- `rule-matching`: la colección booleana `is_tor` como condición del conjunto, su evaluación (`@truthy`) y su proyección desde la columna.

## Impact

- **Backend:** el vocabulario de colecciones (variante `is_tor` y su resolución booleana); el operador (`Operator` gana la variante booleana); la proyección (`project()` incluye la condición de `is_tor`); el matcheo (`evaluate()` retira `check_tor` y evalúa la colección); la persistencia (`rule_conditions` escribe y lee el operador real); y la validación de la autoría avanzada.
- **Base de datos:** **una migración** — `backend/migrations/20261026000000_project_is_tor_into_rule_conditions.up.sql`, backfill idempotente (`WHERE NOT EXISTS`) de la fila `is_tor`/`@truthy` para las reglas con `is_tor = 1`.
- **API:** superficies **aditivas**: `is_tor` entra en el vocabulario de `conditions` y su operador `@truthy` en la validación. El modo simple no cambia.
- **Frontend:** el catálogo de colecciones, el editor de condiciones y la proyección de siembra/inferencia de modo.
- **Cambio observable:** **ninguno en modo simple** —ni en el evento (no se anota evidencia) ni en la decisión de las 42 plantillas—. En **modo avanzado** la columna `is_tor` deja de participar fuera del conjunto, conforme al requisito de `rule-matching`: una regla avanzada con la columna `is_tor = 1` y sin condición `is_tor` en su lista ya **no** restringe el tráfico a Tor.

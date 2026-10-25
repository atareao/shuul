# Design: is-tor-collection

## Contexto

Ver `proposal.md — Why`. F2a dejó el matcheo sobre un conjunto ordenado de condiciones; F2b-1 hizo de `rule_conditions` la fuente persistida; F2b-2a amplió el vocabulario con las colecciones con clave; F2b-2b entregó el editor. `IS_TOR` sigue siendo el predicado booleano `check_tor` en `CacheRule::evaluate()`, combinado al final de la conjunción y fuera del conjunto de condiciones.

Dos restricciones del sistema condicionan el diseño:

1. **El modo simple no puede cambiar.** Las 42 plantillas y las reglas guardadas con los 14 campos y `is_tor` deben decidir byte a byte igual que hoy.
2. **`rule_conditions` es la fuente única del matcheo.** Lo que no esté en esa tabla no existe al arrancar: una regla con `is_tor = 1` ya almacenada perdería su condición Tor si la carga leyera solo de la tabla y el backfill no existiera.

## Objetivos y no-objetivos

**Objetivos:**

- `is_tor` como colección booleana del vocabulario, con resolución desde `Option<bool>`.
- Operador booleano `@truthy` que compara el booleano.
- Proyección desde la columna `is_tor` y retirada de `check_tor`.
- Round-trip del operador en `rule_conditions`.
- Validación de la autoría avanzada para `@truthy`.
- Editor con `@truthy` y la inferencia de modo correcta.

**No-objetivos:**

- `@bool false` y la negación por condición → **F2d** (`negate`). «Sólo Tor» no se expresa en esta sub-entrega.
- El resto de operadores (`@pm`, `@ipMatch`, los de cadena y validación) → **F2c**.
- La lógica booleana `condition_operator` (`and`/`or`) → **F2d**.
- Anotar `is_tor` en la evidencia → descartado explícitamente aquí (D5).

## Decisiones

### D1. `Collection::IsTor`, la única colección booleana

El vocabulario gana la variante `IsTor`: `field_name() == "is_tor"`, `requires_key() == false`, `from_field_name("is_tor") == Some(IsTor)`. NO está en `Collection::ALL`, que siguen siendo **las 14** de campo —es la lista de la representación simple y del orden histórico, no el catálogo completo—. Es la única colección que no se resuelve por `request_value` (que devuelve texto): se resuelve desde `NewRequest.is_tor` (`Option<bool>`).

**Alternativa considerada:** una colección de texto `is_tor` con valores `"true"`/`"false"`. Se descarta: obliga a inventar una representación de cadena que no existe en la petición y que solo serviría para comparar contra `@streq true`, exactamente el andamiaje muerto que la fase entera evita.

### D2. El operador `@truthy`

`Operator` gana una variante booleana además de `Rx(Regex)`, serializada como `@truthy`. La condición se cumple **si y solo si** el booleano de la petición es exactamente `true`; `Some(false)` y `None` no cumplen.

**Alternativa considerada:** `@streq true` sobre la cadena `"true"`. Se descarta por lo mismo que D1. **No se introduce** `@bool false` ni negación: expresar «sólo Tor» (no-Tor) es F2d (`negate`), y añadir ahora un operador de dos direcciones sería alcance fuera de esta sub-entrega.

### D3. La proyección añade `is_tor` al final

`project()` conserva la proyección de los 14 campos en su orden canónico y, si la regla tiene `is_tor == Some(true)`, añade **al final** una condición `{ collection: IsTor, key: None, pipeline: vacía, operator: Truthy }`. `is_tor == Some(false)` e `is_tor == None` proyectan **ninguna** condición: son neutrales, como la clausura histórica, que solo decide con `Some(true)`.

Esto replica exactamente el predicado histórico que se combinaba **al final** de la conjunción, de modo que la condición resultante decide lo mismo y en el mismo punto del cortocircuito.

### D4. `evaluate` retira `check_tor`

El chequeo de Tor deja de ser un predicado aparte: pasa a ser una condición más del conjunto, evaluada en AND con cortocircuito como las demás. `CacheRule` pierde el campo `is_tor` —redundante en cuanto la columna proyecta la condición—, y con él `check_tor`, en cascada. Es la forma natural de la arquitectura del motor: un único camino de matcheo, sin predicados al margen.

### D5. La colección booleana no anota evidencia

Hoy `check_tor` **no** anota nada en el evento de auditoría. Convertirlo en condición abre la puerta a un `{"is_tor": "true"}` que hoy no existe: **sería un cambio observable**. Se decide explícitamente **no anotar evidencia** para `is_tor`, de modo que la forma del evento no cambia y la decisión observable es idéntica a la de hoy. Es una decisión **revisable**: si una fase posterior quiere anotar el booleano, el cambio sería explícito y visible, nunca por el arrastre de la generalización.

### D6. La columna `operator` deja de estar fijada a `@rx`

La escritura persiste `@rx` (con su patrón) o `@truthy` (con patrón vacío). La lectura lee la columna `operator`, reconstruye `Rx`/`Truthy`, y **descarta** una fila incoherente con un diagnóstico que la nombra: `@truthy` en una colección de texto, `is_tor` con operador distinto de `@truthy`, `@truthy` con transformaciones no vacías, o clave en una colección sin clave. Descartar en vez de aplicar mal es el **fallo seguro** del rollback a una imagen anterior.

`From<&Condition> for ConditionInput` serializa `is_tor` como `{collection:"is_tor", key:null, transformations:"", operator:"@truthy", pattern:""}`.

### D7. La validación de `@truthy` vive en el compilador de condiciones

`ConditionInput::compile` acepta `@truthy` **únicamente** para la colección `is_tor`, exigiendo `pattern` vacío, `key` ausente y `transformations` vacías; para el resto de colecciones el operador es ausente o `@rx`. `@truthy` fuera de `is_tor` → `400`; `@rx` sobre `is_tor` → `400`. Sigue siendo el único punto de validación de la autoría avanzada, invocado por las tres vías de escritura.

### D8. Migración de backfill imprescindible

`rule_conditions` es la fuente **única** del matcheo. Sin el backfill, las reglas con `is_tor = 1` ya almacenadas perderían su condición Tor al arrancar: la carga lee la tabla y el píxel de la columna `is_tor` no se reinterpreta en caliente. La migración inserta, para cada regla con `is_tor = 1`, su fila (`collection='is_tor'`, `operator='@truthy'`, `pattern=''`, `transformations=''`, `key=NULL`, `negate=0`) con `position` contigua tras las existentes. `WHERE NOT EXISTS` la hace **idempotente**.

**Alternativa considerada:** reinterpretar la columna `is_tor` al cargar en vez de migrar. Se descarta: reintroduciría en la carga una fuente de verdad paralela a la tabla, justo lo que F2b-1 eliminó, y dejaría dos caminos de matcheo divergentes.

### D9. La columna `is_tor` de `rules` se conserva como representación simple

Igual que los 14 campos, la columna `is_tor` sigue almacenándose, sigue siendo entrada/salida de la API y del export/import, y se proyecta a la tabla al escribir. No se retira: es la representación que la interfaz simple maneja hoy.

### D10. El frontend ofrece `@truthy` y no degrada el modo

`is_tor` entra en el catálogo de colecciones; el editor ofrece, para `is_tor`, el operador fijo `@truthy` y **no** patrón, ni clave, ni transformaciones. `projectConditions` —que siembra la lista y sirve a la **inferencia de modo**— pasa a incluir la condición de `is_tor` cuando `is_tor === true`, de modo que una regla simple con `is_tor` **no** se degrade a avanzado al abrir su diálogo. `Condition.operator` pasa a `"@rx" | "@truthy"`.

## Riesgos y compromisos

- **El caso `is_tor = Some(false)` que NO debe invertir el sentido.** Proyectar el `false` a una condición negaría todo el tráfico que no venga de Tor. → Se cubre con tests explícitos del caso `Some(false)` (no proyecta) y `None` (no proyecta), además del `Some(true)`.
- **La coherencia de la fila irresoluble.** Una fila con operador incoherente para su colección no debe aplicarse mal. → Se descarta con diagnóstico en la lectura (D6).
- **El coste.** La columna `is_tor` se sigue escribiendo y proyectando; la resolución booleana es trivial frente a un regex. → Sin coste medible.
- **Rollback.** Volver a una imagen anterior deja filas `@truthy` que esa versión no conoce. → Las descarta con diagnóstico (D6), fallo seguro. La columna `is_tor` sigue ahí, así que una versión anterior seguiría aplicando `check_tor` y la regla no se pierde.

## Migración

**Una migración:** `backend/migrations/20261026000000_project_is_tor_into_rule_conditions.up.sql`.

Para cada regla con `is_tor = 1` inserta en `rule_conditions` su fila `is_tor`/`@truthy` (`pattern=''`, `transformations=''`, `key=NULL`, `negate=0`), con la `position` contigua tras las filas existentes de esa regla. `WHERE NOT EXISTS` la hace idempotente. Es imprescindible porque la tabla es la fuente única del matcheo (D8). El rollback se hace revirtiendo la imagen.

## Open Questions

Ninguna que afecte a las specs ni al plan.

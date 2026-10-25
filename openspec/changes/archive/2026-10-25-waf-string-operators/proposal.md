# Proposal: waf-string-operators

## Why

El motor sabe recorrer un conjunto ordenado de condiciones (F2a), persistirlo en `rule_conditions` (F2b-1), resolver 20 colecciones (F2b-2a y F2b-3) y compararlas con **dos** operadores: el regex histórico `@rx` y el booleano `@truthy`. Coraza tiene alrededor de veinticinco; Shuul usa dos. Toda comparación que no sea un regex —«¿el `User-Agent` contiene `nikto`?», «¿el método es exactamente `POST`?», «¿la extensión termina en `.exe`?»— obliga hoy a escribir un regex: más lento de evaluar, más difícil de leer y más fácil de equivocar.

**F2c-1** abre el vocabulario de operadores con los **seis operadores de cadena** del catálogo de Coraza —`@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`— y con el **selector de operador** del editor. Con ellos el patrón deja de ser un regex obligatorio y pasa a ser un dato del operador: la comparación es una comparación de `str`, sin compilación en el camino caliente.

**Cero cambio observable en modo simple.** El modo simple sigue proyectando **solo** `@rx` —`project()` compila siempre `Operator::Rx`—, así que las 42 plantillas y las reglas ya guardadas deciden byte a byte como hoy y la forma del evento de auditoría no cambia. Los operadores nuevos solo son alcanzables por la **autoría avanzada**. **No hay migración**: la columna `operator` de `rule_conditions` ya existe, admite texto libre y su `DEFAULT` es `@rx`.

## What Changes

1. **El patrón vive en el operador.** Hoy el patrón está dentro de `Operator::Rx(Regex)`. Con operadores heterogéneos, cada variante guarda lo que necesita y `Operator` expone su **patrón canónico** (`pattern() -> Cow<'_, str>`), de modo que `From<&Condition>`, la persistencia y la evidencia dejan de tener un `match` sobre dos variantes. `Condition` **no** gana un campo `pattern` y `project()` **no** cambia de forma: la IR se mantiene estable y el cambio queda acotado.

2. **Seis operadores de cadena**, con la semántica de Coraza y aplicados al valor **ya transformado** por la pipeline:

   | Operador | Semántica |
   |---|---|
   | `@contains` | el valor **contiene** el patrón (sensible a mayúsculas) |
   | `@streq` | el valor es **exactamente igual** al patrón (sensible a mayúsculas) |
   | `@beginsWith` | el valor **empieza por** el patrón |
   | `@endsWith` | el valor **termina por** el patrón |
   | `@within` | el **valor** (aguja) está contenido en alguno de los elementos del **patrón** (lista separada por comas, con espacios recortados); es el inverso de `@contains` |
   | `@unconditionalMatch` | **siempre** casa; exige **patrón vacío** |

3. **Nombres canónicos.** Los de Coraza, con su grafía exacta: `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within`, `@unconditionalMatch`. `Operator::name()` pasa a cubrir las **ocho** variantes (los seis nuevos más `@rx` y `@truthy`) y es la representación que la escritura persiste y la lectura reconoce.

4. **Validación en un solo punto.** `ConditionInput::compile` sigue siendo el **único** punto de validación de la autoría avanzada —lo invocan las tres vías de escritura— y pasa a exigir **patrón no vacío** a los operadores de cadena y **patrón vacío** a `@unconditionalMatch`. `@truthy` sigue siendo exclusivo de `is_tor`, `is_tor` sigue exigiendo `@truthy`, `@rx` sigue compilando el regex y un operador desconocido sigue siendo `400 Bad Request`.

5. **Transformaciones.** Los operadores de cadena **sí** admiten pipeline, con el contrato ya vigente: la pipeline se aplica al **valor de la petición** —nunca al patrón, que se usa tal como se compiló— antes de comparar.

6. **Persistencia y fallo seguro.** `write_conditions` persiste el nombre canónico y el patrón canónico del operador; `condition_from_row` reconstruye el operador desde el nombre y el patrón y **descarta con diagnóstico** —nombrando la fila— una fila con operador desconocido, patrón que no compile o combinación incoherente. Es el fallo seguro ya establecido para el rollback a una imagen anterior, que no conoce los operadores nuevos.

7. **Evidencia.** Los operadores de cadena anotan evidencia con el **mismo contrato** que `@rx`: solo cuando la condición casa **y** su pipeline no está vacía, bajo la clave de la colección —o la clave compuesta `colección[clave]`—. `@unconditionalMatch` sigue ese mismo contrato. No se inventa forma nueva para el evento de auditoría.

8. **Frontend.** El editor de condiciones gana un **selector de operador** por fila, con un catálogo de siete (`@rx` por defecto más los seis nuevos). `is_tor` conserva el operador **fijo `@truthy`** y sin selector. `@unconditionalMatch` **no** exige patrón: el campo se oculta y el patrón viaja vacío. La **inferencia de modo simple/avanzado no cambia**: sigue comparando las condiciones recibidas con la proyección de los 14 campos, que solo produce `@rx`. La validación de fila impide guardar con patrón vacío salvo en `@unconditionalMatch`.

9. **Rendimiento.** Los comparadores se construyen **una vez**, al cargar o validar la regla, no por petición: los operadores de cadena son comparaciones de `str` sin compilación en el camino caliente.

**Fuera de alcance (sub-entregas siguientes del roadmap):** `@pm`, `@ipMatch` y `@ipMatchF` (**F2c-2**); `@gt`, `@ge`, `@lt`, `@le` y los validadores `@validateByteRange`, `@validateUrlEncoding` y `@validateUtf8Encoding` (**F2c-3**); `@detectSQLi` y `@detectXSS` (**F5**); y la **negación** por condición (`negate`) y la lógica booleana `condition_operator` (**F2d**).

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `rules`: el vocabulario de operadores en la columna `operator` de `rule_conditions` y su round-trip, la validación de la autoría avanzada por operador y el selector de operador del diálogo de regla.
- `rule-matching`: el contrato de la condición (el operador como sujeto de la decisión, con su patrón) y la evaluación de los operadores de cadena.

## Impact

- **Backend:** el vocabulario de operadores (`Operator` gana seis variantes y su patrón canónico); el matcheo (`condition_matches` decide por operador); la validación (`ConditionInput::compile`); la persistencia (`write_conditions` y `condition_from_row`); y la representación (`From<&Condition> for ConditionInput`, que pasa a leer el patrón del operador).
- **Base de datos:** **ninguna migración**. La columna `operator` de `rule_conditions` ya existe, admite texto libre y su `DEFAULT` es `@rx`; los seis nombres nuevos caben en ella sin cambio de esquema. El rollback es revertir la imagen, y las filas con operadores nuevos se descartan con diagnóstico (fallo seguro).
- **API:** superficies **aditivas**: `conditions[].operator` admite seis valores más y `conditions[].pattern` deja de ser obligatorio para `@unconditionalMatch`. El modo simple no cambia.
- **Frontend:** el tipo del operador, el catálogo de operadores, el selector por fila del editor y la validación de fila por operador.
- **Cambio observable:** **ninguno en modo simple** —`project()` sigue compilando `Operator::Rx`, así que las 42 plantillas deciden igual y la forma del evento no cambia—. En **modo avanzado** la única diferencia es la que ya establecía F2b-1: el conjunto persistido es exactamente la lista dada.
- **Plantillas:** ninguna se toca. Las 42 siguen describiéndose con los 14 campos y la columna `is_tor`, que se proyectan a `@rx` y `@truthy`.

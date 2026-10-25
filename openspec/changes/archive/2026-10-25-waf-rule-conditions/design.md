# Design: waf-rule-conditions

## Contexto

F2a dejó el matcheo definido sobre un `Vec<Condition>`, pero la única fuente de esas
condiciones son las 14 columnas de `rules`, proyectadas en memoria en cada carga
(`CacheRule::read_all_active` → `condition::project`). Esa forma no admite una
colección que no sea uno de los 14 campos: el almacén es el techo.

F2b-1 introduce el almacén (la tabla `rule_conditions`) sin cambiar el vocabulario,
igual que F2a introdujo la representación sin cambiar el comportamiento. El
vocabulario nuevo (F2b-2) y `IS_TOR` (F2b-3) llegan después, sobre una base probada.

## Objetivos y no-objetivos

**Objetivos**

- Crear la tabla `rule_conditions` y poblarla desde las 14 columnas (backfill y
  escritura).
- Alimentar el matcheo desde la tabla, con un único camino de datos.
- Cero cambio observable: mismo `matched`, misma evidencia, misma oferta de API.

**No-objetivos (caen en sub-entregas posteriores)**

- Colecciones nuevas y su extracción del request → **F2b-2**.
- `IS_TOR` como condición → **F2b-3**.
- Operadores nuevos → **F2c**.
- Lógica booleana (`condition_operator`, `negate`) → **F2d**.

## Decisiones

### 1. La tabla es la fuente del matcheo, no un espejo

Un `dual-write` que escriba la tabla pero siga leyendo las 14 columnas sería dos
fuentes de verdad que pueden divergir en silencio. La tabla es la **única** fuente
del conjunto de condiciones; las 14 columnas se conservan como representación
«simple» de la API y de export/import, y se proyectan a la tabla en cada escritura.
Un solo camino de datos hacia `Vec<Condition>`.

### 2. El esquema nace completo; el contrato se llena por partes

`key`, `operator` y `negate` existen en la tabla desde el principio para no volver a
migrar cuando lleguen F2b-2 y F2c/F2d. En F2b-1 son constantes: `key = NULL`,
`operator = '@rx'`, `negate = 0`. Declarar el contrato completo antes de
implementarlo sería andamiaje; declarar el **esquema** completo y estrenar cada
columna cuando le toca no lo es.

### 3. `position` es la posición en el conjunto, contigua y 0-based

El conjunto de una regla se numera `0..N-1` en el orden de `Collection::ALL`. El
backfill la calcula con `ROW_NUMBER() OVER (PARTITION BY rule_id ORDER BY <orden>)`.
`UNIQUE(rule_id, position)` impide dos condiciones en el mismo sitio.

### 4. La escritura reemplaza el conjunto

Al crear, editar o importar, se borran las filas previas de esa regla y se
reinsertan las nuevas. Es simple y correcto; el `id` de la fila no se usa en ningún
sitio. Un upsert por `position` sería más fino pero tendría que borrar además las
posiciones sobrantes, que es exactamente lo que el reemplazo ya hace.

### 5. `transformations` es por condición

Cada fila lleva su pipeline. En F2b-1 todas las filas de una regla llevan la
pipeline de la regla, porque es lo que `condition::project` ya clona en cada
condición. Persistirlo por condición deja el esquema listo para F2b-2 sin migrar.

### 6. Las 14 columnas se conservan

Siguen siendo la entrada y la salida de la API en modo simple y las protagonistas de
export/import. La tabla no las sustituye en el contrato público.

### 7. El parseo `Collection` ↔ `TEXT` es simétrico y total

La colección se persiste por el nombre que ya usa (`field_name()`); al leer, el
texto se parsea de vuelta. Un valor desconocido no debería ocurrir con el
vocabulario de F2b-1; si ocurre, la fila se descarta con un `debug!` que la nombra,
sin tumbar la carga de la regla entera.

### 8. `is_tor` sigue fuera del conjunto

No se persiste como condición. Es el predicado booleano que F2b-3 convertirá en
colección, con su operador (F2c) y su negación (F2d).

## Riesgos y compromisos

- **Equivalencia.** El único riesgo real es que la proyección desde la tabla no sea
  exacta. Se cubre con los tests de F2a (que siguen verdes) más un test de
  equivalencia tabla↔columnas sobre las 42 plantillas del catálogo.
- **Duplicación de datos.** Mientras la tabla no albergue condiciones que las
  columnas no expresen, lleva lo mismo que ellas. Es el precio de introducir el
  almacén sin cambiar el contrato, y desaparece en F2b-2.
- **Backfill en SQL.** La proyección 14 columnas → filas se hace con una ventana
  (`ROW_NUMBER`) sobre `UNION ALL` de los 14 campos; hay que probarla con reglas de
  cero, una y varias condiciones.

## Migración

Una migración `.up.sql`: `CREATE TABLE rule_conditions` (con `UNIQUE(rule_id,
position)` y `FOREIGN KEY ... ON DELETE CASCADE`) y el backfill desde las 14
columnas de `rules`. El patrón es el del repositorio: `.up.sql` comentado, sin
`.down.sql`.

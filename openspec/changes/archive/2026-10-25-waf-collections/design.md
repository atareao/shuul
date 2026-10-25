# Design: waf-collections

## Context

Ver `proposal.md — Why`. F2a dejó el matcheo sobre un conjunto ordenado de condiciones; F2b-1 hizo de la tabla de condiciones la fuente persistida, y su esquema ya nace con las columnas `key`, `operator` y `negate` (constantes en esa fase). Esta sub-entrega **estrena `key`**.

Dos restricciones del sistema condicionan el diseño:

1. **Dos caminos de extracción distintos.** El pipeline del WAF recibe el mapa de cabeceras completo por `forwardAuth`; el del **Jail** construye la petición a mano desde un payload de reporte que sólo trae path, query y unas pocas cabeceras —no cookies ni el resto de cabeceras—.
2. **El modo simple no puede cambiar.** Las 42 plantillas y las reglas guardadas con los 14 campos deben comportarse byte a byte igual que en F2b-1.

## Goals / Non-Goals

**Goals:**

- Extraer cookies, cabeceras, argumentos del query, basename y extensión.
- Ampliar el vocabulario de colecciones con esas cinco, con clave para las tres multi-valor.
- Persistir y leer la clave; evaluar las colecciones nuevas.
- Autoría avanzada por API: lista de condiciones con validación, y ciclo completo de representación.

**Non-Goals:**

- Interfaz del editor de condiciones → **F2b-2b**.
- `IS_TOR` como colección → **F2b-3**.
- Operadores nuevos (`@pm`, `@ipMatch`, los de cadena y validación) → **F2c**; la negación por condición y la lógica booleana → **F2d**. Por eso la lista de condiciones no acepta ni `negate` ni operadores distintos de `@rx` en esta sub-entrega.
- Las colecciones de **nombres** (`args_get_names`, `request_headers_names`, `request_cookies_names`): sólo tienen sentido con los operadores de conjunto de F2c.

## Decisions

### 1. La extracción vive en el modelo de la petición, no en el matcheo

Las colecciones nuevas se extraen una vez, al construir la petición, en vez de derivarlas en el momento de evaluar cada condición. Así la resolución del valor sigue devolviendo una referencia prestada, el coste de parseo no se paga una vez por condición, y queda el gancho natural para la memoización de la fase de rendimiento (F4).

**Alternativa considerada:** derivar en el matcheo bajo demanda. Se descarta: obliga a devolver un valor prestado o construido al vuelo (una `Cow`) y repite el parseo por cada condición que use la colección.

**Contra asumida:** el modelo de la petición crece y hay que poblar sus campos en sus dos constructores (el de la extracción del WAF y el literal del Jail).

### 2. Cinco colecciones, tres con clave

El vocabulario gana `request_cookies`, `request_headers` y `args_get` (con clave) y `request_basename`, `request_extension` (sin clave). Una función declara qué colecciones exigen clave. Los nombres persistidos van en minúsculas, coherentes con las 14 actuales. El conjunto del modo simple sigue siendo **el de las 14**: las nuevas nunca se proyectan desde los 14 campos.

**Alternativa considerada:** adoptar los nombres de colección en mayúsculas al estilo Coraza (`REQUEST_COOKIES`, `ARGS_GET`). Se descarta: rompería la coherencia con los nombres ya persistidos y con la clave de la evidencia.

### 3. La clave es un dato de la condición, no del vocabulario

La colección describe **qué** se mira y es un valor copiable; la clave es un dato de la condición. La condición gana una clave opcional y la resolución del valor la recibe. Una colección con clave sin ella devuelve ausencia: la validación de la escritura lo impide, pero el matcheo no confía en esa validación.

### 4. La evidencia de una condición con clave usa una clave compuesta

La evidencia es un mapa de nombre a valor. Dos condiciones sobre la misma colección con claves distintas colisionarían bajo el mismo nombre, así que las colecciones con clave anotan `colección[clave]`. Las 14 actuales y basename/extensión conservan su nombre. No es un cambio observable: las colecciones con clave son nuevas.

### 5. El modo lo decide la presencia de la lista de condiciones

No se añade una columna de «modo». En la escritura, la lista ausente o nula significa modo simple (proyección desde los 14 campos) y su presencia (aunque sea vacía) significa modo avanzado (la lista es el conjunto, y los 14 campos se almacenan tal como vengan, sin participar del matcheo). En la lectura, la representación siempre incluye las condiciones; el cliente reconstruye el modo. Evita una migración y una fuente de verdad extra.

**Alternativa considerada:** una columna que marque la representación (`simple`/`advanced`). Se descarta: es información derivable del propio conjunto y una columna más que mantener coherente.

### 6. La autoría avanzada valida en un solo punto

Un validador único convierte la lista de entrada en el conjunto compilado: colección conocida, clave obligatoria exactamente cuando la colección la exige, operador ausente o `@rx`, patrón compilable, transformaciones resolubles. Lo invocan las tres vías de escritura (crear, editar, importar). Un fallo devuelve un error de entrada (400) que nombra el motivo, igual que ya hacen la validación del pipeline y la de las transformaciones.

### 7. La respuesta incluye las condiciones; en el listado, por lote

La representación de una regla incluye sus condiciones, construidas desde la tabla. Para el listado y la exportación, las condiciones de todas las reglas se cargan en **una** consulta por lote, no una por regla. Añadir un campo a la respuesta es aditivo: los clientes actuales lo ignoran.

### 8. Las colecciones con clave son del WAF

El pipeline del WAF recibe el mapa de cabeceras completo; el del Jail, un payload de reporte sin cookies ni el resto de cabeceras. El WAF extrae todo; el Jail deriva de su payload lo que puede (argumentos del query, basename, extensión) y deja cookies y cabeceras ausentes —y por tanto esas condiciones no casan en el Jail, que es lo correcto: no hay dato que mirar—. Se documenta como limitación conocida.

## Risks / Trade-offs

- **El modelo de la petición crece y tiene dos constructores.** → Las colecciones derivables se pueblan en un helper compartido; los tests de caracterización de los 14 campos garantizan que nada existente cambia.
- **Guardar todas las cabeceras por petición tiene coste.** → Es el precio de que cualquier cabecera sea alcanzable; la fase de rendimiento (F4) es el sitio para optimizarlo. En el Jail no se guarda.
- **Divergencia entre modo simple y avanzado.** → El modo simple queda intacto por construcción (la proyección no cambia); el avanzado se prueba con el ciclo completo y con exportación/importación.
- **La importación de reglas avanzadas.** → El upsert por nombre reemplaza el conjunto; con condiciones presentes persiste la lista, sin ellas proyecta. Cubierto por tests.
- **Rollback.** → Volver a una imagen anterior deja la columna `key` con valores que esa versión no conoce: descarta esas condiciones con un diagnóstico en vez de aplicarlas mal. Es un fallo seguro.

## Migration Plan

**Ninguna migración.** La columna `key` de la tabla de condiciones ya existe desde F2b-1 con valor nulo por defecto; estrenarla no cambia el esquema. El rollback se hace revirtiendo la imagen.

## Open Questions

Ninguna que afecte a las specs ni al plan.

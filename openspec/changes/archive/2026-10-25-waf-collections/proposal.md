# Proposal: waf-collections

## Why

El matcheo sabe recorrer un conjunto de condiciones (F2a) y sabe persistirlo (F2b-1), pero el **vocabulario** sigue siendo el de los 14 campos planos (`ip_address` … `x_request_id`). No hay forma de expresar «la cookie `PHPSESSID` contiene X», «la cabecera `Authorization` es Y» ni «el parámetro `id` del query lleva SQLi»: el query se guarda como una cadena opaca y las cookies y el resto de cabeceras se descartan al extraer la petición. Es el segundo agujero medido del `WAF-ROADMAP.md` (§2.2): **se tiran colecciones que ya llegan por el cable**.

F2b-2 cruza ese techo y se subdivide. Esta sub-entrega —**F2b-2a**— hace el **backend**: el vocabulario nuevo, su extracción, su persistencia con `key` y la autoría avanzada por API. La interfaz del editor de condiciones llega en **F2b-2b**.

## What Changes

1. **Extracción de colecciones nuevas.** La extracción de la petición pasa a resolver, además de los campos actuales: las **cookies** (del encabezado `Cookie`), **todas las cabeceras** con su nombre, los **argumentos del query uno a uno**, el **basename** y la **extensión** del path.

2. **Vocabulario de colecciones.** El vocabulario de colecciones gana cinco entradas: `request_cookies`, `request_headers` y `args_get` (las tres **con clave**: el nombre de la cookie, de la cabecera o del parámetro) y `request_basename`, `request_extension` (sin clave).

3. **La IR y el matcheo estrenan `key`.** Cada condición gana una clave opcional; el matcheo resuelve el valor de la colección por esa clave y evalúa las colecciones nuevas igual que las 14. La evidencia de una condición con clave usa una clave **compuesta** (`request_cookies[PHPSESSID]`) para no colisionar entre condiciones de la misma colección.

4. **La persistencia estrena la columna `key`** que F2b-1 dejó a nulo: se escribe y se lee.

5. **Autoría avanzada por API.** La creación y la edición aceptan una **lista de condiciones** (`conditions`). Ausente o nula = **modo simple** (las 14 columnas, comportamiento intacto). Presente = **modo avanzado**: las condiciones se validan (colección conocida, clave obligatoria exactamente cuando la colección la exige, operador `@rx`, patrón compilable, transformaciones resolubles) y son el conjunto persistido.

6. **Round-trip.** La representación de una regla devuelta por la API (crear, editar, leer, listar y exportar) incluye sus `conditions`, para que el cliente pueda reconstruir y reenviar una regla avanzada. La importación persiste las condiciones de cada entrada cuando vienen.

**No hay cambio observable en modo simple.** Una regla escrita con las 14 columnas se comporta byte a byte igual que en F2b-1.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `request-extraction`: se añade la extracción de las colecciones nuevas (cookies, cabeceras, argumentos del query, basename y extensión).
- `rule-matching`: se añade la evaluación de las colecciones nuevas con clave y su evidencia compuesta.
- `rules`: se añade la persistencia de la clave, la autoría avanzada por lista de condiciones y su exposición en la representación de la regla.

## Impact

- **Backend:** la extracción de la petición (colecciones nuevas); el vocabulario de colecciones (cinco variantes nuevas, la clave y la resolución del valor); el matcheo (clave y evidencia compuesta); la persistencia (columna `key`); los modelos de escritura y de respuesta de la regla; y la importación.
- **Base de datos:** **ninguna migración** — la columna `key` ya existe en la tabla de condiciones desde F2b-1.
- **API:** superficies **aditivas**: un campo `conditions` en el cuerpo de escritura y en la representación de la regla. El modo simple no cambia.
- **Frontend:** ninguno en esta sub-entrega (llega en F2b-2b).

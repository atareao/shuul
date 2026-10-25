# Proposal: waf-rule-conditions

## Why

El matcheo ya sabe recorrer un conjunto de condiciones (F2a), pero **no sabe de
dónde sacarlas**: la única fuente son las 14 columnas de `rules`, proyectadas en
memoria en cada carga de la caché. Esa forma fija un techo nuevo: solo se puede
expresar «un regex por campo». Una cookie, una cabecera con nombre arbitrario o un
argumento del query no caben en ninguna de las 14 columnas, así que tampoco caben
en el almacén.

La fase **F2b** cruza ese techo con la tabla `rule_conditions`. Cruzarlo de una vez
mezclaría la **persistencia** con el **vocabulario nuevo** (colecciones) y con
`IS_TOR`, y un fallo en cualquiera de las tres partes sería indistinguible de las
otras. Esta sub-entrega —**F2b-1**— hace **solo la persistencia**: introduce la
tabla, la puebla desde las 14 columnas y hace que el matcheo se alimente de ella,
con el mismo vocabulario y el mismo comportamiento.

## What Changes

1. **Una tabla `rule_conditions`.** El conjunto ordenado de condiciones de una
   regla se persiste como filas: `(rule_id, position, collection, key,
   transformations, operator, pattern, negate)`, con `UNIQUE(rule_id, position)` y
   borrado en cascada.

2. **Un backfill desde los 14 campos.** La migración puebla la tabla desde las
   columnas actuales, de modo que ninguna regla existente se queda sin condiciones.

3. **La tabla pasa a ser la fuente del matcheo.** Al crear, editar o importar una
   regla, las 14 columnas se proyectan a filas de `rule_conditions`; la carga de la
   caché lee el conjunto desde la tabla. Un **único** camino de datos hacia
   `Vec<Condition>`.

4. **Las 14 columnas se conservan.** Siguen siendo la entrada y la salida de la API
   y de export/import. No se retiran.

**No es un cambio observable.** Misma decisión, misma evidencia, misma oferta de
API, misma forma del evento. La equivalencia la garantiza la proyección que F2a ya
demostró total.

## Capabilities

**Modified Capabilities**

- `rules` — se añaden tres requisitos: la persistencia de las condiciones en
  `rule_conditions`, la equivalencia de la proyección desde la tabla con la de los
  14 campos, y la conservación de las 14 columnas como representación simple.

**New Capabilities**

- Ninguna. La tabla es el almacén de una representación que ya existe.

## Impact

- **Backend**: `backend/src/models/rule.rs` (`create`, `update`, `import_handler` en
  `backend/src/http/rule.rs`, `read_all_active`, `delete`) y el helper que persiste y
  lee el conjunto de condiciones.
- **Base de datos**: **una migración** — creación de `rule_conditions` más el
  backfill.
- **API**: ninguna. El modo simple no cambia.
- **Frontend**: ninguna.
- **Docs**: `AGENTS.md` (la sección que describe el esquema y la persistencia) y
  `WAF-ROADMAP.md` (marcar F2b-1 como entregada, al cerrar).
- **Fuera de alcance**: las colecciones nuevas y su extracción (F2b-2), `IS_TOR`
  (F2b-3), los operadores nuevos (F2c), la lógica booleana `and`/`or`/`not` (F2d),
  la puntuación por anomalía (F3) y el cuerpo de la petición (F5).

## Decisions taken when opening the change

- **La tabla es la fuente del matcheo, no un espejo.** Un `dual-write` que escriba
  la tabla pero siga leyendo las 14 columnas dejaría **dos** fuentes que pueden
  desincronizarse sin que nada lo note. Con la tabla como fuente, el matcheo tiene
  **un solo** camino de datos y F2b-2 es puramente aditivo: el vocabulario nuevo
  entra como filas nuevas sobre una base ya probada.
- **El modo simple se conserva tal cual.** La API sigue recibiendo y devolviendo los
  14 campos; el backend los proyecta a filas. Nada cambia para el cliente.
- **Las 14 columnas no se retiran.** Siguen siendo la representación «simple» y la
  de export/import. Retirarlas sería un cambio observable que esta sub-entrega no
  hace.
- **El esquema nace completo; el contrato se llena por partes.** Las columnas `key`,
  `operator` y `negate` existen desde el principio para no volver a migrar, pero en
  F2b-1 son constantes (`NULL`, `@rx`, `0`): su contrato llega con F2b-2 (`key`) y
  con F2c/F2d (operador y `negate`).
- **`transformations` es por condición.** Cada fila lleva su pipeline; en F2b-1
  todas las filas de una regla llevan la pipeline de la regla, que es lo que
  `project()` ya clona en cada condición.
- **`is_tor` sigue fuera del conjunto.** No se persiste como condición; es F2b-3.

## Límites que se aceptan

- **Sin vocabulario nuevo.** F2b-1 no añade ni una colección: las filas solo llevan
  las 14. Su valor es estructural: deja la persistencia probada para que F2b-2 sea
  aditivo.
- **Una escritura más por regla.** Cada creación, edición o importación escribe las
  14 columnas **y** proyecta sus filas. El coste es despreciable.
- **El backfill duplica datos** (columna y fila) mientras la segunda representación
  no aporte nada que la primera no exprese. Es el precio de introducir el almacén
  sin cambiar el contrato; deja de ser duplicación pura en F2b-2, cuando la tabla
  albergue condiciones que las columnas no pueden expresar.

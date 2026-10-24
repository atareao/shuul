# Proposal: pagination-defaults

## Why

Tres requisitos del usuario sobre el tamaño de página de las tablas de administración, pedidos
a la vez porque son el mismo contrato aplicado a sitios distintos:

1. **La biblioteca de plantillas no está paginada.** `templates_page.tsx:609` usa
   `pagination={false}`, así que las 42 plantillas se renderizan en una sola lista. Se pide que
   pagine a 10 por página.
2. **El Log Viewer arranca en 50.** `logs_page.tsx:457-458` usa `pageSize: 50` y los rangos
   `20/50/100`, distintos de los del resto.
3. **Todas las tablas deben arrancar en 10 y ofrecer los rangos 10, 50 y 100.**

**Estado medido** (`pageSize` en todo el frontend; sólo hay tres sitios):

| Sitio | Hoy |
|---|---|
| `custom_table.tsx` — reglas, baneos, perfiles de rate limit | 10 por defecto y rangos `10/50/100` — **ya cumple** |
| `logs_page.tsx:457-458` | **50** por defecto y rangos `20/50/100` |
| `templates_page.tsx:609` | `pagination={false}`: **sin paginar** |

**Nota sobre los changes sin archivar.** El requisito `Client-side tables are paginated by the
client only` del módulo `table-pagination` (`admin-ui-fixes`, aún sin archivar) describe al Log
Viewer con un tamaño de página de 50 en la premisa de un escenario. **No se toca**: con los
rangos `10/50/100` un tamaño de 50 sigue siendo un estado alcanzable, así que esa premisa
continúa siendo válida y este change sólo **añade** requisitos, sin modificar ninguno de los
pendientes.

## What Changes

- **Biblioteca de plantillas** (`templates_page.tsx`): el listado pasa a paginar **en cliente**
  —no es `CustomTable`, y no puede serlo: el endpoint sirve el catálogo completo y el filtrado
  ya es local— con 10 filas por página y los rangos `10/50/100`. Al cambiar un filtro o la
  búsqueda, la paginación SHALL **volver a la primera página**, que es lo que ya hace
  `CustomTable` cuando cambian sus filtros: si no, el usuario puede quedarse en una página que
  el conjunto filtrado ya no alcanza y ver la tabla vacía.
- **Log Viewer** (`logs_page.tsx`): el tamaño de página por defecto pasa de 50 a **10** y los
  rangos de `20/50/100` a **`10/50/100`**.
- **El resto de tablas no se toca**: `CustomTable` ya cumple las tres condiciones.

## Non-goals

- **No se persiste el tamaño de página del Log Viewer.** El usuario pide «por defecto 10»; el
  estado persistido del Log Viewer sigue siendo `{hiddenEvents, autoRefresh}`. Es coherente con
  `CustomTable`, cuyo tamaño de página tampoco se persiste y se reinicia al desmontar.
- **No se pagina en servidor el endpoint de plantillas.** El catálogo son 42 filas que ya viajan
  completas y cuyo filtrado es local; darle paginación en servidor es otro cambio, con su propia
  API y su propio contrato.
- No se cambia `CustomTable` ni su contrato de paginación, tal como quedaron en `admin-ui-fixes`
  y `ban-row-identity`.
- No se toca el backend, ni el pipeline WAF/Jail, ni las reglas, ni los perfiles.

## Impact

- `frontend/src/pages/admin/templates_page.tsx` — paginación en cliente (10 por defecto, rangos
  10/50/100, vuelta a la primera página al filtrar)
- `frontend/src/pages/admin/logs_page.tsx` — 10 por defecto y rangos 10/50/100
- `frontend/src/pages/admin/templates_page.test.tsx` — RED de la paginación del listado
- `frontend/src/pages/admin/logs_page.test.tsx` — RED del tamaño de página por defecto y sus rangos
- Spec `table-pagination` — requisito nuevo sobre el valor por defecto y los rangos
- Sin cambios en backend, migraciones, base de datos ni infraestructura

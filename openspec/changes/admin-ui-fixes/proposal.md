# Proposal: admin-ui-fixes

## Why

Tres defectos independientes de la UI de administración, agrupados porque ninguno
justifica un change propio y los tres se verifican con la misma batería de tests de
frontend.

**1. La paginación de las tablas es incorrecta y afecta a todas las vistas.**
`CustomTable` pide **una página** al servidor (`page`/`limit`) y a continuación
entrega ese resultado a antd junto con el `total` del servidor, de modo que antd
**vuelve a recortar** un conjunto ya paginado (`frontend/node_modules/antd/es/table/InternalTable.js:328-335`).
Además `clientFilter` se aplica **antes** de ese recorte
(`frontend/src/components/custom_table.tsx:462-464`), el `pageSize` del estado se
**sobreescribe** con el que devuelve el servidor en cada respuesta (`:380`) y el
`total` viaja *stale* en el cambio de página (`:310`). Síntoma observado por el
usuario en *Active Bans*: la página muestra 10 filas y al pasar a la siguiente el
número cambia. En el mismo componente el parámetro `asc` **nunca se envía**
(`:344-345`), así que ordenar una columna siempre consulta al servidor en sentido
ascendente.

El backend es correcto y no se toca: `backend/src/http/ban.rs:189-218` pagina con
`skip`/`take`, `records = bans.len()` y `pages = ceil(records/limit)`.

**2. Las preferencias del Log Viewer no sobreviven a la navegación.** La spec
`log-viewer` vigente ya exige persistirlas en `localStorage`, pero el commit
`ec33b9b` (2026-09-22) introdujo `updateUrl()`
(`frontend/src/pages/admin/logs_page.tsx:217-230`) y un `componentDidMount` que
**lee de la URL y sobreescribe** lo persistido (`:232-248`). Al salir y volver, la
URL llega sin query: `hiddenEvents` vuelve a `[]` y `autoRefresh` a `false`. Es una
**regresión** contra el escenario *«State survives navigation away and back»* de la
spec. Además se quiere que `autoRefresh` esté **activado por defecto**, lo que
**cambia** el escenario *«State defaults when localStorage is empty»* (hoy exige
`false`).

**3. El gráfico *Evolution by Method* no aporta nada.** Ocupa una tarjeta entera del
dashboard y su serie se solapa con la del top de métodos. Se retira de la UI.

## What Changes

- **A. Paginación (requisito nuevo).** `CustomTable` pasa a ser **el único
  paginador** de las tablas paginadas en servidor: recibe una página y no deja que
  antd la recorte; `clientFilter` **desaparece** como prop (era una bomba latente:
  filtrar antes del recorte es exactamente cómo se rompió el filtro de pipeline en
  la página de reglas); el `pageSize` es del cliente y no se reescribe desde la
  respuesta; y el sentido de ordenación (`asc`) viaja a la API. Nuevo módulo de spec
  `table-pagination`.

- **B. Preferencias del Log Viewer (MODIFIED).** `localStorage` pasa a ser la
  **única** fuente de verdad: la query string de la URL no se lee ni se escribe para
  esta finalidad. `autoRefresh` pasa a valer **`true`** por defecto cuando no hay
  nada persistido. Es un **MODIFIED** del requisito
  `Persist filter and auto-refresh state across views` de `log-viewer`, cuyo
  escenario de defaults cambia de `false` a `true`.

- **C. Retirada del gráfico (sin delta de spec).** Se elimina la tarjeta *Evolution
  by Method* de `charts_page.tsx` y el componente
  `frontend/src/components/charts/evolution_by_method.tsx`, que queda huérfano y se
  **borra**. Ninguna spec cubre esa tarjeta, así que no hay delta.

## Non-goals

- **El endpoint `GET /api/v1/stats/evolution_by_method` y la serie `method_series`
  del `StatsCollector` NO se tocan.** No es «quitar un gráfico»: `method_series`
  forma parte del snapshot que se persiste en `stats_cache`
  (`backend/src/models/stats.rs:66`, con `#[serde(default)]` por compatibilidad) y
  el collector lo alimenta en cada `record_blocked`/`record_allowed`. Retirarlo es
  un cambio de formato de persistencia y de una API con su propia spec, cuya sección
  `## Architecture` es prosa y **no es expresable como delta de OpenSpec**. Va como
  change aparte si se decide.
- No se toca el backend, ni el pipeline WAF/Jail, ni las reglas, ni los perfiles.
- No se unifica el flujo *Apply template* con `RuleDialog`.

## Impact

- `frontend/src/components/custom_table.tsx` — paginación, ordenación, retirada de `clientFilter`
- `frontend/src/pages/admin/logs_page.tsx` — `localStorage` como única fuente; `autoRefresh` por defecto
- `frontend/src/pages/admin/charts_page.tsx` — retirada de la tarjeta y de su fetch
- `frontend/src/components/charts/evolution_by_method.tsx` — **se elimina**
- `frontend/src/pages/admin/logs_page.test.tsx` — el escenario de defaults cambia a `true`
- Tests nuevos de paginación para `CustomTable`
- Sin cambios en backend, migraciones, base de datos ni infraestructura

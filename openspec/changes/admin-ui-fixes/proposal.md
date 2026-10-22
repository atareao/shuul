# Proposal: admin-ui-fixes

## Why

Tres defectos de la UI de administración, agrupados porque ninguno justifica un change
propio y los tres se verifican con la misma batería de tests de frontend.

**1. `CustomTable` no es el único paginador de las tablas que paginan en servidor.**

El componente pide **una página** al servidor (`page`/`limit`) y entrega ese resultado a
antd, lo que deja la paginación repartida entre dos responsables. Hay **dos mecanismos
verificados** por los que el componente deja de ser el único paginador:

- el `pageSize` del estado se **sobreescribe** con el `limit` que devuelve el servidor en
  cada respuesta (`frontend/src/components/custom_table.tsx:380`), de modo que el tamaño
  de página elegido por el usuario puede desaparecer;
- cuando el `dataSource` que llega a antd es **mayor** que el `pageSize` del estado, antd
  **vuelve a recortar** un conjunto ya paginado
  (`frontend/node_modules/antd/es/table/InternalTable.js`, antd 6.6.6: cortocircuito
  `if (mergedData.length < total)` y `slice` cuando `mergedData.length > pageSize`).

Además `clientFilter` es una prop **declarada y no usada por ninguna página**, que se
aplica **antes** de ese recorte (`custom_table.tsx:462-464`): una bomba latente para la
primera vista que la use.

**Evidencia y límite de la evidencia.** El síntoma reportado por el usuario en *Active
Bans* («la página muestra 10 filas y al pasar a la siguiente el número cambia») se ha
intentado reproducir y **no se ha reproducido** contra un servidor fiel: con 34 registros,
`CustomTable` renderizado muestra **10 / 10 / 10 / 4** filas y «Total: 34 records (4
pages)». El backend es correcto y no se toca: `backend/src/http/ban.rs:189-218` pagina con
`skip`/`take`, devuelve el `limit` pedido (`.min(100)`) y calcula `pages =
ceil(records/limit)`. El change se justifica, por tanto, por los **mecanismos verificados**
de arriba y por la higiene del componente, **no** por la reproducción del síntoma literal.
El síntoma exacto queda como incógnita abierta.

**Premisas descartadas durante el RED.** El RED desmintió dos afirmaciones de la primera
versión de este proposal, y se dejan escritas para que no vuelvan:

- que `asc` «nunca se envía»: **falso**. `custom_table.tsx:351-356` envía `asc=true` /
  `asc=false` junto a `sort_by` desde `v0.1.2`; el test RED correspondiente **ya pasa en
  verde** antes de tocar código. El requisito se conserva en el delta como **guarda de
  regresión**, no como defecto;
- que `clientFilter` fuera la causa de un fallo del filtro de pipeline en la página de
  reglas: **falso**. Ese filtro ya es server-side vía `params`
  (`frontend/src/pages/admin/rules_page.tsx:299-302`) y ninguna página usa `clientFilter`.

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

- **A. Paginación (requisito nuevo).** `CustomTable` pasa a ser **el único paginador** de
  las tablas paginadas en servidor: recibe una página y no deja que antd la recorte; el
  `pageSize` es del cliente y **no** se reescribe desde la respuesta; y `clientFilter`
  **desaparece** como prop, cerrando la vía de filtrar antes del recorte. Nuevo módulo de
  spec `table-pagination`, que incluye además, como **guarda de regresión**, el contrato
  de que el sentido de ordenación (`asc`) viaja a la API — comportamiento que ya se cumple
  hoy.

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

- **Higiene del arnés de tests (sin delta de spec).** `@testing-library/react` y
  `@testing-library/user-event` se usaban desde el test ya existente pero **no estaban
  declaradas** en `frontend/package.json` (pasaban sólo por *hoisting* de pnpm); se
  declaran como `devDependencies`.

## Non-goals

- **El endpoint `GET /api/v1/stats/evolution_by_method` y la serie `method_series`
  del `StatsCollector` NO se tocan.** No es «quitar un gráfico»: `method_series`
  forma parte del snapshot que se persiste en `stats_cache`
  (`backend/src/models/stats.rs:66`, con `#[serde(default)]` por compatibilidad) y
  el collector lo alimenta en cada `record_blocked`/`record_allowed`. Retirarlo es
  un cambio de formato de persistencia y de una API con su propia spec, cuya sección
  `## Architecture` es prosa y **no es expresable como delta de OpenSpec**. Va como
  change aparte si se decide.
- **No se investiga más el síntoma literal «10 → 12» dentro de este change.** Queda
  documentado como no reproducido; si el usuario aporta una reproducción, se abrirá el
  trabajo con ella.
- No se toca el backend, ni el pipeline WAF/Jail, ni las reglas, ni los perfiles.
- No se unifica el flujo *Apply template* con `RuleDialog`.

## Impact

- `frontend/src/components/custom_table.tsx` — paginación, ordenación, retirada de `clientFilter`
- `frontend/src/pages/admin/logs_page.tsx` — `localStorage` como única fuente; `autoRefresh` por defecto
- `frontend/src/pages/admin/charts_page.tsx` — retirada de la tarjeta y de su fetch
- `frontend/src/components/charts/evolution_by_method.tsx` — **se elimina**
- `frontend/src/pages/admin/logs_page.test.tsx` — el escenario de defaults cambia a `true`
- `frontend/src/components/custom_table.test.tsx` — tests nuevos de paginación
- `frontend/package.json` — dependencias de test declaradas explícitamente
- Sin cambios en backend, migraciones, base de datos ni infraestructura

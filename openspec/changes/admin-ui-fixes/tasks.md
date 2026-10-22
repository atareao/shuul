# Tasks: admin-ui-fixes

> **Alcance:** tres arreglos de la UI de administración (§A paginación,
> §B preferencias del Log Viewer, §C retirada del gráfico *Evolution by Method*).
> **Frontend únicamente**: no se toca el backend, ni el pipeline WAF/Jail, ni el
> endpoint `stats/evolution_by_method` ni la serie `method_series`.
> **Baseline:** `npx vitest run` en verde con su recuento y `npx tsc --noEmit` sin
> errores, medidos en el árbol limpio sobre `develop` antes de empezar. El recuento
> de tests no debe bajar.

## 1. Preparación

- [ ] 1.1 Anotar el baseline: `npx vitest run` en verde con su recuento y `npx tsc --noEmit` sin errores. Verificación: el recuento queda escrito en esta tarea.
- [ ] 1.2 **Reproducir el defecto de paginación antes de tocar código** y dejar escrito el mecanismo exacto que produce el síntoma (*Active Bans* debe mostrar 10 filas y no cambia el número al pasar de página). Verificación: existe un test que **falla** por la razón documentada, con el número observado anotado aquí.
- [ ] 1.3 Confirmar que el header del delta coincide **literalmente** con el de la spec destino: `### Requirement: Persist filter and auto-refresh state across views` en `openspec/specs/log-viewer/spec.md`. Verificación: `grep -n '^### Requirement:' openspec/specs/log-viewer/spec.md` y el delta muestra el mismo título, carácter a carácter.

## 2. Tests (RED)

- [ ] 2.1 RED: `CustomTable` muestra **exactamente** las filas de la página pedida (10 en una página llena, 4 en la última de 34 registros). Verificación: `npx vitest run` falla en aserción.
- [ ] 2.2 RED: la reproducción de 1.2 falla por el mecanismo documentado. Verificación: el fallo nombra la causa (recorte de antd sobre una página ya paginada, `clientFilter` antes del recorte, o `pageSize` reescrito desde la respuesta).
- [ ] 2.3 RED: cambiar el tamaño de página a 50 no lo deja reescrito por el servidor y vuelve a la primera página. Verificación: el test falla en aserción.
- [ ] 2.4 RED: la ordenación descendente envía `asc=false` junto a `sort_by`. Verificación: el test falla porque la petición no lleva `asc`.
- [ ] 2.5 RED (MODIFIED): con `localStorage` vacío, `logs_page` arranca con `autoRefresh === true`. El test existente `uses default state when localStorage has no logsPage key` espera `false` y SHALL actualizarse a `true`. Verificación: el test falla con el valor esperado nuevo.
- [ ] 2.6 RED (MODIFIED): cargar `/admin/logs?hidden=admitted&autoRefresh=false` **no** altera el estado restaurado de `localStorage`. Verificación: el test falla porque hoy la URL sobreescribe.
- [ ] 2.7 RED: alternar un filtro **no** modifica `window.location.search`. Verificación: el test falla porque hoy se llama a `history.replaceState`.
- [ ] 2.8 Retirar los tests de `logs_page.test.tsx` que fijen el comportamiento por URL. Verificación: `grep -n "replaceState\|location.search" frontend/src/pages/admin/logs_page.test.tsx` no devuelve escenarios de comportamiento.

## 3. GREEN

- [ ] 3.1 `custom_table.tsx`: una sola paginación (no dejar que antd recorte una página ya paginada).
- [ ] 3.2 `custom_table.tsx`: enviar el sentido de orden (`asc`) y volver a la primera página al cambiar de columna.
- [ ] 3.3 `custom_table.tsx`: **eliminar** la prop `clientFilter` y todos sus usos; el `pageSize` deja de reescribirse desde la respuesta.
- [ ] 3.4 `logs_page.tsx`: eliminar `updateUrl()` y la lectura de la URL en `componentDidMount`; `localStorage` como única fuente; `autoRefresh` por defecto `true`.
- [ ] 3.5 `charts_page.tsx`: retirar la tarjeta *Evolution by Method* y su petición; **eliminar** `frontend/src/components/charts/evolution_by_method.tsx`.
- [ ] 3.6 Verificar que no queda ninguna referencia: `git grep -n "evolution_by_method" -- frontend/src` no devuelve nada.

## 4. REFACTOR

- [ ] 4.1 `npx tsc --noEmit` sin errores.
- [ ] 4.2 `npm run lint` sin errores ni avisos nuevos y **sin** `eslint-disable`.
- [ ] 4.3 `npx vitest run` en verde; el recuento no baja respecto al baseline y no queda ningún test ignorado o saltado que no lo estuviera antes.

## 5. Verificación manual

- [ ] 5.1 En el navegador: *Active Bans* muestra 10 filas en todas las páginas y el selector de tamaño no se desincroniza.
- [ ] 5.2 En el navegador: ordenar una columna en descendente ordena de verdad.
- [ ] 5.3 En el navegador: entrar en *Log Viewer*, ocultar un tipo de evento, ir a *Rules* y volver → la selección se conserva y el auto-refresh está activo.
- [ ] 5.4 En el navegador: *Charts* ya no muestra la tarjeta *Evolution by Method*.
- [ ] 5.5 Confirmar que la URL no cambia al alternar filtros del Log Viewer.

# Tasks: pagination-defaults

## 1. RED
- [x] 1.1 `templates_page.test.tsx`: con las 42 plantillas, el listado muestra 10 filas y la
      última página muestra las 2 restantes. **Hecho:** test «arranca en 10 filas y la última
      página (la 5ª) muestra las 2 restantes». En rojo antes del GREEN: sin paginación se
      renderizaban las 42 filas (`getDataRows()` devolvía 42, no 10).
- [x] 1.2 `templates_page.test.tsx`: el selector de tamaño de página ofrece 10, 50 y 100. **Hecho:**
      test «ofrece exactamente 10, 50 y 100 en el selector de tamaño de página». En rojo antes del
      GREEN: con `pagination={false}` el desplegable de tamaño no existía y el `combobox` no se
      encontraba.
- [x] 1.3 `templates_page.test.tsx`: al filtrar desde la última página, el listado vuelve a la primera.
      **Hecho:** test «al filtrar desde la última página vuelve a la primera, sin quedar vacío». En
      rojo antes del GREEN: sin `page` en el estado, filtrar desde la 5ª dejaba la tabla vacía.
- [x] 1.4 `logs_page.test.tsx`: el Log Viewer muestra 10 entradas por defecto y su selector ofrece
      10, 50 y 100. **Hecho:** test «6.1 arranca en 10 entradas y el selector ofrece 10, 50 y 100».
      En rojo antes del GREEN: el visor arrancaba en 50 y ofrecía 20/50/100.
- [x] 1.5 `npx vitest run`: los tests nuevos **fallan** y los 35 existentes siguen en verde. **Hecho:**
      los **4 tests nuevos** en rojo y los **29 preexistentes** en verde (33 en total: la estimación
      de 35 era alta). Los dos ficheros de test crecieron **sólo por adición** en el commit del RED
      (`66/0` en `logs_page.test.tsx` y `93/0` en `templates_page.test.tsx`), sin borrar una sola línea.

## 2. GREEN
- [x] 2.1 `templates_page.tsx`: paginación en cliente con 10 por defecto, rangos 10/50/100 y vuelta
      a la primera página al cambiar filtros o búsqueda. **Hecho:** `page`/`pageSize` al estado (10 por
      defecto) y `pagination` controlada en el `<Table>` con `pageSizeOptions` 10/50/100; `page: 1` en
      el `onChange` de la búsqueda y de los cinco filtros (pipeline, severidad, categoría, acción,
      must-have). Cambiar el tamaño de página también vuelve a la página 1.
- [x] 2.2 `logs_page.tsx`: 10 por defecto y rangos 10/50/100. **Hecho:** `pageSize` al estado (10 por
      defecto) y la `pagination` del `<Table>` leyéndolo, con `pageSizeOptions` 10/50/100. Su selector
      estaba **inerte** antes porque la prop era constante; ahora vive en el estado. **No** se persiste
      en `localStorage` (el estado persistido sigue siendo `{hiddenEvents, autoRefresh}`).
- [x] 2.3 `npx vitest run` en verde (los 35 actuales más los nuevos). **Hecho:** **33/33** en verde
      (`vitest exit=0`, 3 ficheros). El andamiaje de los preexistentes que afirmaban el catálogo
      completo se adaptó con un helper (`showWholeCatalog()`) que fija el rango 100 en el propio
      selector; **ninguna aserción se tocó** — el único borrado en el `git diff` de los ficheros de
      test es un comentario (`logs_page.test.tsx`: `15/1`; `templates_page.test.tsx`: `65/0`).

## 3. REFACTOR
- [x] 3.1 `npx tsc -b --force` sin errores: es el gate de `pnpm run build`, el mismo que corre el
      CI y el Dockerfile. **Hecho:** `tsc exit=0`. Dos errores corregidos: `TS6133` (`page` sin usar en
      `logs_page.tsx`, renombrado a `_page` con la convención `_` que ya usa el repo) y `TS2345` (una
      unión de dos formas en el ternario de `templates_page.tsx`, reescrita como un único literal).
- [x] 3.2 `npm run lint` sin avisos, sin `eslint-disable` y sin `any`. **Hecho:** `lint exit=0`, sin
      avisos nuevos; ningún `eslint-disable`, `any`, `@ts-ignore` ni `@ts-expect-error` añadido.
- [x] 3.3 Repasar que `CustomTable` no cambia de comportamiento: ya cumple las tres condiciones.
      **Hecho:** `custom_table.tsx` no se tocó; el reset a la página 1 de las vistas nuevas replica el
      que ya hacía `CustomTable`, sin alterar sus props.

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Plantillas: 10 filas por página y el selector con 10, 50 y 100.
- [ ] 4.2 Log Viewer: 10 entradas por defecto y el selector con 10, 50 y 100.
- [ ] 4.3 Reglas, baneos y perfiles de rate limit: siguen arrancando en 10 con los mismos rangos.

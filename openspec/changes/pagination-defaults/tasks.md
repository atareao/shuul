# Tasks: pagination-defaults

## 1. RED
- [ ] 1.1 `templates_page.test.tsx`: con las 42 plantillas, el listado muestra 10 filas y la
      última página muestra las 2 restantes.
- [ ] 1.2 `templates_page.test.tsx`: el selector de tamaño de página ofrece 10, 50 y 100.
- [ ] 1.3 `templates_page.test.tsx`: al filtrar desde la última página, el listado vuelve a la primera.
- [ ] 1.4 `logs_page.test.tsx`: el Log Viewer muestra 10 entradas por defecto y su selector ofrece
      10, 50 y 100.
- [ ] 1.5 `npx vitest run`: los tests nuevos **fallan** y los 35 existentes siguen en verde.

## 2. GREEN
- [ ] 2.1 `templates_page.tsx`: paginación en cliente con 10 por defecto, rangos 10/50/100 y vuelta
      a la primera página al cambiar filtros o búsqueda.
- [ ] 2.2 `logs_page.tsx`: 10 por defecto y rangos 10/50/100.
- [ ] 2.3 `npx vitest run` en verde (los 35 actuales más los nuevos).

## 3. REFACTOR
- [ ] 3.1 `npx tsc -b --force` sin errores: es el gate de `pnpm run build`, el mismo que corre el
      CI y el Dockerfile.
- [ ] 3.2 `npm run lint` sin avisos, sin `eslint-disable` y sin `any`.
- [ ] 3.3 Repasar que `CustomTable` no cambia de comportamiento: ya cumple las tres condiciones.

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Plantillas: 10 filas por página y el selector con 10, 50 y 100.
- [ ] 4.2 Log Viewer: 10 entradas por defecto y el selector con 10, 50 y 100.
- [ ] 4.3 Reglas, baneos y perfiles de rate limit: siguen arrancando en 10 con los mismos rangos.

# Tasks: ban-row-identity

## 1. RED — reproducir la acumulación de filas
- [ ] 1.1 Test que renderiza `CustomTable` con una página de 10 filas donde dos comparten `id`
      y afirma que se renderizan exactamente 10 filas.
- [ ] 1.2 Espía de `console.error` que afirma que React no emite el aviso de claves duplicadas.
- [ ] 1.3 `npx vitest run`: confirmar que el test nuevo **falla**, y anotar si jsdom reproduce o
      no el DOM acumulado. Si no lo reproduce, dejar el test en la forma que sí reproduzca el
      defecto (claves duplicadas), sin debilitar la afirmación.
- [ ] 1.4 Confirmar que el resto de la suite sigue en verde.

## 2. GREEN — identidad por partición
- [x] 2.1 `CustomTable`: prop opcional `rowKey?: (record: T) => string | number`, por defecto
      `record.id`. **Hecho:** prop opcional en `Props<T>` y `<Table rowKey={rowKey ?? ((record) => record.id.toString())}>`; sin `rowKey` el comportamiento es idéntico al de hoy (test 4.4 en verde). Documentada la causa en el JSDoc de la prop.
- [x] 2.2 `BansPage`: pasar la partición `${ban.ip_address}:${ban.rule_id ?? 0}`. **Hecho:** `banRowKey` en `bans_page.tsx` y `rowKey={banRowKey}` en el `CustomTable`; el `?? 0` distingue el baneo sin regla (tests 4.1–4.3 y los dos de `bans_page` en verde).
- [x] 2.3 `npx vitest run` en verde (los 29 actuales más los nuevos). **Hecho:** **4 ficheros de test, 35 tests, 0 fallos** (los 29 previos + los 6 nuevos), `EXIT=0`.

## 3. REFACTOR
- [x] 3.1 `npx tsc -b --force` sin errores: es el gate de `pnpm run build`, el mismo que corre
      el CI y el Dockerfile. **Hecho:** `EXIT=0`, 0 errores (resuelve los 3 `TS2322` de `Property 'rowKey' does not exist` declarando el tipo de la prop, sin silenciar).
- [x] 3.2 `npm run lint` sin avisos, sin `eslint-disable` y sin `any`. **Hecho:** `EXIT=0`, sin avisos.
- [x] 3.3 Repasar que ninguna otra página que use `CustomTable` cambia de comportamiento. **Hecho:** los únicos consumidores son `rules_page.tsx` y `rate_limit_profiles_page.tsx`, y ninguno pasa `rowKey` → siguen usando `record.id`. (`templates_page.tsx` y `logs_page.tsx` usan el `<Table>` de antd directamente, fuera del alcance de este change.)

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Active Bans: la página 1 muestra 10 filas, la siguiente 10 y al volver 10, sin crecimiento.
- [ ] 4.2 Las dos filas de una IP baneada dos veces siguen siendo distinguibles por la columna Rule.
- [ ] 4.3 Desbanear una de las dos filas quita sólo esa y la otra permanece activa.

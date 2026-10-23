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
- [ ] 2.1 `CustomTable`: prop opcional `rowKey?: (record: T) => string | number`, por defecto
      `record.id`.
- [ ] 2.2 `BansPage`: pasar la partición `${ban.ip_address}:${ban.rule_id ?? 0}`.
- [ ] 2.3 `npx vitest run` en verde (los 29 actuales más los nuevos).

## 3. REFACTOR
- [ ] 3.1 `npx tsc -b --force` sin errores: es el gate de `pnpm run build`, el mismo que corre
      el CI y el Dockerfile.
- [ ] 3.2 `npm run lint` sin avisos, sin `eslint-disable` y sin `any`.
- [ ] 3.3 Repasar que ninguna otra página que use `CustomTable` cambia de comportamiento.

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Active Bans: la página 1 muestra 10 filas, la siguiente 10 y al volver 10, sin crecimiento.
- [ ] 4.2 Las dos filas de una IP baneada dos veces siguen siendo distinguibles por la columna Rule.
- [ ] 4.3 Desbanear una de las dos filas quita sólo esa y la otra permanece activa.

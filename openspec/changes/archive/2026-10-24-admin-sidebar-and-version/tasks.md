# Tasks: admin-sidebar-and-version

## 1. RED (tests que fallan primero)

- [x] 1.1 `src/constants.test.ts` (nuevo): `formatTitle` devuelve el nombre con la versión entre paréntesis cuando la hay, y el nombre sin paréntesis cuando recibe `undefined`, cadena vacía o solo espacios; y `TITLE` nunca contiene la cadena `undefined`.
- [x] 1.2 `src/build_env.test.ts` (nuevo): `frontend/.env.production` declara `VITE_VERSION` con una versión semántica, esa versión es la misma que la que gobierna el versionado del proyecto (`.vampus.yml`), y ese fichero mantiene el `VITE_VERSION` en su lista de sustituciones.
- [x] 1.3 `src/layouts/admin_layout.test.tsx`: las tres entradas del pie son entradas de menú (`menuitem`) con nombre accesible «Theme», «Log out» y «Collapse», viven en un bloque al pie que NO contiene el menú de navegación, y ese bloque es el último del cuerpo de la barra.
- [x] 1.4 `src/layouts/admin_layout.test.tsx`: con la barra colapsada el bloque del pie se pliega a iconos (lleva la marca de plegado de antd) y las tres entradas siguen existiendo.
- [x] 1.5 `src/layouts/admin_layout.test.tsx`: la entrada de colapso anuncia el estado con `aria-expanded` —verdadero desplegada, falso colapsada— y alterna el estado al pulsarla.
- [x] 1.6 `src/layouts/admin_layout.test.tsx`: pulsar la entrada de tema cambia el tema —cambia el modo persistido— y el icono de la entrada refleja el tema activo.
- [x] 1.7 `src/layouts/admin_layout.test.tsx`: pulsar «Log out» navega a `/admin/logout`.
- [x] 1.8 `src/layouts/admin_layout.test.tsx`: la barra lateral queda pegada a la ventana (posición pegajosa, anclada arriba y de la altura de la ventana) y el menú de navegación se desplaza dentro de la barra.
- [x] 1.9 `src/layouts/admin_layout.test.tsx`: la marca —icono y nombre— encabeza la barra por encima de la primera entrada de navegación, y con la barra colapsada se ve solo el icono.
- [x] 1.10 `src/layouts/admin_layout.test.tsx`: el pie de página de la aplicación muestra el nombre de la aplicación y NO contiene la cadena `undefined` (guarda de regresión: no se espera que falle en RED en local, donde `.env` sí declara una versión).
- [x] 1.11 Ejecutar `npx vitest run`: los tests nuevos fallan por la ausencia del comportamiento —no por errores de montaje ni de tipos— y los que no cambian de contrato siguen en verde.
- [x] 1.12 `src/test/setup.ts`: registrar los matchers de `@testing-library/jest-dom` con su flavor de vitest —hoy el setup importa el flavor de jest, cuyos tipos no aumentan el `Assertion` de vitest—, para que `toHaveTextContent` y `toHaveClass` existan también para el compilador y el RED falle por la ausencia del comportamiento, no por tipos.

## 2. GREEN (implementación mínima)

- [x] 2.1 `frontend/.env.production` (nuevo): `VITE_BASE_URL=""` y `VITE_VERSION=0.20.0`.
- [x] 2.2 `.vampus.yml`: añadir `frontend/.env.production` a `replaces` con el mismo patrón de `VITE_VERSION` que ya tiene `.env.development`.
- [x] 2.3 `src/constants.ts`: `formatTitle`, que omite el paréntesis cuando no hay versión, y `TITLE` construido con él.
- [x] 2.4 `src/pages/admin/dashboard_page.tsx` y `src/layouts/admin_layout.tsx`: usar el `TITLE` de `constants` en lugar de la plantilla literal local.
- [x] 2.5 `src/layouts/admin_layout.tsx`: el pie pasa a ser un `Menu` con tres entradas (`Menu.Item`), hermano del menú de navegación y no seleccionable, con las etiquetas en inglés; la entrada de tema consume el contexto de tema y conmuta al pulsarla; la de colapso alterna el estado y declara `aria-expanded` y `aria-controls`.
- [x] 2.6 `src/layouts/admin_layout.tsx`: la barra lateral se pega a la ventana y el menú de navegación se desplaza dentro, con crecimiento flexible.
- [x] 2.7 `src/layouts/admin_layout.tsx`: la marca en la parte alta —icono y nombre—, con el color del texto tomado del token de antd; con la barra colapsada, solo el icono. El `div.demo-logo-vertical` desaparece.
- [x] 2.8 `npx vitest run` en verde, `npx tsc -b --force` y `npm run lint` limpios.

## 3. REFACTOR

- [x] 3.1 Comprobar con `pnpm build` que la versión queda horneada en `dist/` —buscando el valor en el bundle— y que sin declaración no aparece `undefined` en la interfaz.
- [x] 3.2 `npm run lint` y `npx tsc -b --force` limpios, sin `eslint-disable` ni `any`; ningún color literal nuevo en el diff.
- [x] 3.3 Comprobar que no queda ninguna referencia al `div.demo-logo-vertical` ni ninguna construcción literal del título de la aplicación.

## 4. Verificación manual (operador, en el navegador)

- [ ] 4.1 En Dashboard, Templates y Charts: el bloque del pie de la barra lateral —Theme, Log out y Collapse— se ve sin desplazarse, aunque la página sea más alta que la ventana.
- [ ] 4.2 Con la barra colapsada: solo iconos, sin desbordar el raíl, y el icono de Shuul sigue arriba.
- [ ] 4.3 La versión se ve correcta —no `undefined`— en el título del Dashboard y en el pie de las páginas.

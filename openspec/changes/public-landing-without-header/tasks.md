# Tasks: public-landing-without-header

## 1. RED (tests que fallan primero)

- [x] 1.1 `src/layouts/public_layout.test.tsx`: reescribir el test de la cabecera —el contrato viejo exigía que existiera un elemento con la clase `ant-layout-header`— por el contrato nuevo: la envoltura pública NO pinta ningún elemento con esa clase.
- [x] 1.2 `src/layouts/public_layout.test.tsx`: la envoltura pública NO ofrece ningún control de inicio de sesión —no hay botón cuyo nombre accesible sea «sign in» o «log in»—, y el mock de `startSsoLogin` deja de tener uso en este fichero.
- [x] 1.3 `src/layouts/public_layout.test.tsx`: el pie de la parte pública sigue existiendo y sigue mostrando el copyright.
- [x] 1.4 `src/layouts/public_layout.test.tsx`: el contenido de la ruta hija se sigue renderizando —el `Outlet` no se lleva por delante el contenido—.
- [x] 1.5 Ejecutar `npx vitest run`: los tests nuevos fallan por la ausencia del comportamiento —no por errores de montaje ni de tipos— y los que no cambian de contrato siguen en verde.

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/layouts/public_layout.tsx`: fuera el `Header` y los cuatro `import` que solo le servían —`Button`, `LoginOutlined`, `ModeSwitcher` y `startSsoLogin`—; la envoltura se queda con el contenido y el pie.
- [x] 2.2 `src/layouts/public_layout.tsx`: el `Content` pasa a ser un contenedor flexible en columna, para que la portada pueda ocupar el alto disponible.
- [x] 2.3 `src/pages/public/home_page.tsx`: la portada ocupa el alto disponible en vez de declarar `height: "100vh"`.
- [x] 2.4 `src/components/mode_switcher.tsx`: se retira —quedaba sin consumidores al irse la cabecera—.
- [x] 2.5 `npx vitest run` en verde, `npx tsc -b --force` y `npm run lint` limpios.

## 3. REFACTOR

- [x] 3.1 `npm run lint` y `npx tsc -b --force` limpios, sin `eslint-disable` ni `any`.
- [x] 3.2 Comprobar que no queda ninguna referencia a `mode_switcher` ni a la cabecera pública —ni en código, ni en tests, ni en el `dist` recién construido—.
- [x] 3.3 `pnpm build` sin errores.

## 4. Verificación manual (operador, en el navegador)

- [x] 4.1 Sin sesión, en `/`: NO hay franja superior. La portada empieza en la parte alta de la pantalla.
- [x] 4.2 La portada cabe entera, sin barra de desplazamiento vertical.
- [x] 4.3 El botón «Sign in with PocketID» sigue arrancando el flujo SSO, y el pie con el copyright sigue a la vista.

## 5. Archivado

- [ ] 5.1 `openspec archive public-landing-without-header` en rama propia, y actualizar el `Purpose` de `openspec/specs/app-shell/spec.md` —que hoy dice «la envoltura pública con su cabecera»—, porque el archivado no lo reescribe.

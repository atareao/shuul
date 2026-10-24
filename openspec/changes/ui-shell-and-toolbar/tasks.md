# Tasks: ui-shell-and-toolbar

## 1. RED (tests que fallan primero)

- [x] 1.1 Frontend: test que fija que el layout del panel no pinta cabecera —no hay ningún elemento con la clase `ant-layout-header`— y que siguen existiendo la barra lateral y el pie de página. Fichero nuevo `layouts/admin_layout.test.tsx`.
- [x] 1.2 Frontend: test que fija que el botón de cerrar sesión, el conmutador de tema y el control de colapsar están dentro de la barra lateral y comparten contenedor, anclado abajo.
- [x] 1.3 Frontend: test que fija que ese pie coloca los tres controles en fila con la barra desplegada y en columna con la barra colapsada.
- [x] 1.4 Frontend: test que fija que los botones de cerrar sesión y de colapsar exponen nombre accesible, y que el conmutador de tema conserva el suyo.
- [x] 1.5 Frontend: test que fija que el control de colapsar alterna el estado de la barra (la clase `ant-layout-sider-collapsed` va y viene).
- [x] 1.6 Frontend: test que fija que el `AuthGuard`, sin sesión, redirige a `/` y no a `/login`.
- [x] 1.7 Frontend: test que fija que la portada pública ofrece un botón rotulado que arranca el SSO y que el icono de la cabecera pública hace lo mismo; el helper del SSO se sustituye por un mock. Fichero nuevo `pages/public/home_page.test.tsx`.
- [x] 1.8 Frontend: test que fija que `/login` ya no es una ruta propia y que visitarla lleva a la portada.
- [x] 1.9 Frontend: test que fija que el buscador de plantillas está en la misma fila que los cuatro filtros y el interruptor, que la fila no envuelve y que el buscador es el elemento flexible mientras los filtros pueden encogerse. En `pages/admin/templates_page.test.tsx`.
- [x] 1.10 Ejecutar `npx vitest run`: los nuevos fallan y **todos** los existentes siguen en verde.

## 2. GREEN (implementación mínima)

- [x] 2.1 `common/auth.ts`: el helper que arranca el SSO, único punto que compone `{BASE_URL}/api/v1/auth/sso`.
- [x] 2.2 Borrar `pages/public/login_page.tsx` y su ruta en `App.tsx`; `/login` pasa a redirigir a `/`.
- [x] 2.3 Borrar `components/signin.tsx` y `components/register.tsx`, sin rescatar nada de su interior.
- [x] 2.4 `layouts/admin_layout.tsx`: fuera el `Header`; el pie de la barra lateral con los tres controles, `trigger={null}` y `styles={{ body }}` en columna para anclarlo abajo.
- [x] 2.5 `layouts/admin_layout.tsx`: el `AuthGuard` navega a `/`.
- [x] 2.6 `components/mode_switcher.tsx`: fuera el `marginLeft`; `layouts/public_layout.tsx` recupera la separación con un `gap`.
- [x] 2.7 `layouts/public_layout.tsx`: el icono arranca el SSO con el helper.
- [x] 2.8 `pages/public/home_page.tsx`: el botón rotulado de entrada.
- [x] 2.9 Las nueve redirecciones del 401 pasan de `/login` a `/`.
- [x] 2.10 `pages/admin/templates_page.tsx`: el buscador entra en la fila de filtros como primer elemento y pasa a ser el flexible; los filtros, encogibles.
- [x] 2.11 `npx vitest run` en verde y `tsc -b --force` limpio.

## 3. REFACTOR

- [x] 3.1 `npm run lint` y `tsc -b --force` limpios, sin `eslint-disable` ni `any`.
- [x] 3.2 Comprobar que no queda ninguna referencia **ejecutable** a `/login` en `frontend/src`: las únicas apariciones son el comentario que explica la redirección en `App.tsx` y las rutas de prueba que comprueban que no se navega a ella.
- [x] 3.3 Comprobar que la URL del SSO se compone en un único sitio.
- [x] 3.4 Comprobar que no queda ninguna referencia a `signin`, `register`, `UserCredentials` ni `SignInSubmitHandler` en `frontend/src`.
- [x] 3.5 Comprobar que el anclaje del pie no depende de CSS global: va por el `styles` del `Sider`, sin reglas nuevas en `App.css` ni en `index.css`.

## 4. Verificación manual (operador, en el navegador)

- [ ] 4.1 Sin sesión, abrir `/admin/`: se acaba en la portada, con el botón de iniciar sesión a la vista.
- [ ] 4.2 En el panel: no hay franja superior; el pie de la barra lateral lleva cerrar sesión, tema y colapsar en una fila, y al colapsar pasan a columna sin salirse del raíl.
- [ ] 4.3 En plantillas: el buscador y los filtros comparten una sola línea; al estrechar la ventana los controles se encogen y la fila no se parte.

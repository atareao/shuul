# Proposal: admin-sidebar-and-version

## Why

**Cuatro detalles que se ven al usar la v0.20.0 en el navegador.** Los tres primeros son de la barra lateral del panel; el cuarto es un defecto de build que la interfaz arrastra desde hace varias versiones.

**1. Las tres entradas del pie no se parecen a las del menú.** El change anterior bajó al pie el botón de cerrar sesión, el conmutador de tema y el control de colapsar, pero los dejó como **botones** —el tema, incluso, como un `Switch`— dentro de un `div` con `display: flex`. Las entradas de navegación, en cambio, son entradas de menú con icono y etiqueta, una por línea. El resultado es un bloque que no se lee como el resto de la barra.

**2. El pie desaparece en Dashboard, Templates y Charts.** Es una **regresión** del change anterior, no un compromiso. El pie se ancla con `marginTop: "auto"` dentro del cuerpo del `Sider`, y el `Sider` se estira a la altura del layout: en las páginas más altas que la ventana el cuerpo crece, el `auto` empuja el pie al fondo del **contenido** y el bloque se va por debajo del pliegue. En las páginas cortas se ve porque el layout mide exactamente `100vh`. El disparador nativo de antd era `position: fixed`, así que antes de aquel change el bloque **siempre** estaba a la vista.

**3. La barra lateral no lleva la marca de la aplicación.** Arriba del todo hay un `div.demo-logo-vertical` —un resto de la plantilla de antd, sin una sola regla CSS que lo pinte—, así que la barra empieza directamente en «Dashboard» y no dice en ningún sitio en qué aplicación estás.

**4. La versión se imprime como `(undefined)`.** `constants.ts` lee `import.meta.env.VITE_VERSION` y **nadie la define para el build de producción**: el `Dockerfile` solo declara `VITE_BASE_URL` y el workflow de release no pasa ningún argumento de build. El único fichero que declara `VITE_VERSION` es `frontend/.env.development`, y Vite **no lo carga** en un build de producción (carga `.env` y `.env.production`). El `VERSION` que leen el título del Dashboard y el pie de las páginas es, por tanto, `undefined`, y la plantilla literal `` `Shuul (${VERSION})` `` lo escribe tal cual.

## What Changes

1. **Las tres entradas del pie pasan a ser entradas de menú.** Un segundo `Menu` dentro de la barra lateral, hermano del de navegación, con tres entradas con icono y etiqueta, una por línea: «Theme», «Log out» y «Collapse». La entrada de tema **cambia el tema al pulsarla** y su icono refleja el tema activo. La de colapsar alterna el estado y lo anuncia con `aria-expanded`. Con la barra colapsada las tres muestran solo su icono, como las de navegación.
2. **El pie deja de irse por debajo del pliegue.** La barra lateral se pega a la ventana (`position: sticky`, `top: 0`, `height: 100vh`) y el menú de navegación se desplaza dentro, con crecimiento flexible. El pie queda siempre al fondo de la parte visible.
3. **La barra lleva la marca.** El `div` de la plantilla se sustituye por el icono de Shuul y el nombre «Shuul», en la parte alta y por encima de la navegación. Con la barra colapsada se ve solo el icono.
4. **La versión se declara para el build de producción.** Un `frontend/.env.production` versionado por `.vampus.yml` —igual que ya lo está `.env.development`— y un `formatTitle` que, si la versión no estuviera, imprime el nombre sin paréntesis en vez de `undefined`.

## Decisions taken when opening the change

- **El pie es un `Menu`, no una fila de botones.** Es lo único que da la misma forma que la navegación sin escribir una sola regla CSS: mismas clases, mismo `theme="dark"`, mismo relleno, y se pliega solo con la barra porque `Menu` lee `siderCollapsed` del `SiderContext`. Además, `selectable={false}` evita que las tres entradas entren en el estado «seleccionada», que es cosa de la navegación.
- **La API de hijos (`<Menu.Item>`), no la de `items`.** `aria-expanded` y `aria-controls` son obligatorios en la entrada de colapsar, y con la API de `items` no hay forma de declararlos sin salirse de los tipos: el tipo de un item solo admite `data-*` como índice. Con `<Menu.Item>` los props son `React.HTMLAttributes<HTMLLIElement>` y los atributos llegan al `<li role="menuitem">`. Verificado contra el antd instalado, no supuesto.
- **La entrada de tema muestra el icono del tema activo** (luna en oscuro, sol en claro), que es lo que hacía el `Switch` al que sustituye.
- **La etiqueta de la entrada de colapso es fija: «Collapse».** El escenario «Las etiquetas están en inglés» fija las tres etiquetas de forma literal, y el estado lo anuncia `aria-expanded` —el patrón habitual de un control de despliegue—, no el nombre accesible. Una etiqueta que alternara entre «Collapse» y «Expand» dejaría la entrada sin un nombre estable por el que localizarla y en pantalla no se leería nunca: con la barra colapsada solo se ve el icono.
- **El color del texto de la marca sale del token de antd** (`colorTextLightSolid`). La barra lateral es oscura y `.ant-layout-sider-dark` solo declara `background`, no `color`: sin un color explícito el nombre se heredaría del texto oscuro de la página y quedaría invisible. Se prefiere el token a un literal, así que el layout vuelve a recibir el `token` que se le retiró en el change anterior —ahora con un uso real—, como ya hace la envoltura pública.
- **La versión va en `frontend/.env.production`, no en un argumento de build.** Es el mecanismo que el repositorio ya usa para la versión —`frontend/.env.development` está versionado por vampus con el mismo patrón—, no exige tocar el `Dockerfile` ni el workflow de release, y se puede comprobar en local con `pnpm build` sin construir una imagen. `vampus` escribe la versión nueva en los dos ficheros de entorno en el mismo commit en que sube `backend/Cargo.toml`, así que las tres no pueden desincronizarse.
- **La interfaz no imprime `undefined` aunque la declaración falte.** `formatTitle` omite el paréntesis cuando no hay versión. Es una red de seguridad: el defecto de fondo lo cierra el fichero de entorno, y un test lo vigila.

## Límites que este change revoca

Dos de los límites que el change anterior aceptó por escrito quedan revocados aquí, y por eso se dejan dichos:

- **«El pie de la barra lateral no es pegajoso».** Era una regresión, no un compromiso: el disparador nativo de antd era `position: fixed` y siempre estaba a la vista. Se arregla con `position: sticky` sobre la barra, que no exige copiar ningún color interno de antd.
- **«El `div.demo-logo-vertical` se queda».** Desaparece: lo sustituye la marca real.

## Límites que se aceptan

- **jsdom no calcula layout.** El comportamiento pegajoso de la barra y el plegado de las etiquetas se fijan en los tests por el **mecanismo** declarado (la posición y la altura de la barra, la marca de plegado de antd), no por los píxeles: la comprobación visual es del operador, en las tareas 4.1 y 4.2.
- **El título del Dashboard no tiene test propio.** Montar esa página exige simular sus seis endpoints, más el enrutador y el i18n, para una aserción de una línea: el título del Dashboard y el pie del panel leen **el mismo** `TITLE`, que sí está cubierto por el test de `constants`, y la comprobación en pantalla es la tarea 4.3.
- **El icono de la aplicación (`src/assets/logo.svg`, 156 KB con mapas de bits incrustados) se usa tal cual.** Reducirlo es un trabajo aparte y no lo pide este change.

## Impact

- **Frontend**: `layouts/admin_layout.tsx` (el grueso del change), `constants.ts`, `pages/admin/dashboard_page.tsx`, más dos ficheros de test nuevos y la reescritura de los tests del pie en `layouts/admin_layout.test.tsx`.
- **Configuración**: `frontend/.env.production` (nuevo) y `.vampus.yml` (una entrada más en `replaces`).
- **Fuera de alcance**: backend, WAF, Jail, `Dockerfile`, workflows y el `compose.yaml` del despliegue. No hay migración de datos ni cambio de API.

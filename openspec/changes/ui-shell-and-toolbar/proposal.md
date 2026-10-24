# Proposal: ui-shell-and-toolbar

## Why

Tres cosas del armazón de la interfaz, sin relación entre ellas, que van en un mismo change porque ninguna toca lógica de dominio: la entrada al panel, la cabecera del panel y la barra de herramientas de plantillas.

**1. `/login` es un desvío que ya no aporta nada.**

La ruta existe (`App.tsx:70`), su página es un logo y un botón que arranca el SSO (`login_page.tsx:32-34`), y la sesión cerrada desemboca siempre ahí: el `AuthGuard` navega a `/login` (`admin_layout.tsx:72`), el icono de la cabecera pública navega a `/login` (`public_layout.tsx:45`) y **nueve sitios** hacen `window.location.href = "/login"` al recibir un 401 —el helper compartido `common/utils.ts:52` y ocho copias inline en `dialogs/custom_dialog.tsx:151`, `dialogs/rule_dialog.tsx:171,244`, `dialogs/rate_limit_profile_dialog.tsx:171`, `pages/admin/settings_page.tsx:63,95`, `pages/admin/logs_page.tsx:172` y `pages/admin/templates_page.tsx:320`.

El usuario sin sesión tiene que acabar en la página pública —`shuul.territoriolinux.es`— y allí encontrar el botón de iniciar sesión. Una ruta intermedia que solo sirve para pintar ese mismo botón es un salto de más, y además parte el flujo en dos sitios que hay que mantener de acuerdo.

**2. La cabecera del panel gasta 64 px de alto para dos botones.**

`admin_layout.tsx:131-147` pinta un `Header` con el botón de cerrar sesión y el conmutador de tema alineados a la derecha; el resto de la franja está vacío. La barra lateral, que ya es el sitio de la navegación, tiene además un pie nativo de 48 px (`.ant-layout-sider-trigger`) que hoy solo aloja la flecha de colapsar. Los dos botones caben ahí abajo, y cada página del panel recupera 64 px de alto.

**3. El buscador de plantillas ocupa una línea entera para él solo.**

`templates_page.tsx:598-606` declara el buscador con `width: "100%"` en su propia línea, y el comentario de `:596-597` explica que se hizo así «para no competir por el espacio con los filtros». La fila de filtros mide unos 880 px de contenido (cuatro `Col` de 160, 160, 240 y 160 px, los separadores de 12 px y el interruptor «Must have»), así que en cualquier pantalla de escritorio sobra sitio para que el buscador comparta esa línea en vez de gastar una propia.

**Y la deuda de la misma era se va con ellos.** `components/signin.tsx` y `components/register.tsx` —106 líneas cada uno, 212 en total— son los formularios de login local, con campos de email y contraseña, que ya no importa nadie: ni una importación en `frontend/src`, y los dos tipos que exportan (`UserCredentials` y `SignInSubmitHandler`) no aparecen en ningún otro fichero. Son restos de antes del SSO obligatorio, exactamente como la página `/login`. Y ya costaron mantenimiento sin servir a nadie: el change `frontend-lint-cleanup` los tocó en octubre para quitarles seis avisos de lint a cada uno (`openspec/changes/archive/2026-10-18-frontend-lint-cleanup/proposal.md:47-48`) y siguieron sin usarse.

## What Changes

1. **Se elimina la ruta `/login` y la página que la servía.** La sesión cerrada desemboca en la portada pública: el `AuthGuard` navega a `/` y las nueve redirecciones del 401 pasan a `/`. Una visita a `/login` —un enlace o marcador anterior— redirige a `/`, en lugar de dejar la pantalla en blanco como haría una ruta que ya no existe.
2. **La portada pública es la entrada al panel.** `home_page.tsx` incorpora un botón rotulado que arranca el SSO, con la etiqueta que usaba la página borrada («Sign in with PocketID»). El icono de la cabecera pública (`public_layout.tsx:45`) se queda y arranca el SSO directamente, sin ruta intermedia. Los dos llaman al mismo helper nuevo, `common/auth.ts`, que pasa a ser el único sitio donde se compone la URL del endpoint de SSO.
3. **La cabecera del panel desaparece.** El botón de cerrar sesión y el conmutador de tema bajan al pie de la barra lateral, junto al control de colapsar/descolapsar, que pasa a ser un botón propio: el `Sider` se declara con `trigger={null}` y el pie es un contenedor dentro del `Sider` anclado abajo con `margin-top: auto` sobre el cuerpo en columna que permite `styles={{ body }}` de antd v6. Cuando la barra está desplegada los tres controles van en una fila; cuando está colapsada, en columna, porque tres controles no caben en los 80 px del raíl.
4. **El buscador de plantillas comparte la línea de los filtros.** Entra en el mismo `Row` que los cuatro `Select` y el interruptor «Must have», como primer elemento, y pasa a ser el elemento flexible: declara su ancho preferido, crece por encima de él para llevarse el hueco libre y cede espacio encogiéndose cuando falta. Los filtros declaran su ancho preferido y pueden encogerse también, pero no crecen: solo el buscador se lleva el hueco libre, y el interruptor `must_have` no se encoge. La fila no envuelve (`wrap={false}`): «todo en una única línea» se cumple a cualquier ancho, y el peor caso es que los controles se estrechen, nunca que la fila se parta ni que el contenido se salga del contenedor. Para que eso sea cierto de verdad, cada `Col` flexible declara `minWidth: 0`: sin él un `Col` de una fila flex tiene `min-width: auto` —su tamaño de contenido—, la fila se queda con un suelo duro y desborda su contenedor en ventanas estrechas. `min-width: 0` no es un mínimo rígido, es lo que levanta el suelo automático; lo que no se declara es ningún `min-width` con valor positivo.
5. **Se borran los dos formularios de login local.** `components/signin.tsx` y `components/register.tsx` desaparecen: son 212 líneas de código muerto del mismo desvío que la página `/login`, y con ellos se va el último modelo de credenciales locales —email y contraseña— del frontend.

**Decisiones tomadas al abrir el change:**

- **La redirección es relativa (`/`), no absoluta.** El destino es el mismo origen que sirve el panel, así que `https://shuul.territoriolinux.es` y `/` son la misma página; escribir el dominio en el código ataría el frontend a producción —rompería el desarrollo en `localhost` y los tests, que montan la aplicación sin dominio— sin aportar nada. El propio backend ya usa una ruta relativa para volver del SSO (`auth.rs:208`, `window.location.href = "/admin/"`). Si se quiere el dominio explícito, es una constante en un sitio.
- **El pie de la barra lateral es propio, no el disparador nativo de antd.** El disparador nativo (`.ant-layout-sider-trigger`) es una barra de 48 px de alto y todo el ancho, `position: fixed` y `bottom: 0` (`antd@6.6.6`, `layout/style/sider.js:50-58`), y con `collapsible` el `Sider` se reserva `paddingBottom` de esos 48 px (`sider.js:30-31`). Es sitio para un icono, no para tres controles; y al ser `fixed` no lo gobierna el flujo del `Sider`.
- **El conmutador de tema pierde su `marginLeft: 16`** (`mode_switcher.tsx:19`). Dentro del pie esa separación fija descuadra el control, y en el raíl colapsado lo desplaza fuera de sitio; el espaciado pasa a ser responsabilidad del contenedor, así que la cabecera pública —que lo usa— recibe un `gap` para conservar el suyo.
- **El conmutador de tema sigue siendo el `Switch` que es.** No se convierte en botón ni cambia de forma: el pie lo aloja tal cual, y el escenario de que el tema se cambia desde el pie es el mismo que había en la cabecera.
- **Los dos controles de icono declaran su propio `aria-label`.** El botón de cerrar sesión no declara ninguno hoy: su nombre accesible lo aporta el icono, que antd pinta como `<span role="img" aria-label="logout">` (`admin_layout.tsx:140-145`), de modo que el nombre depende de la librería de iconos y no del control. El de colapsar no existe todavía: lo pintaba antd como un `div` sin nombre. Los dos ganan un `aria-label` explícito —el del control, no el del icono—, como ya declara el conmutador de tema (`mode_switcher.tsx:20`).
- **El borrado de los dos formularios es un borrado, no un rediseño.** No se rescata nada de su interior —ni el `Card`, ni las validaciones, ni las claves de traducción—: no los importa nadie y el SSO no los usa. Si algún día hiciera falta un formulario local, volvería a escribirse contra el contrato de entonces, no contra estos restos.
- **El pie de la barra lateral no es pegajoso.** Queda al fondo de la barra, pero si una página es más alta que la ventana baja con el contenido. El disparador nativo de antd era `position: fixed` y este no lo es: hacerlo pegajoso exigiría duplicar en el pie el color de fondo de la barra lateral —un valor de la paleta interna de antd— para que el menú no se transparentara por debajo, y se prefiere no copiar ese color. Se acepta como límite conocido.
- **Los `console.log` de `render` y el `div.demo-logo-vertical` de `admin_layout.tsx` se quedan.** Son restos de la plantilla repartidos por todo el frontend, no de este change; limpiarlos es una tarea aparte y hacerlo solo aquí dejaría el proyecto desigual.
- **El rótulo visible «Must have» no conmuta el interruptor al pulsarlo.** El `Switch` ya expone su nombre accesible con `aria-label`, y un `<label>` no puede asociarse a un botón: convertirlo en área de clic es otra decisión y no se toma aquí.
- **Los tests de la barra fijan el mecanismo, no la forma de declararlo.** Comprueban los tres tokens del `flex` de cada `Col` y las clases de la fila, porque jsdom no calcula layout: la comprobación de los píxeles es del operador. Una implementación equivalente con `flexGrow`/`flexShrink`/`flexBasis` o con una clase CSS no pasaría esos tests; es deliberado y está documentado en la cabecera del bloque.

## Non-goals

- **No se toca el backend.** Las nueve redirecciones del 401 y la ruta `/login` son frontend; el callback del SSO ya devuelve al usuario a `/admin/` (`auth.rs:208`) y ese contrato no cambia.
- **No se añade una ruta comodín (`*`).** Una URL desconocida sigue sin pintar nada, como hoy: lo único que cambia es que `/login`, que existía, redirige a la portada en vez de desaparecer en silencio. Convertir cualquier URL inválida en una redirección a la portada es otra decisión y no se toma aquí.
- **No se toca la cabecera de la parte pública** más allá del `gap` y de que su icono arranque el SSO: el raíl lateral solo existe en el panel, así que ahí no hay pie al que bajar nada.
- **No se cambia la forma del conmutador de tema**, por lo dicho arriba.
- **No se rediseñan los filtros de plantillas**: sus anchos preferidos, sus opciones y su comportamiento se quedan como están; lo único que cambia es que pueden encogerse y que comparten línea con el buscador.
- **No hay cambio de contrato de API.** Ningún endpoint, ningún tipo, ningún esquema.

## Impact

- `frontend/src/App.tsx` — la ruta `/login` fuera y su redirección a `/`
- `frontend/src/pages/public/login_page.tsx` — **se borra**
- `frontend/src/components/signin.tsx` y `frontend/src/components/register.tsx` — **se borran** (212 líneas, cero importaciones)
- `frontend/src/common/auth.ts` — **nuevo**: el helper que arranca el SSO
- `frontend/src/pages/public/home_page.tsx` — el botón de entrada
- `frontend/src/layouts/public_layout.tsx` — el icono arranca el SSO; `gap` en la cabecera
- `frontend/src/layouts/admin_layout.tsx` — sin `Header`; el pie con los tres controles; el `AuthGuard` apunta a `/`
- `frontend/src/components/mode_switcher.tsx` — pierde el `marginLeft` fijo
- `frontend/src/pages/admin/templates_page.tsx` — el buscador entra en la fila de filtros
- las nueve redirecciones del 401: `common/utils.ts`, `dialogs/custom_dialog.tsx`, `dialogs/rule_dialog.tsx`, `dialogs/rate_limit_profile_dialog.tsx`, `pages/admin/settings_page.tsx`, `pages/admin/logs_page.tsx`, `pages/admin/templates_page.tsx`
- tests: `layouts/admin_layout.test.tsx` y `pages/public/home_page.test.tsx` **nuevos**; los del buscador en `pages/admin/templates_page.test.tsx`; la redirección de `/login` en el montaje de la aplicación
- `openspec/specs/app-shell/spec.md` — capability nueva (`ADDED`); al archivar hay que redactar su `Purpose`, no dejarlo en `TBD`
- `openspec/specs/templates/spec.md` — un requisito `MODIFIED` («Templates are presented as a single filterable listing»)
- **Ninguna aserción existente se debilita**: los seis ficheros de test actuales no tocan ni la cabecera, ni la ruta `/login`, ni la línea de filtros de plantillas, ni los dos formularios que se borran —una búsqueda de `Header`, `Sider`, `login`, `Logout`, `signin` y `register` en los `*.test.tsx` solo devuelve `custom_table.test.tsx:293-298`, que son cabeceras de columna de la tabla—
- **Verificación manual en el navegador**: jsdom no calcula layout, así que ni el anclaje del pie ni la línea única de la barra de herramientas se pueden comprobar en los tests; estos fijan el mecanismo (dónde vive cada control, qué estilo lleva, si la fila envuelve) y la comprobación visual es del operador

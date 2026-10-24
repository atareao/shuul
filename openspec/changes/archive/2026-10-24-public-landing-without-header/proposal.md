# Proposal: public-landing-without-header

## Why

**La portada pública arrastra una cabecera que ya no aporta nada.** En `/`, sin sesión, la parte alta de la pantalla la ocupa una franja con dos controles: un botón de inicio de sesión —solo icono— y un conmutador de tema. Los dos son redundantes.

**1. El botón de la cabecera duplica el de la portada.** La portada ya ofrece un botón rotulado —«Sign in with PocketID»— que arranca exactamente el mismo flujo SSO. El icono de la cabecera es una segunda entrada al mismo sitio, sin etiqueta, en una franja aparte.

**2. El conmutador de tema no tiene a quién servir.** El panel de administración no usa el componente `components/mode_switcher.tsx`: su entrada de tema vive en el pie de la barra lateral y está implementada allí mismo. La cabecera pública era su único consumidor, así que al quitarla el componente queda huérfano.

**3. La franja desplaza la portada hacia abajo sin motivo.** La portada ocupa el alto de la ventana por su cuenta, así que la cabecera —64 px más su relleno— solo añade desplazamiento vertical a una pantalla cuyo contenido ya cabe entero.

## What Changes

1. **La envoltura pública pierde su cabecera.** `layouts/public_layout.tsx` deja de pintar el `Header`: se queda con el contenido y el pie. Con él se van el botón de inicio de sesión y el conmutador de tema, y los cuatro `import` que solo servían a la cabecera.
2. **`components/mode_switcher.tsx` se retira.** Era el único consumidor de la cabecera y el panel no lo usa. Un componente que ya no se monta en ningún sitio es código muerto.
3. **La portada deja de forzar el alto de la ventana.** El contenido público pasa a ser un contenedor flexible y la portada ocupa el alto disponible en vez de declarar `100vh`, de modo que quitar la cabecera no deja un desplazamiento residual.
4. **El pie se queda.** La envoltura pública mantiene el pie con el copyright.

## Decisions taken when opening the change

- **Se quita la cabecera entera, no se oculta condicionalmente.** La envoltura pública solo sirve a `/` (y a `/login`, que redirige a `/`). Ni el botón de inicio de sesión ni el conmutador de tema tienen sentido con la sesión abierta —el panel tiene los suyos—, así que una ocultación por estado de sesión añadiría una condición para no cambiar nada en la práctica. Menos código, mismo resultado.
- **El botón de la portada queda como la única entrada al SSO en la parte pública.** Es el que ya está rotulado y el que fija el escenario «La portada ofrece entrar»; el icono de la cabecera desaparece del contrato, con su escenario y su test.
- **El conmutador de tema se retira en vez de reubicarse.** La portada usa el tema por defecto y no ofrece cambiarlo sin entrar al panel. Reubicarlo sería inventar un sitio nuevo para un control que nadie ha pedido en esa pantalla.
- **El pie de la portada se conserva.** No forma parte de lo pedido y no molesta.
- **El contenido público pasa a ser un contenedor flexible.** Sin la cabecera, la portada seguía declarando el alto de la ventana dentro de un área de contenido con relleno, así que la página conservaba una barra de desplazamiento de unos cien píxeles para un contenido que cabe entero. La portada crece ahora dentro del área de contenido en vez de imponerle su alto.
- **El requisito de la portada se retira y se sustituye, en vez de modificarse.** El validador de OpenSpec rechaza que un bloque MODIFIED pierda un escenario, y el escenario «El icono de la cabecera pública arranca el SSO» tiene que desaparecer con la cabecera. Se retira el requisito entero y lo sustituyen dos: uno para la envoltura sin cabecera y otro para la portada como única entrada al SSO.

## Límites que se aceptan

- **No se toca el `token` de la envoltura.** El layout sigue leyendo `coloBgLayout` —la clave heredada, en la práctica `undefined`— y `colorText` del token de antd, tal como están documentados en el fichero. Es una rareza preexistente, no de este change.
- **jsdom no calcula layout.** Que la portada quepa sin desplazarse no se puede comprobar en los tests —no hay motor de maquetación—, así que queda como comprobación manual del operador.

## Impact

- **Frontend**: `layouts/public_layout.tsx` (el grueso del change), `pages/public/home_page.tsx` (deja de forzar el alto de la ventana), `components/mode_switcher.tsx` (se retira) y la reescritura del test de la envoltura pública.
- **Spec**: `app-shell` — se retira el requisito «La portada pública es la entrada al panel», que exigía que el icono de la cabecera arrancase el SSO, y lo sustituyen dos requisitos: «La parte pública no pinta cabecera» y «La portada es la única entrada al SSO en la parte pública».
- **Fuera de alcance**: backend, WAF, Jail, `Dockerfile`, workflows, `compose.yaml` y el panel de administración. No hay migración de datos ni cambio de API.

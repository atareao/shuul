# app-shell Specification

## Purpose

El armazón que envuelven todas las páginas: la envoltura pública con su cabecera, y el
panel de administración con su barra lateral —navegación, sesión, tema y colapso— y sin
cabecera superior. Cubre además las dos entradas al panel: sin sesión se acaba en la
portada pública, y es la portada la que arranca el flujo SSO.

## Requirements

### Requirement: La sesión cerrada devuelve a la portada pública

NO SHALL existir una ruta `/login`, ni una página de inicio de sesión propia, ni un formulario de login local: la aplicación NO SHALL tener una pantalla intermedia entre el usuario sin sesión y el botón que arranca el SSO.

Sin sesión, el panel de administración SHALL devolver al usuario a la portada pública (`/`). Eso SHALL valer para los tres caminos que hoy desembocan en la ruta eliminada: la navegación a una ruta del panel sin sesión, el cierre de sesión y cualquier respuesta `401` de la API.

Una visita directa a `/login` —un enlace o marcador anterior— SHALL redirigir a `/`, y NO SHALL dejar la pantalla en blanco.

#### Scenario: Sin sesión, el panel devuelve a la portada
- **WHEN** un usuario sin sesión abre `/admin/`
- **THEN** SHALL acabar en `/` y NO SHALL ver ninguna pantalla de inicio de sesión propia

#### Scenario: Cerrar sesión deja al usuario en la portada
- **WHEN** el usuario cierra sesión desde el panel
- **THEN** la sesión SHALL quedar cerrada, con el token fuera de `localStorage`
- **AND** el usuario SHALL acabar en `/`

#### Scenario: Una respuesta 401 devuelve a la portada
- **WHEN** cualquier llamada a la API responde `401`
- **THEN** el navegador SHALL navegar a `/`
- **AND** NO SHALL navegar a `/login`

#### Scenario: Un enlace viejo a /login no deja la pantalla en blanco
- **WHEN** el usuario abre `/login` directamente
- **THEN** SHALL acabar en `/`

#### Scenario: Los formularios de login local ya no existen
- **WHEN** se inspecciona el frontend
- **THEN** NO SHALL existir `components/signin.tsx` ni `components/register.tsx`
- **AND** NO SHALL quedar ningún tipo de credenciales locales, con email y contraseña, en el código

### Requirement: La portada pública es la entrada al panel

La portada pública SHALL ofrecer un botón rotulado que arranque el flujo SSO, y ese botón SHALL usar el mismo mecanismo que la página de inicio de sesión eliminada: una navegación a `{BASE_URL}/api/v1/auth/sso`.

El icono de la cabecera pública SHALL arrancar el mismo flujo, sin pasar por ninguna ruta intermedia.

La URL del endpoint de SSO SHALL componerse en un único punto del frontend.

#### Scenario: La portada ofrece entrar
- **WHEN** un usuario sin sesión abre `/`
- **THEN** SHALL ver un botón rotulado para iniciar sesión
- **AND** al pulsarlo SHALL navegar al endpoint de SSO, `{BASE_URL}/api/v1/auth/sso`

#### Scenario: El icono de la cabecera pública arranca el SSO
- **WHEN** el usuario pulsa el icono de la cabecera de la parte pública
- **THEN** SHALL navegar al endpoint de SSO
- **AND** NO SHALL pasar por una ruta intermedia

#### Scenario: La URL del SSO vive en un solo sitio
- **WHEN** se inspecciona el frontend
- **THEN** SHALL existir un único punto que componga `{BASE_URL}/api/v1/auth/sso`
- **AND** NO SHALL existir `pages/public/login_page.tsx`

### Requirement: El panel de administración no tiene barra superior

El layout del panel NO SHALL pintar una cabecera superior: NO SHALL existir en su árbol un elemento con la clase `ant-layout-header`.

El contenido del panel SHALL empezar en la parte alta del área de contenido, sin la franja que ocupaba la cabecera.

#### Scenario: El panel no pinta cabecera
- **WHEN** se renderiza cualquier página del panel
- **THEN** NO SHALL existir un elemento con la clase `ant-layout-header`
- **AND** SHALL seguir existiendo la barra lateral y el pie de página

### Requirement: El pie de la barra lateral gobierna la sesión, el tema y el colapso

El cierre de sesión, el cambio de tema y el colapso de la barra lateral SHALL gobernarse desde un bloque de entradas al pie de la barra lateral, y esas entradas SHALL tener la misma forma que las de navegación: icono y etiqueta, una por línea.

Las tres entradas SHALL rotularse en inglés, como el resto de la navegación: «Theme», «Log out» y «Collapse».

La entrada de tema SHALL cambiar el tema al pulsarla —NO SHALL ser un interruptor— y su icono SHALL reflejar el tema activo.

La entrada de colapso SHALL alternar el estado de la barra lateral y SHALL anunciarlo con `aria-expanded`.

Con la barra lateral colapsada, las tres entradas SHALL mostrar solo su icono, igual que las entradas de navegación.

El bloque SHALL quedar siempre a la vista: la barra lateral SHALL permanecer pegada a la ventana, de modo que el bloque NO SHALL quedar por debajo del pliegue ni siquiera en las páginas más altas que la ventana, y el menú de navegación SHALL desplazarse dentro de la barra.

El bloque NO SHALL contener el menú de navegación.

#### Scenario: Las tres entradas viven al pie, con forma de menú
- **WHEN** se renderiza cualquier página del panel
- **THEN** las tres entradas SHALL estar dentro de la barra lateral
- **AND** SHALL compartir un bloque al pie de la barra
- **AND** SHALL tener la forma de las entradas de navegación, con icono y etiqueta
- **AND** ese bloque NO SHALL contener el menú de navegación

#### Scenario: Las etiquetas están en inglés
- **WHEN** se renderiza el bloque del pie
- **THEN** las tres entradas SHALL rotularse «Theme», «Log out» y «Collapse»

#### Scenario: El pie se ve también en las páginas largas
- **WHEN** se abre una página del panel más alta que la ventana
- **THEN** la barra lateral SHALL permanecer pegada a la ventana
- **AND** el bloque del pie SHALL seguir a la vista, sin quedar por debajo del pliegue
- **AND** el menú de navegación SHALL desplazarse dentro de la barra

#### Scenario: El tema se cambia desde el pie
- **WHEN** el usuario pulsa la entrada de tema
- **THEN** el tema de la aplicación SHALL cambiar
- **AND** el icono de la entrada SHALL reflejar el tema activo

#### Scenario: La barra lateral se colapsa desde el pie
- **WHEN** el usuario pulsa la entrada de colapso
- **THEN** la barra lateral SHALL colapsar o desplegarse, alternando en cada pulsación
- **AND** la entrada SHALL anunciar el estado de la barra con `aria-expanded`

#### Scenario: Las entradas se pliegan con la barra
- **WHEN** la barra lateral está colapsada
- **THEN** las tres entradas SHALL mostrar solo su icono
- **AND** NO SHALL desbordar el ancho del raíl

### Requirement: La barra lateral lleva la marca de la aplicación

La barra lateral SHALL mostrar en su parte alta, por encima de la navegación, el icono de la aplicación y el nombre «Shuul».

Con la barra lateral colapsada SHALL mostrarse solo el icono.

La marca NO SHALL ser una entrada de navegación ni SHALL ocupar el lugar de ninguna.

#### Scenario: La marca encabeza la barra
- **WHEN** se renderiza cualquier página del panel
- **THEN** SHALL verse el icono de la aplicación y el nombre «Shuul» en la parte alta de la barra lateral
- **AND** SHALL estar por encima de la primera entrada de navegación

#### Scenario: Con la barra colapsada solo queda el icono
- **WHEN** la barra lateral está colapsada
- **THEN** SHALL verse el icono de la aplicación
- **AND** NO SHALL verse el nombre

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

### Requirement: La sesión y el tema se gobiernan desde el pie de la barra lateral

El control de cerrar sesión, el conmutador de tema y el control de colapsar/descolapsar la barra lateral SHALL vivir en un mismo contenedor, dentro de la barra lateral y anclado a su parte inferior.

Los tres controles SHALL colocarse en una fila cuando la barra lateral está desplegada y en columna cuando está colapsada, de modo que quepan en el ancho del raíl.

Cada control SHALL declarar su propio nombre accesible con `aria-label`, porque los de cerrar sesión y colapsar son botones de solo icono y su nombre NO SHALL depender del que aporte el icono que lleven dentro.

El conmutador de tema NO SHALL declarar márgenes propios: el espaciado SHALL ser responsabilidad del contenedor en el que se monte.

#### Scenario: Los tres controles viven en el pie de la barra lateral
- **WHEN** se renderiza cualquier página del panel
- **THEN** el botón de cerrar sesión, el conmutador de tema y el control de colapsar SHALL estar dentro de la barra lateral
- **AND** los tres SHALL compartir el mismo contenedor, anclado a la parte inferior de la barra

#### Scenario: Una fila desplegada, una columna colapsada
- **WHEN** la barra lateral está desplegada
- **THEN** los tres controles SHALL colocarse en una fila
- **AND** al colapsar la barra SHALL colocarse en columna

#### Scenario: Cada control declara su nombre accesible
- **WHEN** se renderiza el pie de la barra lateral
- **THEN** el botón de cerrar sesión SHALL declarar su propio `aria-label`, no el del icono
- **AND** el control de colapsar SHALL declarar su propio `aria-label`, no el del icono
- **AND** el conmutador de tema SHALL seguir declarando el suyo

#### Scenario: El tema se cambia desde el pie
- **WHEN** el usuario conmuta el tema desde el pie de la barra lateral
- **THEN** el tema de la aplicación SHALL cambiar, como cambiaba desde la cabecera

#### Scenario: La barra lateral se colapsa desde el pie
- **WHEN** el usuario pulsa el control de colapsar
- **THEN** la barra lateral SHALL colapsar o desplegarse, alternando en cada pulsación

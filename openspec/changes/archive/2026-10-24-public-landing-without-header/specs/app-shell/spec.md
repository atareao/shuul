# app-shell Specification (delta)

## REMOVED Requirements

### Requirement: La portada pública es la entrada al panel

**Reason**: El requisito exigía que el icono de la cabecera pública arrancase el flujo SSO, y esa cabecera desaparece entera: con ella se va el icono, así que la exigencia deja de tener sujeto. Lo que el requisito pedía y sigue vigente —que la portada sea la entrada al panel, con su botón rotulado, y que la URL del SSO se componga en un solo punto— se conserva, reescrito en los requisitos que añade este delta.

**Migration**: Lo sustituyen dos requisitos de este mismo delta: «La parte pública no pinta cabecera», para la envoltura, y «La portada es la única entrada al SSO en la parte pública», que recoge los dos escenarios que se conservan.

## ADDED Requirements

### Requirement: La parte pública no pinta cabecera

La envoltura pública NO SHALL pintar una cabecera superior: NO SHALL existir en su árbol un elemento con la clase `ant-layout-header`.

La envoltura pública NO SHALL ofrecer un control de inicio de sesión propio.

El contenido público SHALL empezar en la parte alta del área de contenido, sin la franja que ocupaba la cabecera.

La portada SHALL ocupar el alto disponible del área de contenido, y NO SHALL imponer el alto de la ventana.

El pie de la parte pública SHALL conservarse.

#### Scenario: La parte pública no pinta cabecera
- **WHEN** se renderiza cualquier página de la parte pública
- **THEN** NO SHALL existir un elemento con la clase `ant-layout-header`
- **AND** NO SHALL existir un control de inicio de sesión en la envoltura
- **AND** SHALL seguir existiendo el pie de página

#### Scenario: El contenido público empieza arriba
- **WHEN** un usuario sin sesión abre `/`
- **THEN** el contenido de la portada SHALL empezar en la parte alta del área de contenido, sin la franja que ocupaba la cabecera

#### Scenario: La portada no fuerza el alto de la ventana
- **WHEN** un usuario sin sesión abre `/`
- **THEN** la portada SHALL ocupar el alto disponible del área de contenido
- **AND** NO SHALL provocar desplazamiento vertical por sí sola

### Requirement: La portada es la única entrada al SSO en la parte pública

La portada pública SHALL ofrecer un botón rotulado que arranque el flujo SSO, y ese botón SHALL usar el mismo mecanismo que la página de inicio de sesión eliminada: una navegación a `{BASE_URL}/api/v1/auth/sso`.

Ese botón SHALL ser la única entrada al flujo SSO en la parte pública.

La URL del endpoint de SSO SHALL componerse en un único punto del frontend.

#### Scenario: La portada ofrece entrar
- **WHEN** un usuario sin sesión abre `/`
- **THEN** SHALL ver un botón rotulado para iniciar sesión
- **AND** al pulsarlo SHALL navegar al endpoint de SSO, `{BASE_URL}/api/v1/auth/sso`

#### Scenario: La URL del SSO vive en un solo sitio
- **WHEN** se inspecciona el frontend
- **THEN** SHALL existir un único punto que componga `{BASE_URL}/api/v1/auth/sso`
- **AND** NO SHALL existir `pages/public/login_page.tsx`

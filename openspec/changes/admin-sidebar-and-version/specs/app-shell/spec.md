# app-shell Specification (delta)

## REMOVED Requirements

### Requirement: La sesión y el tema se gobiernan desde el pie de la barra lateral

**Reason**: El bloque del pie deja de ser una fila de controles —el tema, un interruptor— y pasa a ser un bloque de entradas con la forma del menú de navegación, y la barra lateral se pega a la ventana para que el pie no quede por debajo del pliegue ni en las páginas más altas que la ventana. El requisito se reescribe entero, y con él el escenario que describía la disposición en fila y en columna, que desaparece.

**Migration**: Lo sustituye el requisito «El pie de la barra lateral gobierna la sesión, el tema y el colapso», en este mismo delta.

## ADDED Requirements

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

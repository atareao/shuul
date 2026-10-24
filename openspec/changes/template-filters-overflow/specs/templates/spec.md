# templates Specification (delta)

## ADDED Requirements

### Requirement: La previsualización de una plantilla no desborda y ofrece el valor completo

Los tags que muestran el valor de un filtro de plantilla SHALL truncar su contenido con elipsis cuando no quepa en el ancho disponible, en vez de desbordar su contenedor. El tag SHALL poder encogerse hasta el ancho del contenedor y NO SHALL fijar un ancho propio en píxeles.

El valor completo SHALL seguir siendo accesible: cada uno de esos tags SHALL ir dentro de un `Tooltip` de antd cuyo `title` sea el valor íntegro, de modo que el texto recortado en pantalla pueda leerse entero.

El texto completo SHALL seguir presente en el DOM del tag: el truncado SHALL ser visual (CSS), no SHALL recortar el valor con JavaScript.

Esto SHALL cumplirse en los dos sitios que muestran los filtros de una plantilla: la previsualización del diálogo de aplicar plantilla —filtros y scope— y el detalle expandible de una fila del listado.

#### Scenario: Un path de 314 caracteres no desborda el diálogo de aplicar plantilla
- **WHEN** el usuario abre el diálogo de aplicar la plantilla `Scanner directorios+configs+paneles`, cuyo `path` mide 314 caracteres
- **THEN** el tag del path SHALL llevar el estilo que trunca su contenido (`max-width: 100%`, `overflow: hidden`, `text-overflow: ellipsis` y `white-space: nowrap`)

#### Scenario: El valor recortado se puede leer entero
- **WHEN** el usuario pasa el ratón por el tag del path de esa plantilla
- **THEN** SHALL aparecer un tooltip cuyo texto sea el regex completo, sin recortar

#### Scenario: Los filtros cortos se ven como antes
- **WHEN** el usuario abre el diálogo de aplicar una plantilla cuyos filtros caben en una línea
- **THEN** cada tag SHALL mostrar su valor completo en una sola línea, sin elipsis visible
- **AND** el texto del tag SHALL seguir conteniendo el valor íntegro

#### Scenario: El detalle expandible de una fila se comporta igual
- **WHEN** el usuario expande la fila de una plantilla con un filtro largo
- **THEN** los tags del detalle SHALL llevar el mismo estilo que los del diálogo
- **AND** cada uno SHALL ir dentro de un `Tooltip` con su valor completo

#### Scenario: El scope del diálogo también lo usa
- **WHEN** el usuario escribe un FQDN largo en el diálogo de aplicar plantilla
- **THEN** el tag del scope SHALL llevar el mismo estilo que los de los filtros
- **AND** SHALL ir dentro de un `Tooltip` con su valor completo

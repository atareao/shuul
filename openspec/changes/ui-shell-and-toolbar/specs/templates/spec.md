# templates Specification (delta)

## MODIFIED Requirements

### Requirement: Templates are presented as a single filterable listing

La biblioteca de plantillas SHALL presentarse como **un único listado tabular**, sin
pestañas ni acordeones, con una fila por plantilla de regla y columnas de nombre,
pipeline, categoría, severidad, acción, perfil de rate limit (solo JAIL) y una acción
de aplicación.

El listado SHALL ofrecer filtros por pipeline, severidad, categoría y acción, además
de la búsqueda por texto. La condición `must_have` SHALL ser **un filtro**, no un
grupo, sección o acordeón.

La búsqueda por texto SHALL compartir línea con los cuatro filtros y con el interruptor
`must_have`: la barra de herramientas SHALL ser **una sola línea**, y el buscador NO
SHALL ocupar una línea propia a ancho completo. La línea NO SHALL envolver: antes de
partirse, los controles SHALL encogerse. El buscador SHALL ser el elemento flexible:
SHALL declarar su ancho preferido, crecer por encima de él para ocupar el espacio libre
y ceder espacio encogiéndose cuando falte. Los filtros SHALL declarar su ancho preferido
y SHALL poder encogerse también. Ningún control SHALL fijar un ancho propio en píxeles
ni un mínimo rígido: el ancho efectivo SHALL ser el que reparta la fila, de modo que la
línea nunca desborde su contenedor.

El detalle de los filtros de una plantilla SHALL mostrarse en la propia fila
(expandible) y NO SHALL ocupar el espacio de la lista con etiquetas de resumen.

El orden por defecto del listado SHALL respetar el orden del catálogo: primero las
plantillas `must_have` y después el resto por peso.

#### Scenario: All rule templates appear in one list
- **WHEN** el usuario abre la biblioteca de plantillas
- **THEN** SHALL ver 42 filas en un único listado (24 WAF y 18 JAIL)
- **AND** NO SHALL haber pestañas que separen WAF de JAIL
- **AND** NO SHALL haber ninguna sección ni acción de perfiles de rate limit en la página

#### Scenario: Filtering by pipeline
- **WHEN** el usuario filtra por pipeline JAIL
- **THEN** SHALL listarse solo las 18 plantillas JAIL
- **AND** al quitar el filtro SHALL volver a verse el catálogo completo

#### Scenario: Filtering by severity
- **WHEN** el usuario filtra por severidad `"critico"`
- **THEN** SHALL listarse solo las plantillas con `severity: "critico"`

#### Scenario: Must-have is a filter
- **WHEN** el usuario activa el filtro "must have"
- **THEN** SHALL listarse las 16 plantillas con `must_have: true`
- **AND** el catálogo completo SHALL seguir accesible desactivando el filtro

#### Scenario: Filters combine with free-text search
- **WHEN** el usuario filtra por pipeline JAIL y escribe "scanner" en la búsqueda
- **THEN** SHALL listarse solo las plantillas JAIL cuyo nombre o descripción contenga "scanner"

#### Scenario: The search box shares the line with the filters
- **WHEN** el usuario abre la biblioteca de plantillas
- **THEN** el buscador, los cuatro filtros y el interruptor `must_have` SHALL estar en la misma fila
- **AND** esa fila NO SHALL envolver

#### Scenario: The search box takes the free space
- **WHEN** la ventana es más ancha que la suma de los anchos preferidos de los controles
- **THEN** el buscador SHALL crecer para ocupar el hueco sobrante

#### Scenario: The row shrinks before it wraps
- **WHEN** la ventana se estrecha
- **THEN** los controles SHALL encogerse por debajo de su ancho preferido, cediendo espacio
- **AND** la fila NO SHALL partirse en dos líneas
- **AND** el contenido NO SHALL salirse de su contenedor

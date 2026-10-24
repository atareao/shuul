# table-pagination Specification

## Purpose
TBD - created by archiving change pagination-defaults. Update Purpose after archive.

## Requirements

### Requirement: Toda tabla paginada arranca en 10 filas y ofrece los rangos 10, 50 y 100

Toda tabla paginada de la administración SHALL arrancar en **10 filas por página** y SHALL
ofrecer exactamente los rangos **10, 50 y 100** en su selector de tamaño de página, con
independencia de si pagina en servidor (`CustomTable`: reglas, baneos y perfiles de rate limit)
o en cliente (el listado de plantillas y el Log Viewer).

El listado de plantillas SHALL paginar en cliente —el endpoint sirve el catálogo completo y el
filtrado es local— y SHALL volver a la primera página cuando cambie cualquier filtro o la
búsqueda, para no dejar al usuario en una página que el conjunto filtrado ya no alcanza.

#### Scenario: Default page size on a server-paginated table
- **GIVEN** la página de reglas
- **WHEN** se carga por primera vez
- **THEN** la tabla muestra 10 filas
- **AND** el selector de tamaño de página ofrece 10, 50 y 100

#### Scenario: The templates listing is paginated at 10
- **GIVEN** la biblioteca de plantillas con sus 42 plantillas (24 WAF y 18 JAIL)
- **WHEN** se carga por primera vez
- **THEN** el listado muestra 10 filas
- **AND** el selector de tamaño de página ofrece 10, 50 y 100
- **AND** la última página muestra las 2 plantillas restantes

#### Scenario: The Log Viewer starts at 10 and keeps the same ranges
- **GIVEN** la página del Log Viewer
- **WHEN** se carga por primera vez
- **THEN** la tabla muestra 10 entradas
- **AND** el selector de tamaño de página ofrece 10, 50 y 100

#### Scenario: Filtering the templates listing returns to the first page
- **GIVEN** el listado de plantillas situado en su última página
- **WHEN** el usuario escribe en la búsqueda o cambia un filtro
- **THEN** el listado vuelve a la primera página
- **AND** la tabla no queda vacía por un número de página fuera de rango

### Requirement: Server-paginated tables paginate exactly once

Las tablas que paginan en servidor (`CustomTable`) SHALL mostrar **exactamente** las
filas de la página solicitada. El componente SHALL ser el único paginador: su
`dataSource` SHALL ser la página devuelta por el servidor y antd NO SHALL volver a
recortarla.

El tamaño de página SHALL ser propiedad del cliente y SHALL NOT reescribirse con el
valor que devuelva el servidor. El total de registros y el número de páginas SHALL
proceder de la respuesta del servidor y SHALL reflejar **esa** respuesta.

El componente NO SHALL aplicar ningún filtrado local antes de paginar: cualquier
filtro de una vista SHALL viajar al servidor como parámetro de consulta.

#### Scenario: A page shows exactly the requested page size
Given the server holds 34 records
And the page size is 10
When the user opens the first page
Then the table shows 10 rows
When the user moves to the second page
Then the table shows 10 rows
And the page size is still 10

#### Scenario: The last page shows only the remaining rows
Given the server holds 34 records
And the page size is 10
When the user opens the fourth page
Then the table shows 4 rows

#### Scenario: Server totals are reflected, not the previous ones
Given the server holds 34 records
When the user opens any page
Then the reported total is 34
And the reported page count is 4

#### Scenario: Changing the page size keeps the new size
Given the user is on the third page with a page size of 10
When the user changes the page size to 50
Then the table shows the first page of 50 records
And the page size is still 50 after the server responds

### Requirement: Sorting sends the direction to the server

El sentido de ordenación SHALL viajar a la API: el componente SHALL enviar el
parámetro de orden junto a `sort_by`, de modo que ordenar una columna en sentido
descendente consulte al servidor en sentido descendente. Cambiar de columna SHALL
volver a la primera página.

#### Scenario: Descending sort reaches the API
Given the user sorts by `ip_address` in descending order
When the component queries the server
Then the request carries `sort_by=ip_address`
And the request carries `asc=false`
And the table shows the first page of that order

### Requirement: Client-side tables are paginated by the client only

La página Log Viewer SHALL paginar en cliente: pide el buffer completo y pagina
sobre el conjunto ya filtrado localmente. NO SHALL aplicar el contrato de paginación
en servidor de `CustomTable`, porque no hay servidor que pagine.

#### Scenario: Hiding events re-paginates the remaining entries
Given the buffer holds 120 entries and the page size is 50
When the user hides an event type, leaving 80 entries
Then the page shows 50 of those 80 entries
And the reported total is 80

## ADDED Requirements

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

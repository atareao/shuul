# rule-matching Specification (delta)

## ADDED Requirements

### Requirement: El matcheo recorre un conjunto de condiciones proyectadas desde la regla

El matcher SHALL evaluar cada regla como un **conjunto ordenado de condiciones**. Una condición SHALL declarar la colección de la petición que restringe, la pipeline de transformaciones que se aplica a su valor antes de compararlo y el patrón con el que se compara.

Una regla SHALL casar cuando **todas** sus condiciones casen. El conjunto SHALL evaluarse en AND y **con cortocircuito**: la primera condición que no case SHALL detener la evaluación, de modo que el resultado y su evidencia describan la decisión que de verdad se tomó.

Una regla que **no** declare ninguna condición SHALL ser **neutral**: NO SHALL impedir el match.

El contrato de cada condición SHALL ser el mismo que el de los tres brazos del matcheo actual:

- la colección restringida y presente → decide el patrón, aplicado al valor **transformado** por su pipeline;
- la colección restringida y **ausente** → la condición NO SHALL cumplirse y SHALL emitirse el `debug!` que nombra la regla y la colección;
- una colección que la regla **no** restringe → no genera condición: es neutral.

El chequeo de `is_tor` NO SHALL formar parte del conjunto de condiciones: NO SHALL transformarse, NO SHALL compararse con un patrón, y SHALL seguir siendo el predicado booleano que decide por sí solo. Se modelará como colección cuando la lógica booleana lo permita.

Este contrato SHALL ser el mismo para los dos pipelines: el WAF y el Jail comparten el matcher.

#### Scenario: Una regla con varias condiciones casa solo si todas casan
- **GIVEN** una regla con una condición sobre `path` y otra sobre `user_agent`, y una petición que trae ambas y casa las dos
- **THEN** la regla SHALL casar
- **AND** con la misma regla y una petición cuyo `user_agent` no case, la regla NO SHALL casar

#### Scenario: Una regla sin condiciones es neutral
- **GIVEN** una regla que no restringe ninguno de los 14 campos
- **THEN** su conjunto de condiciones SHALL estar vacío
- **AND** la regla SHALL casar cualquier petición

#### Scenario: La evaluación corta en la primera condición que no casa
- **GIVEN** una regla con una condición sobre `path` que casa y otra sobre `user_agent` que no casa
- **THEN** la regla NO SHALL casar
- **AND** el resultado SHALL ser el mismo que el de la cadena de comprobaciones actual

#### Scenario: Una colección restringida y ausente sigue sin casar
- **GIVEN** una regla con una condición sobre `user_agent` y una petición sin esa cabecera
- **THEN** la condición NO SHALL cumplirse
- **AND** la regla NO SHALL casar
- **AND** SHALL emitirse el `debug!` que nombra la colección y la regla

#### Scenario: `is_tor` sigue fuera de las condiciones
- **GIVEN** una regla con `is_tor` marcado y una petición que no es Tor
- **THEN** la regla NO SHALL casar
- **AND** la regla SHALL casar la misma petición marcada como Tor

### Requirement: La proyección de los 14 campos a condiciones es total y equivalente

Todo campo de los 14 (`ip_address`, `protocol`, `fqdn`, `path`, `query`, `city_name`, `country_name`, `country_code`, `user_agent`, `method`, `referer`, `content_type`, `accept_language`, `x_request_id`) que la regla **restrinja** SHALL proyectarse a **exactamente una** condición. Un campo cuya condición almacenada esté **en blanco** NO SHALL proyectarse a ninguna condición.

La proyección SHALL ser **total**: ningún campo de los 14 SHALL quedar sin representación posible. Y SHALL ser **equivalente**: el resultado del matcheo y la forma del evento de auditoría SHALL ser **idénticos** a los de la cadena de comprobaciones que sustituye.

La clave con la que la evidencia anota el valor transformado SHALL seguir siendo el **nombre del campo** tal como el evento lo usa hoy —`path`, `user_agent`…—, NO SHALL renombrarse a un vocabulario de colección.

#### Scenario: Cada campo restringido genera su condición
- **GIVEN** una regla para cada uno de los 14 campos, con ese campo restringido
- **THEN** cada regla SHALL proyectar **una** condición
- **AND** la condición SHALL llevar el patrón que la regla tenía en ese campo

#### Scenario: Un campo en blanco no proyecta condición
- **GIVEN** una regla con `path` restringido y `query` en blanco
- **THEN** la proyección SHALL contener solo la condición de `path`
- **AND** la regla SHALL evaluar `query` como neutral

#### Scenario: La equivalencia se sostiene sobre las plantillas
- **GIVEN** cada plantilla del catálogo proyectada a condiciones
- **WHEN** se evalúa contra las mismas peticiones que antes
- **THEN** las decisiones SHALL ser las mismas que con la cadena de comprobaciones
- **AND** la evidencia SHALL tener el mismo contenido y la misma forma

#### Scenario: La evidencia conserva el nombre del campo
- **GIVEN** una regla con pipeline no vacía sobre `path`
- **WHEN** casa
- **THEN** la evidencia SHALL anotarse bajo la clave `path`
- **AND** NO SHALL anotarse bajo un nombre de colección

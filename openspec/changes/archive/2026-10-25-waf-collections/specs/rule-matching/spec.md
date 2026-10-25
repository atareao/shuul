## ADDED Requirements

### Requirement: El matcheo evalúa las colecciones nuevas con clave

El matcheo SHALL evaluar las colecciones nuevas con las mismas reglas que las 14 actuales: la colección presente decide, la ausente no casa —con su diagnóstico de depuración que nombra la colección y la regla—, y la pipeline que la condición declare se aplica al valor antes de compararlo.

Las colecciones **con clave** (`request_cookies`, `request_headers`, `args_get`) SHALL resolver el valor de la entrada cuyo nombre es la clave de la condición. Una condición con clave SHALL exigir que la colección tenga una entrada con ese nombre; si no la tiene, la colección SHALL considerarse ausente y la condición NO SHALL casar.

La evidencia de una condición con clave SHALL usar una clave **compuesta** `colección[clave]` (por ejemplo `request_cookies[PHPSESSID]`) para no colisionar con otra condición de la misma colección. La evidencia de las 14 colecciones actuales y de `request_basename` y `request_extension` SHALL conservar su clave actual (el nombre de la colección).

#### Scenario: Una condición de cookie casa por su nombre

- **GIVEN** una regla con una condición de la colección `request_cookies` y clave `PHPSESSID`
- **WHEN** la petición trae la cookie `PHPSESSID=abc`
- **THEN** la condición SHALL evaluar el valor `abc` contra el patrón

#### Scenario: Una cookie ausente no casa y se diagnostica

- **GIVEN** una regla con una condición de la colección `request_cookies` y clave `PHPSESSID`
- **WHEN** la petición no trae esa cookie
- **THEN** la condición NO SHALL casar
- **AND** SHALL emitirse un diagnóstico que nombre la colección y la regla

#### Scenario: La evidencia distingue dos condiciones de la misma colección

- **GIVEN** una regla con dos condiciones de la colección `request_headers` con claves distintas y pipeline no vacía
- **WHEN** ambas casan
- **THEN** la evidencia SHALL contener una entrada por clave, como `request_headers[authorization]` y `request_headers[x-api-key]`

#### Scenario: La decisión del modo simple no cambia

- **GIVEN** una regla descrita con los 14 campos actuales
- **THEN** su resultado de matcheo y su evidencia SHALL ser idénticos a los de F2b-1

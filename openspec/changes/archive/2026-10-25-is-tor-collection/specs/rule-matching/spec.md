## MODIFIED Requirements

### Requirement: El matcheo recorre un conjunto de condiciones proyectadas desde la regla

El matcher SHALL evaluar cada regla como un **conjunto ordenado de condiciones**. Una condición SHALL declarar la colección de la petición que restringe, la pipeline de transformaciones que se aplica a su valor antes de compararlo y el patrón con el que se compara.

Una regla SHALL casar cuando **todas** sus condiciones casen. El conjunto SHALL evaluarse en AND y **con cortocircuito**: la primera condición que no case SHALL detener la evaluación, de modo que el resultado y su evidencia describan la decisión que de verdad se tomó.

Una regla que **no** declare ninguna condición SHALL ser **neutral**: NO SHALL impedir el match.

El contrato de cada condición SHALL ser el mismo que el de los tres brazos del matcheo actual:

- la colección restringida y presente → decide el patrón, aplicado al valor **transformado** por su pipeline;
- la colección restringida y **ausente** → la condición NO SHALL cumplirse y SHALL emitirse el `debug!` que nombra la regla y la colección;
- una colección que la regla **no** restringe → no genera condición: es neutral.

La colección booleana `is_tor` SHALL formar parte del conjunto de condiciones como una más, evaluada en AND y con cortocircuito con las demás. Su resolución SHALL ser la del booleano de la petición, no la de `request_value`.

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

## ADDED Requirements

### Requirement: La columna `is_tor` se proyecta a una condición booleana

La proyección desde la representación simple SHALL incluir la columna `is_tor`: cuando `is_tor` sea `Some(true)` SHALL proyectar **exactamente una** condición de la colección `is_tor`, con operador `@truthy`, al final del conjunto —después de las condiciones de los 14 campos—. `Some(false)` y `None` NO SHALL proyectar ninguna condición: son **neutrales**, igual que la clausura histórica, que sólo decide con `Some(true)`.

Proyectar el `false` a una condición invertiría el sentido de la regla y bloquearía todo el tráfico que no venga de Tor, así que el caso `Some(false)` SHALL cubrirse explícitamente.

#### Scenario: `is_tor` verdadero proyecta una condición
- **GIVEN** una regla con `is_tor = Some(true)` y dos campos restringidos
- **THEN** la proyección SHALL contener las dos condiciones de los campos
- **AND** SHALL contener una tercera condición `is_tor` con operador `@truthy`, la última

#### Scenario: `is_tor = Some(false)` es neutral y no proyecta
- **GIVEN** una regla con `is_tor = Some(false)`
- **THEN** la proyección NO SHALL contener ninguna condición de `is_tor`

#### Scenario: `is_tor` ausente no proyecta
- **GIVEN** una regla con `is_tor = None`
- **THEN** la proyección NO SHALL contener ninguna condición de `is_tor`

### Requirement: El matcheo evalúa la colección booleana `is_tor`

La colección `is_tor` SHALL resolver su valor desde el booleano `is_tor` de la petición, no desde `request_value`, que devuelve texto. El operador `@truthy` SHALL cumplirse **si y solo si** el booleano de la petición es exactamente `true`.

`Some(true)` SHALL cumplir la condición. `Some(false)` y la ausencia (`None`) NO SHALL cumplirla y SHALL emitir el `debug!` que nombra la colección y la regla: la ausencia es el brazo fail-open, hermano del de los campos ausentes, e incluye el caso de un `TorService` sin cargar o de una dirección que no parsea como IP.

La pipeline de la condición SHALL ser, por contrato, **vacía**: un booleano no se transforma. La colección NO SHALL anotar **evidencia** —decisión explícita, revisable—, de modo que la forma del evento de auditoría NO SHALL cambiar.

#### Scenario: Una petición de Tor cumple la condición
- **GIVEN** una regla con una condición `is_tor`/`@truthy` y una petición con `is_tor = Some(true)`
- **THEN** la condición SHALL cumplirse

#### Scenario: Una petición que no es Tor no cumple
- **GIVEN** la misma regla y una petición con `is_tor = Some(false)`
- **THEN** la condición NO SHALL cumplirse
- **AND** la regla NO SHALL casar
- **AND** SHALL emitirse el `debug!` que nombra la colección y la regla

#### Scenario: La ausencia del dato es fail-open y diagnostica
- **GIVEN** la misma regla y una petición con `is_tor = None` (TorService sin cargar o IP irresoluble)
- **THEN** la condición NO SHALL cumplirse
- **AND** la regla NO SHALL casar
- **AND** SHALL emitirse el `debug!` que nombra la colección y la regla

#### Scenario: La colección booleana no anota evidencia
- **GIVEN** una regla con una condición `is_tor`/`@truthy` que casa
- **THEN** NO SHALL emitirse evidencia para esa condición
- **AND** la forma del evento de auditoría SHALL ser la misma que antes de esta sub-entrega

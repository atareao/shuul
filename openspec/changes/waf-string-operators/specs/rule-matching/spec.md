## MODIFIED Requirements

### Requirement: El matcheo recorre un conjunto de condiciones proyectadas desde la regla

El matcher SHALL evaluar cada regla como un **conjunto ordenado de condiciones**. Una condición SHALL declarar la colección de la petición que restringe, la pipeline de transformaciones que se aplica a su valor antes de compararlo y el **operador** con el que se compara, que SHALL llevar consigo su propio patrón.

Una regla SHALL casar cuando **todas** sus condiciones casen. El conjunto SHALL evaluarse en AND y **con cortocircuito**: la primera condición que no case SHALL detener la evaluación, de modo que el resultado y su evidencia describan la decisión que de verdad se tomó.

Una regla que **no** declare ninguna condición SHALL ser **neutral**: NO SHALL impedir el match.

El contrato de cada condición SHALL ser el mismo que el de los tres brazos del matcheo actual:

- la colección restringida y presente → decide el **operador** sobre el valor **transformado** por su pipeline;
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

### Requirement: El matcheo evalúa los operadores de cadena

El conjunto de operadores del matcheo SHALL ser: `@rx` (regex), `@truthy` (booleano, solo para la colección `is_tor`), `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`. Cada condición SHALL declarar **exactamente un** operador, y el operador SHALL llevar consigo el patrón con el que compara: el matcheo NO SHALL recibir el patrón por otro camino.

La decisión de un operador de cadena SHALL tomarse sobre el valor **ya transformado** por la pipeline de la condición —nunca sobre el patrón, que se usa tal como se compiló—, y SHALL tener esta semántica, idéntica a la de Coraza:

- `@contains` SHALL casar cuando el valor **contenga** el patrón, con distinción de mayúsculas;
- `@streq` SHALL casar cuando el valor sea **exactamente igual** al patrón, con distinción de mayúsculas;
- `@beginsWith` SHALL casar cuando el valor **empiece por** el patrón, con distinción de mayúsculas;
- `@endsWith` SHALL casar cuando el valor **termine por** el patrón, con distinción de mayúsculas;
- `@within` SHALL casar cuando el **valor** (la aguja) sea igual a alguno de los elementos del **patrón** (el pajar), que es una **lista separada por comas**; los elementos SHALL recortarse de espacios antes de comparar y un elemento que quede vacío tras el recorte NO SHALL casar ningún valor: no SHALL comportarse como un comodín. Es el inverso de `@contains`, donde la aguja es el patrón;
- `@unconditionalMatch` SHALL casar **siempre** que la colección esté presente, sea cual sea su valor, y SHALL exigir **patrón vacío**.

La comparación insensible a mayúsculas NO SHALL ser una variante de operador: se obtiene con la transformación `t:lowercase`, que ya se aplica al valor.

Una colección **ausente** NO SHALL casar con ninguno de estos operadores —tampoco con `@unconditionalMatch`— y SHALL emitir el `debug!` que nombra la colección y la regla: el brazo de la colección ausente precede al operador, porque una condición no puede decidir sobre un dato que la petición no trae.

Los operadores de cadena SHALL anotar **evidencia** con el mismo contrato que `@rx`: solo cuando la condición case **y** su pipeline no esté vacía, bajo la clave de la colección —o la clave compuesta `colección[clave]`—, con el valor **transformado**. Con la pipeline vacía NO SHALL anotarse nada, y `@unconditionalMatch` SHALL seguir ese mismo contrato.

Un operador de cadena SHALL ser válido para las colecciones de texto. Sobre la colección booleana `is_tor` SHALL ser **incoherente** y NO SHALL casar, con el diagnóstico que lo nombra: la colección booleana solo se compara con `@truthy`.

La **negación** de una condición (`negate`) NO SHALL aplicarse en esta sub-entrega: toda condición decide en sentido afirmativo.

#### Scenario: `@contains` casa cuando el valor contiene el patrón

- **GIVEN** una regla con una condición de la colección `user_agent`, operador `@contains` y patrón `nikto`
- **WHEN** la petición trae el `User-Agent` `Mozilla/5.0 (Nikto/2.5)`
- **THEN** la condición NO SHALL casar, porque la comparación distingue mayúsculas
- **AND** con el `User-Agent` `nikto/2.5` la condición SHALL casar

#### Scenario: `@streq` casa solo con la igualdad exacta

- **GIVEN** una regla con una condición de la colección `method`, operador `@streq` y patrón `POST`
- **WHEN** la petición trae el método `POST`
- **THEN** la condición SHALL casar
- **AND** con el método `POSTX` o con el método `post` la condición NO SHALL casar

#### Scenario: `@beginsWith` casa por el principio y no por el final

- **GIVEN** una regla con una condición de la colección `path`, operador `@beginsWith` y patrón `/admin`
- **WHEN** la petición trae el path `/admin/users`
- **THEN** la condición SHALL casar
- **AND** con el path `/wp-admin` la condición NO SHALL casar

#### Scenario: `@endsWith` casa por el final y no por el principio

- **GIVEN** una regla con una condición de la colección `request_basename`, operador `@endsWith` y patrón `.exe`
- **WHEN** la petición trae el basename `setup.exe`
- **THEN** la condición SHALL casar
- **AND** con el basename `setup.exe.txt` la condición NO SHALL casar

#### Scenario: `@within` casa cuando el valor es uno de los elementos de la lista

- **GIVEN** una regla con una condición de la colección `method`, operador `@within` y patrón `GET, POST, HEAD`
- **WHEN** la petición trae el método `POST`
- **THEN** la condición SHALL casar, porque el elemento se recorta antes de comparar
- **AND** con el método `PUT` la condición NO SHALL casar

#### Scenario: `@within` es el inverso de `@contains`

- **GIVEN** una condición de la colección `method` con operador `@within` y patrón `GET,POST` y otra con operador `@contains` y el mismo patrón
- **WHEN** la petición trae el método `GET`
- **THEN** la primera SHALL casar, porque el valor es uno de los elementos del patrón
- **AND** la segunda NO SHALL casar, porque el valor no contiene la cadena `GET,POST`

#### Scenario: Un elemento vacío de `@within` no actúa como comodín

- **GIVEN** una regla con una condición de la colección `method`, operador `@within` y patrón `GET,,POST`
- **WHEN** la petición trae el método `PATCH`
- **THEN** la condición NO SHALL casar, porque el elemento vacío del patrón no casa ningún valor

#### Scenario: `@unconditionalMatch` casa siempre que la colección esté presente

- **GIVEN** una regla con una condición de la colección `referer`, operador `@unconditionalMatch` y patrón vacío
- **WHEN** la petición trae cualquier `Referer`
- **THEN** la condición SHALL casar

#### Scenario: La colección ausente no casa ni con `@unconditionalMatch`

- **GIVEN** la misma regla y una petición que no trae el `Referer`
- **THEN** la condición NO SHALL casar
- **AND** la regla NO SHALL casar
- **AND** SHALL emitirse el `debug!` que nombra la colección y la regla

#### Scenario: Un operador de cadena que no casa detiene la conjunción

- **GIVEN** una regla con una condición de `path` con operador `@beginsWith` y patrón `/api` que casa, y otra de `method` con operador `@streq` y patrón `POST` que no casa
- **THEN** la regla NO SHALL casar
- **AND** el cortocircuito SHALL detener el recorrido en la segunda condición

#### Scenario: La pipeline se aplica al valor y nunca al patrón

- **GIVEN** una regla con una condición de la colección `method`, operador `@streq`, patrón `get` y pipeline `lowercase`
- **WHEN** la petición trae el método `GET`
- **THEN** la condición SHALL casar, porque el valor se transforma antes de comparar
- **AND** con el patrón `GET` la condición NO SHALL casar, porque el patrón no se transforma

#### Scenario: La evidencia de un operador de cadena sigue el contrato de `@rx`

- **GIVEN** una regla con una condición de la colección `user_agent`, operador `@contains`, patrón `nikto` y pipeline `lowercase`
- **WHEN** la condición casa
- **THEN** la evidencia SHALL anotar el valor **transformado** bajo la clave `user_agent`

#### Scenario: Un operador de cadena con pipeline vacía no anota evidencia

- **GIVEN** una regla con una condición de la colección `user_agent`, operador `@contains`, patrón `nikto` y pipeline vacía
- **WHEN** la condición casa
- **THEN** NO SHALL anotarse evidencia para esa condición

#### Scenario: Un operador de cadena sobre `is_tor` es incoherente y no casa

- **GIVEN** una condición de la colección `is_tor` con operador `@contains` —combinación que la autoría rechaza y la lectura descarta—
- **WHEN** se evalúa contra una petición marcada como Tor
- **THEN** la condición NO SHALL casar
- **AND** SHALL emitirse el diagnóstico que nombra la colección y el operador

#### Scenario: El modo simple no cambia

- **GIVEN** una regla descrita con los 14 campos y la columna `is_tor`
- **THEN** sus condiciones SHALL llevar los operadores `@rx` y `@truthy`, como antes de esta sub-entrega
- **AND** su resultado de matcheo y su evidencia SHALL ser idénticos a los de F2b-3

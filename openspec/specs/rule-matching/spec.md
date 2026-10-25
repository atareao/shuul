# rule-matching Specification

## Purpose
Define el matcheo de una regla contra una petición: qué significa que una regla
casa cuando restringe varios campos a la vez, y —de forma señalada— qué ocurre
cuando la regla restringe un campo que la petición no trae.

## Requirements

### Requirement: Rule matching with absent request fields

Una regla SHALL casar una petición cuando **todos** sus filtros casen, evaluados
sobre los 14 campos disponibles: `ip_address`, `protocol`, `fqdn`, `path`,
`query`, `city_name`, `country_name`, `country_code`, `user_agent`, `method`,
`referer`, `content_type`, `accept_language` y `x_request_id`. Un filtro que la
regla **no** restringe SHALL ser **neutral** y NO SHALL impedir el match.

La evaluación de cada campo SHALL ser:

```rust
fn check_match(filter: Option<&Regex>, value: Option<&str>) -> bool {
    match (filter, value) {
        (Some(re), Some(v)) => re.is_match(v), // decide el regex
        (Some(_), None) => false,              // la regla restringe y el valor falta
        (None, _) => true,                     // la regla no restringe: neutral
    }
}
```

Es decir:

- si la regla restringe el campo y la petición trae valor, SHALL decidir el
  regex: `true` si casa, `false` si no;
- si la regla restringe el campo y la petición **no** trae valor, la condición
  SHALL considerarse **NO cumplida**, de modo que la regla NO SHALL casar. Casar
  sin evidencia es el fallo peligroso: un filtro de `user_agent` casaría el 100 %
  de las peticiones sin esa cabecera, y en el WAF una regla de denegación
  denegaría tráfico legítimo mientras en el Jail acumularía observaciones de rate
  limit contra él;
- si la regla **no** restringe el campo, la condición SHALL cumplirse tanto si la
  petición trae valor como si no.

Un valor de cabecera **ausente** y uno **presente pero vacío** SHALL ser
indistinguibles a efectos del matcheo: ambos cuentan como ausentes. Una regla NO
SHALL poder expresar «casar por ausencia de valor»: no es una capacidad
soportada.

Cuando un match se descarte **porque la regla restringe un campo y la petición
no lo trae**, el matcher SHALL emitir un `debug!` que nombre el campo y la regla.
El diagnóstico NO SHALL emitirse cuando el valor esté presente y el regex
simplemente no case —ese es un no-match legítimo, información distinta— ni cuando
la regla no restrinja el campo. El diagnóstico NO SHALL ser un error ni SHALL
interrumpir la evaluación.

Este contrato SHALL ser el mismo para los dos pipelines: el WAF, que construye la
petición desde las cabeceras vivas del forwardAuth, y el Jail, que la construye
desde el access log.

#### Scenario: Restricted field present and matching
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "curl/8.5"`
- **WHEN** la regla se evalúa
- **THEN** el campo casa
- **AND** la regla casa si el resto de sus filtros también casan

#### Scenario: Restricted field present and not matching
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "Mozilla/5.0"` que no casa con ese regex
- **WHEN** la regla se evalúa
- **THEN** el campo NO casa
- **AND** la regla NO casa

#### Scenario: Restricted field absent makes the rule not match
- **GIVEN** una regla con un regex de `user_agent` y una petición cuyo `user_agent` es ausente
- **WHEN** la regla se evalúa
- **THEN** el campo NO casa
- **AND** la regla NO casa
- **AND** se emite un `debug!` que nombra el campo `user_agent` y la regla

#### Scenario: One absent field discards a rule that matches on the others
- **GIVEN** una regla con un regex de `path` y un regex de `x_request_id`, y una petición con `path = "/admin"` (que casa) y `x_request_id` ausente
- **WHEN** la regla se evalúa
- **THEN** la regla NO casa
- **AND** el `debug!` nombra `x_request_id`

#### Scenario: An unrestricted field is neutral
- **GIVEN** una regla que sólo restringe `path` y una petición sin `user_agent`, sin `referer` ni `x_request_id`
- **WHEN** la regla se evalúa
- **THEN** los campos no restringidos son neutrales
- **AND** la regla casa si su regex de `path` casa

#### Scenario: A present but empty header counts as absent
- **GIVEN** una regla con un regex de `referer` y una petición cuya cabecera `Referer` está presente pero vacía
- **WHEN** la regla se evalúa
- **THEN** el campo se trata como ausente
- **AND** la regla NO casa

#### Scenario: The same semantics apply to both pipelines
- **GIVEN** una regla con filtro de `user_agent`, alcanzable desde el WAF (forwardAuth) y desde el Jail (access log)
- **WHEN** llega una petición sin cabecera `User-Agent`
- **THEN** la regla NO casa en ninguno de los dos pipelines
- **AND** en el Jail no se acumula ninguna observación de rate limit por esa petición

#### Scenario: A legitimate regex no-match is not diagnosed as an absent field
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "Mozilla/5.0"` que no casa con el regex
- **WHEN** la regla se evalúa
- **THEN** la regla NO casa
- **AND** NO se emite el `debug!` de campo ausente, porque el valor estaba presente

### Requirement: La pipeline de transformaciones se aplica al valor de la petición antes de compararlo

Cuando la regla declare una pipeline de transformaciones —una especificación `t:` no vacía—, el matcher SHALL aplicarla al **valor de la petición** de cada uno de los 14 campos antes de evaluarlo contra el regex de la regla. La pipeline NO SHALL aplicarse nunca al patrón de la regla.

Las transformaciones SHALL aplicarse **en el orden escrito**, de izquierda a derecha, y **una sola vez** cada una: NO SHALL repetirse hasta que el valor deje de cambiar.

Con la pipeline **vacía** —el caso de toda regla que no la declare— el valor SHALL entregarse a la comparación **sin tocarlo**: el matcheo SHALL ser byte a byte el de una regla sin pipeline, y la implementación NO SHALL copiar el valor.

Un valor de petición **ausente** SHALL seguir siendo ausente con independencia de la pipeline: NO SHALL transformarse `None`, y el brazo «la regla restringe el campo y la petición no lo trae» SHALL seguir sin casar y emitiendo su `debug!`.

El campo `is_tor` NO SHALL verse afectado: no es una cadena y NO SHALL transformarse.

Este contrato SHALL ser el mismo para los dos pipelines: el WAF y el Jail comparten el matcher.

#### Scenario: Una evasión codificada casa con la transformación
- **GIVEN** una regla con patrón `\.\./` y pipeline `urlDecodeUni`
- **WHEN** se evalúa contra una petición cuyo `path` es `%2e%2e%2f`
- **THEN** la regla SHALL casar

#### Scenario: Sin la transformación la misma evasión no casa
- **GIVEN** la misma regla con la pipeline **vacía**
- **WHEN** se evalúa contra la misma petición
- **THEN** la regla NO SHALL casar

#### Scenario: El orden de la pipeline importa
- **GIVEN** dos reglas con el mismo patrón, una con pipeline `lowercase,urlDecodeUni` y otra con `urlDecodeUni,lowercase`
- **WHEN** se evalúan contra un valor que distingue ambos órdenes
- **THEN** el resultado SHALL depender del orden escrito

#### Scenario: La pipeline no se aplica al patrón
- **GIVEN** una regla con patrón `[A-Z]+` y pipeline `lowercase`
- **WHEN** se evalúa contra un valor en mayúsculas
- **THEN** el valor SHALL compararse transformado a minúsculas
- **AND** el patrón SHALL compararse tal como se escribió
- **AND** la regla NO SHALL casar
- **AND** la misma regla sin pipeline SÍ SHALL casar el mismo valor

#### Scenario: Un campo ausente sigue ausente
- **GIVEN** una regla con un regex de `user_agent` y pipeline `urlDecodeUni`, y una petición sin `user_agent`
- **WHEN** se evalúa
- **THEN** la regla NO SHALL casar
- **AND** SHALL emitirse el `debug!` que nombra `user_agent`

#### Scenario: La pipeline vacía no copia el valor
- **GIVEN** una regla sin pipeline
- **WHEN** se evalúa
- **THEN** el valor SHALL entregarse a la comparación sin copiarse
- **AND** el resultado SHALL ser el mismo que antes de existir las transformaciones

#### Scenario: La evasión de doble codificación se cubre escribiéndola dos veces
- **GIVEN** una regla con patrón `\.\./` y pipeline `urlDecodeUni,urlDecodeUni`
- **WHEN** se evalúa contra una petición cuyo `path` es `%252e%252e%252f`
- **THEN** la regla SHALL casar
- **AND** una regla con pipeline `urlDecodeUni` sola NO SHALL casar

### Requirement: La evidencia del matcheo se obtiene del mismo recorrido que decide el matcheo

El matcher SHALL exponer, además del resultado booleano, la **evidencia** del matcheo: por cada campo que la regla **restringe** y que casó, el valor **transformado** que se comparó. La evidencia NO SHALL incluir los campos que la regla no restringe.

La evidencia SHALL obtenerse del **mismo** recorrido de filtros que decide el resultado booleano. NO SHALL existir un segundo recorrido que reevalúe los filtros por su cuenta: dos recorridos divergirían y la evidencia dejaría de describir la decisión que de verdad se tomó.

NO SHALL emitirse evidencia en ninguno de estos dos casos: con la pipeline **vacía** —el valor transformado es el valor crudo, que ya viaja en la petición, y añadirlo sería ruido— y con la pipeline **no vacía pero sin ningún campo de los 14 restringido** —no hay ningún valor transformado que anotar—. La evidencia SHALL existir **solo** cuando la pipeline es no vacía **y** la regla restringe al menos uno de los 14 campos.

Cuando la regla **no** case, NO SHALL emitirse evidencia alguna, aunque algún campo evaluado **antes** del primero que falló haya casado: la cadena de comprobaciones cortocircuita, y sin ese corte la evidencia describiría campos de una decisión que no se tomó. La evidencia SHALL describir siempre la decisión que de verdad se tomó.

#### Scenario: La evidencia nombra el valor transformado
- **GIVEN** una regla con patrón `\.\./`, pipeline `urlDecodeUni` y filtro sobre `path`
- **WHEN** casa una petición cuyo `path` es `%2e%2e%2f`
- **THEN** la evidencia SHALL nombrar `path`
- **AND** SHALL llevar `../` como valor
- **AND** NO SHALL llevar `%2e%2e%2f`

#### Scenario: Una regla sin pipeline no produce evidencia
- **GIVEN** una regla sin pipeline
- **WHEN** casa una petición
- **THEN** NO SHALL emitirse evidencia

#### Scenario: La evidencia solo cubre los campos restringidos
- **GIVEN** una regla con pipeline no vacía que restringe únicamente `path`
- **WHEN** casa
- **THEN** la evidencia SHALL nombrar `path`
- **AND** NO SHALL nombrar `user_agent` ni ningún otro campo no restringido

#### Scenario: Una pipeline sin campo restringido no produce evidencia
- **GIVEN** una regla con pipeline no vacía que solo restringe `is_tor`
- **WHEN** casa
- **THEN** NO SHALL emitirse evidencia

#### Scenario: Una regla que no casa no produce evidencia
- **GIVEN** una regla con pipeline `lowercase` que restringe `path` y `user_agent`
- **WHEN** se evalúa contra una petición cuyo `path` casa y cuyo `user_agent` está ausente
- **THEN** la regla NO SHALL casar
- **AND** NO SHALL emitirse evidencia, aunque `path` haya casado antes de fallar `user_agent`

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

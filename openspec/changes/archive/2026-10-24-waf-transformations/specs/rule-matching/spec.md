# rule-matching Specification (delta)

## ADDED Requirements

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

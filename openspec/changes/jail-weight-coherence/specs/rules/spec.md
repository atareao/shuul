# rules Specification (delta)

## ADDED Requirements

### Requirement: El peso de una regla de Jail es siempre 0

Una regla cuyo pipeline sea `jail` SHALL almacenarse con peso 0, con independencia del valor que traiga la petición: el pipeline de Jail evalúa todas las reglas que casan, sin `break`, de modo que su orden no decide nada y un peso distinto de 0 sería una elección sin efecto. La interfaz SHALL NOT ofrecer el peso como valor elegible para ese pipeline. Las reglas de Jail ya almacenadas SHALL normalizarse a 0 con esa misma regla.

#### Scenario: La API no confía en el cliente
- **GIVEN** una petición de creación de regla con pipeline `jail` y un peso distinto de 0
- **WHEN** la regla se crea
- **THEN** la regla queda almacenada con peso 0
- **AND** el peso que devuelve la API para esa regla es 0

#### Scenario: La edición tampoco reintroduce un peso
- **GIVEN** una regla de Jail existente con peso 0
- **WHEN** se actualiza enviando un peso distinto de 0
- **THEN** la regla sigue almacenada con peso 0

#### Scenario: La normalización alcanza a lo ya desplegado
- **GIVEN** una base de datos con reglas de Jail de pesos variados
- **WHEN** se aplica la migración de normalización
- **THEN** todas las reglas de Jail quedan con peso 0
- **AND** las reglas del WAF conservan su peso

#### Scenario: El peso no se ofrece donde no aplica
- **GIVEN** una regla o una plantilla cuyo pipeline es `jail`
- **WHEN** se abre su diálogo
- **THEN** el campo Weight no se ofrece
- **AND** el valor que la interfaz envía para esa regla es 0

### Requirement: La ordenación de la lista de reglas es estable

La lista de reglas SHALL ordenarse por el criterio pedido y SHALL desempatar por un criterio determinista, de modo que dos consultas consecutivas con el mismo criterio devuelvan el mismo orden aunque haya pesos repetidos, y que la paginación no repita ni pierda filas empatadas.

#### Scenario: Los empates no bailan entre consultas
- **GIVEN** varias reglas con el mismo peso
- **WHEN** se listan ordenadas por peso dos veces consecutivas
- **THEN** las dos respuestas presentan las reglas en el mismo orden

#### Scenario: La paginación de empates no repite ni pierde filas
- **GIVEN** más reglas con el mismo peso que el tamaño de página
- **WHEN** se recorren todas las páginas ordenando por ese peso
- **THEN** la unión de las páginas contiene cada regla exactamente una vez

# rules Specification (delta)

## ADDED Requirements

### Requirement: Las condiciones de una regla se persisten en `rule_conditions`

El sistema SHALL persistir el **conjunto ordenado de condiciones** de cada regla en la tabla `rule_conditions`, con una fila por condición: `(rule_id, position, collection, key, transformations, operator, pattern, negate)`.

`rule_id` SHALL referenciar `rules(id)` con `ON DELETE CASCADE`; el par `(rule_id, position)` SHALL ser único; `position` SHALL fijar el orden de evaluación de las condiciones dentro de la regla.

En esta sub-entrega el vocabulario de colecciones SHALL ser el de los 14 campos actuales; `key` SHALL ser siempre nulo, `operator` SHALL ser siempre `@rx` y `negate` SHALL ser siempre `0`.

El sistema SHALL persistir **una** condición por cada campo de los 14 que la regla restrinja con un patrón **no vacío y compilable**. Un campo ausente, en blanco o cuyo patrón no compile NO SHALL generar fila —su ausencia equivale al brazo neutral del matcheo—.

Escribir una regla SHALL escribir las 14 columnas **y** proyectarlas a `rule_conditions`, de modo que ambas representaciones describan la misma regla.

#### Scenario: Un campo restringido genera su fila
- **GIVEN** una regla con `path` restringido
- **WHEN** se crea por la API
- **THEN** SHALL existir una fila en `rule_conditions` para esa regla con `collection` igual a `path` y `pattern` igual al patrón de la regla

#### Scenario: Un campo en blanco no genera fila
- **GIVEN** una regla con `path` restringido y `query` en blanco
- **THEN** SHALL existir solo la fila de `path`
- **AND** la regla SHALL evaluar `query` como neutral

#### Scenario: El juego de condiciones se reemplaza al editar
- **GIVEN** una regla con condiciones persistidas
- **WHEN** se edita para restringir un conjunto distinto de campos
- **THEN** la tabla SHALL contener exactamente las condiciones de la nueva versión
- **AND** NO SHALL quedar ninguna fila de la versión anterior

#### Scenario: Las condiciones se borran con su regla
- **GIVEN** una regla con condiciones persistidas
- **WHEN** se elimina la regla
- **THEN** sus filas de `rule_conditions` SHALL desaparecer

#### Scenario: La migración rellena las reglas existentes
- **GIVEN** una base con reglas almacenadas antes de esta sub-entrega
- **WHEN** se aplica la migración
- **THEN** cada regla SHALL recibir sus filas correspondientes, una por campo restringido
- **AND** la proyección desde esas filas SHALL ser igual a la de los 14 campos

### Requirement: El matcheo se alimenta de las condiciones persistidas

La carga de la caché SHALL construir el conjunto de condiciones de cada regla a partir de las filas de `rule_conditions`, ordenadas por `position`.

La proyección desde la tabla SHALL ser **equivalente** a la de los 14 campos: mismo resultado de matcheo, misma evidencia y misma forma del evento de auditoría.

Una regla cuya especificación de transformaciones no se pueda leer NO SHALL entrar en la caché, igual que hoy.

#### Scenario: Equivalencia sobre las plantillas
- **GIVEN** cada plantilla del catálogo persistida y recargada
- **THEN** su conjunto de condiciones SHALL ser el mismo que proyecta la regla desde los 14 campos

#### Scenario: La decisión no cambia
- **GIVEN** el mismo juego de reglas que antes de esta sub-entrega
- **THEN** el matcheo SHALL producir las mismas decisiones y la misma evidencia

#### Scenario: Una regla con una especificación irresoluble no entra en la caché
- **GIVEN** una regla cuya especificación de transformaciones no se puede leer
- **THEN** la regla NO SHALL entrar en la caché
- **AND** SHALL emitirse el diagnóstico que la nombra

### Requirement: Las 14 columnas se conservan como representación simple

Las 14 columnas de `rules` SHALL seguir almacenándose y siendo la entrada y la salida de la API en modo simple; export e import SHALL seguir usándolas.

#### Scenario: El ciclo export/import es estable
- **GIVEN** un conjunto de reglas
- **WHEN** se exportan y se importan
- **THEN** las reglas SHALL quedar descritas igual que antes
- **AND** sus condiciones persistidas SHALL coincidir con las de antes

#### Scenario: El modo simple se conserva
- **GIVEN** una regla creada por la API con los 14 campos
- **THEN** SHALL comportarse exactamente como antes de esta sub-entrega

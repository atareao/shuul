# waf-pipeline Specification (delta)

## ADDED Requirements

### Requirement: El evento de auditoría del WAF anota el valor transformado que casó

Cuando la regla que casa en el pipeline del WAF declare una pipeline de transformaciones **no vacía** **y** restrinja **al menos uno** de los 14 campos de la petición, el evento de auditoría —tanto `admitted` como `denied`— SHALL añadir la clave `"matched"`, un objeto que mapea **cada campo restringido que casó** a su valor **transformado**, junto a los valores crudos que el evento ya lleva.

La forma de la clave SHALL ser un objeto anidado cuyas claves son los nombres canónicos de los 14 campos y cuyos valores son los valores transformados: `"matched": {"path": "../"}`. La clave NO SHALL ser una cadena ni un array.

El valor transformado SHALL ser el que de verdad se comparó con el patrón: es lo que permite afinar una regla sin adivinar por qué casó.

Cuando la regla declare la pipeline **vacía**, el evento NO SHALL llevar la clave `"matched"`: el valor transformado es el crudo, que el evento ya lleva, y añadirlo cambiaría la forma de todos los eventos existentes sin aportar información.

Cuando la regla declare la pipeline **no vacía** pero **no restrinja ninguno** de los 14 campos —solo `is_tor`, o solo `allow`—, el evento NO SHALL llevar la clave `"matched"`: no hay ningún valor transformado que anotar, y un objeto vacío no aportaría información.

Las dos condiciones SHALL cumplirse a la vez: la clave existe **solo** con la pipeline no vacía **y** al menos un campo de los 14 restringido.

#### Scenario: Una denegación con pipeline anota el valor transformado
- **GIVEN** una regla con patrón `\.\./`, pipeline `urlDecodeUni` y filtro sobre `path`
- **WHEN** deniega una petición cuyo `path` es `%2e%2e%2f`
- **THEN** el evento SHALL ser `denied`
- **AND** SHALL llevar la clave `"matched"` con la forma `{"path": "../"}`
- **AND** SHALL seguir llevando el valor crudo del `path`

#### Scenario: Una denegación sin pipeline no anota nada nuevo
- **GIVEN** una regla sin pipeline
- **WHEN** deniega una petición
- **THEN** el evento NO SHALL llevar la clave `"matched"`

#### Scenario: Una admisión con pipeline también lo anota
- **GIVEN** una regla con `allow = true` y pipeline no vacía
- **WHEN** admite una petición
- **THEN** el evento SHALL ser `admitted`
- **AND** SHALL llevar la clave `"matched"` con el valor transformado

#### Scenario: El evento nombra los campos que la regla restringe
- **GIVEN** una regla con pipeline no vacía que restringe únicamente `query`
- **WHEN** casa
- **THEN** la clave `"matched"` SHALL llevar el valor transformado de `query`
- **AND** NO SHALL llevar el de ningún campo no restringido

#### Scenario: Una pipeline sin campo restringido no anota nada
- **GIVEN** una regla con pipeline no vacía que no restringe ningún campo de los 14 —solo `is_tor`—
- **WHEN** casa
- **THEN** el evento NO SHALL llevar la clave `"matched"`

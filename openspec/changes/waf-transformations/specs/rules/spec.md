# rules Specification (delta)

## ADDED Requirements

### Requirement: La especificación de transformaciones se valida en la escritura

La especificación de transformaciones de una regla SHALL validarse en las **tres** vías de escritura del cliente —creación, edición e importación—, igual que ya se valida el pipeline.

Una especificación que contenga un token que no se reconozca NO SHALL almacenarse: la creación y la edición SHALL responder `400 Bad Request` con un mensaje que **nombre el token**, y la importación SHALL devolver esa entrada en `rejected` con el motivo, sin abortar la importación de las demás.

El valor por defecto SHALL ser la cadena vacía, que es la pipeline vacía: una regla que no declare transformaciones NO SHALL cambiar de comportamiento.

Los nombres de las transformaciones SHALL ser sensibles a mayúsculas y SHALL ser los canónicos. Un token con otra capitalización SHALL rechazarse con el mensaje que lista los nombres válidos.

El marcador `none` SHALL aceptarse y SHALL limpiar la pipeline construida hasta ese punto.

#### Scenario: Una transformación desconocida se rechaza al crear
- **GIVEN** un `POST /api/v1/rules` con `transformations: "urlDecodeUni,noExiste"`
- **WHEN** el handler procesa la petición
- **THEN** SHALL responder `400 Bad Request`
- **AND** el mensaje SHALL nombrar `noExiste`
- **AND** NO SHALL almacenarse ninguna fila

#### Scenario: Una transformación desconocida se rechaza al editar
- **GIVEN** un `PATCH /api/v1/rules` que introduce un token desconocido
- **WHEN** el handler procesa la petición
- **THEN** SHALL responder `400 Bad Request`
- **AND** la regla SHALL conservar su especificación anterior

#### Scenario: Una transformación desconocida se rechaza al importar sin abortar el resto
- **GIVEN** un `POST /api/v1/rules/import` con dos reglas, una con un token desconocido
- **WHEN** el handler procesa la petición
- **THEN** la regla inválida SHALL aparecer en `rejected` con el motivo
- **AND** la otra SHALL importarse

#### Scenario: Una especificación válida se almacena tal cual
- **GIVEN** un `POST /api/v1/rules` con `transformations: "urlDecodeUni,compressWhitespace"`
- **WHEN** el handler procesa la petición
- **THEN** SHALL responder `201 Created`
- **AND** la regla leída SHALL declarar la misma especificación

#### Scenario: El valor por defecto es la pipeline vacía
- **GIVEN** un `POST /api/v1/rules` sin el campo `transformations`
- **WHEN** el handler procesa la petición
- **THEN** la regla SHALL almacenarse con la cadena vacía

#### Scenario: Un nombre con otra capitalización se rechaza
- **GIVEN** un `POST /api/v1/rules` con `transformations: "urldecodeuni"`
- **WHEN** el handler procesa la petición
- **THEN** SHALL responder `400 Bad Request`
- **AND** el mensaje SHALL listar los nombres válidos

#### Scenario: El marcador none limpia la pipeline
- **GIVEN** un `POST /api/v1/rules` con `transformations: "urlDecodeUni,none,lowercase"`
- **WHEN** el handler procesa la petición
- **THEN** la regla SHALL almacenar una pipeline equivalente a `lowercase`

### Requirement: Una regla con una especificación irresoluble no entra en la caché

Cuando una fila de `rules` ya almacenada declare una especificación de transformaciones que no se pueda leer, la regla NO SHALL cargarse en la caché de reglas activas y SHALL emitirse un `error!` que **nombre la regla y el token**.

La regla NO SHALL participar en ninguno de los dos pipelines con una pipeline **parcial**: cargarla con las transformaciones que sí se reconocen —o con ninguna— la haría decidir algo distinto de lo que su autor escribió, que es el fallo silencioso que este requisito evita.

El resto de reglas SHALL cargarse con normalidad.

#### Scenario: Una fila con un token irresoluble queda fuera de la caché
- **GIVEN** una fila insertada por SQL crudo con `transformations = "urlDecodeUni,noExiste"`
- **WHEN** se cargan las reglas activas
- **THEN** esa regla NO SHALL estar en la caché
- **AND** SHALL emitirse un `error!` que nombre la regla y `noExiste`

#### Scenario: Las demás reglas siguen cargándose
- **GIVEN** la misma base con otra regla válida
- **WHEN** se cargan las reglas activas
- **THEN** la regla válida SHALL estar en la caché

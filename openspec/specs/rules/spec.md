# rules Specification

## Purpose
La regla es la unidad de decisión de Shuul: los filtros que se evalúan contra cada petición, el veredicto y el pipeline al que pertenece. Este capability fija las invariantes del recurso que el resto del sistema da por ciertas: qué pipelines existen y cómo se validan en la escritura, por qué el peso de una regla de Jail es siempre 0 (el pipeline de Jail evalúa todas las reglas que casan, sin `break`, así que su orden no decide nada) y cómo se ordena la lista de reglas para que la paginación no repita ni pierda filas.

## Requirements

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

### Requirement: El pipeline de una regla se valida en la escritura

Una regla SHALL almacenarse únicamente con un pipeline admitido —`waf` o `jail`— y la API SHALL rechazar cualquier otro valor, incluido el mismo identificador con otra capitalización, en las tres vías de escritura (creación, edición e importación). El resto del sistema compara el pipeline de forma literal y sensible a mayúsculas, así que un valor no admitido haría que una regla participase del pipeline de Jail sin que se le aplique la invariante del peso 0. La normalización del legado SHALL alcanzar tanto el peso como la etiqueta del pipeline: una fila histórica con el pipeline en otra capitalización queda fuera de las comparaciones literales del resto del sistema y conservaría su peso, así que la migración la deja canónica.

#### Scenario: La creación rechaza un pipeline no admitido
- **GIVEN** una petición de creación de regla con un pipeline distinto de `waf` y de `jail`
- **WHEN** se procesa
- **THEN** la petición se rechaza
- **AND** no queda ninguna regla almacenada con ese nombre

#### Scenario: La edición rechaza un pipeline no admitido
- **GIVEN** una regla existente
- **WHEN** se actualiza con un pipeline distinto de `waf` y de `jail`
- **THEN** la actualización se rechaza y la regla conserva sus valores

#### Scenario: La importación no cuela un pipeline no admitido
- **GIVEN** una importación que incluye una regla con un pipeline no admitido
- **WHEN** se procesa
- **THEN** esa entrada no se almacena
- **AND** la respuesta identifica la entrada rechazada

#### Scenario: La migración alcanza al legado con otra capitalización
- **GIVEN** una base con una regla de Jail almacenada con el pipeline en mayúsculas
- **WHEN** se aplica la migración de normalización
- **THEN** su peso queda a 0
- **AND** su pipeline queda almacenado de forma canónica (`jail`)
- **AND** las reglas del WAF conservan su peso

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

### Requirement: La condición persistida conserva su clave

El sistema SHALL almacenar la clave de cada condición persistida: el nombre de la cookie, la cabecera o el argumento para las colecciones con clave, y nulo para las colecciones sin clave. Al leer, el matcheo SHALL reconstruir la condición con su clave.

Una condición cuya colección exige clave NO SHALL persistirse sin clave; una condición cuya colección no la exige NO SHALL persistirse con clave.

#### Scenario: Una condición con clave se conserva en el ciclo escritura-lectura

- **GIVEN** una regla con una condición de la colección `request_cookies` y clave `session`
- **WHEN** se crea y se recarga
- **THEN** la condición SHALL tener clave `session`

#### Scenario: Una condición sin clave la conserva nula

- **GIVEN** una regla con una condición de la colección `path`
- **WHEN** se recarga
- **THEN** su clave SHALL ser nula

### Requirement: La autoría avanzada acepta una lista de condiciones

La creación y la edición de una regla SHALL aceptar un campo opcional `conditions` con una lista ordenada de condiciones.

- Ausente o nulo → **modo simple**: el conjunto de condiciones SHALL proyectarse desde los 14 campos, como hasta ahora.
- Presente → **modo avanzado**: el conjunto persistido SHALL ser exactamente la lista dada, en su orden, y los 14 campos NO SHALL participar del matcheo.

Cada condición de la lista SHALL declarar una colección conocida; la clave SHALL ser obligatoria exactamente cuando la colección la exige y NO SHALL admitirse cuando no la exige; el operador SHALL ser ausente o `@rx`; el patrón SHALL compilar; y las transformaciones SHALL resolverse.

Una entrada que no cumpla SHALL rechazarse con `400 Bad Request` nombrando el motivo, sin almacenar la regla.

#### Scenario: Una regla avanzada persiste su lista tal cual

- **GIVEN** una petición de creación con dos condiciones
- **WHEN** se almacena
- **THEN** el conjunto persistido SHALL contener esas dos condiciones, en orden, con sus claves

#### Scenario: Una colección con clave sin clave se rechaza

- **GIVEN** una petición con una condición de la colección `request_cookies` sin clave
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

#### Scenario: Una clave en una colección que no la admite se rechaza

- **GIVEN** una petición con una condición de la colección `path` con clave
- **THEN** la API SHALL responder `400`

#### Scenario: Una colección desconocida se rechaza

- **GIVEN** una petición con una condición de una colección desconocida
- **THEN** la API SHALL responder `400`

#### Scenario: Un patrón que no compila se rechaza

- **GIVEN** una petición con una condición cuyo patrón no compila
- **THEN** la API SHALL responder `400`

#### Scenario: El modo simple se conserva

- **GIVEN** una petición de creación sin `conditions`
- **THEN** la regla SHALL comportarse exactamente como antes de esta sub-entrega

### Requirement: La representación de una regla expone sus condiciones

La representación de una regla devuelta por la API (al crear, editar, leer, listar y exportar) SHALL incluir el conjunto de condiciones persistidas, cada una con su colección, su clave, sus transformaciones, su operador y su patrón.

La importación SHALL persistir las condiciones incluidas en cada entrada; a falta de ellas SHALL proyectarlas desde los 14 campos.

#### Scenario: El ciclo de una regla avanzada es estable

- **GIVEN** una regla avanzada creada por la API
- **WHEN** se lee y se reenvían sus condiciones
- **THEN** la regla SHALL quedar descrita igual

#### Scenario: La exportación e importación conservan una regla avanzada

- **GIVEN** una regla avanzada
- **WHEN** se exporta y se importa
- **THEN** sus condiciones SHALL coincidir con las de antes

#### Scenario: La importación sin condiciones proyecta los 14 campos

- **GIVEN** una entrada de importación sin condiciones
- **THEN** sus condiciones SHALL proyectarse desde los 14 campos

### Requirement: El diálogo de regla ofrece la autoría simple y la avanzada

El diálogo de regla SHALL ofrecer una pestaña de condiciones con un control conmutador entre el **modo simple** y el **modo avanzado**.

En **modo simple** el diálogo SHALL autorizar los 14 campos y SHALL NO enviar el campo `conditions`; el conjunto de condiciones SHALL proyectarse desde los 14 campos, como hasta ahora.

En **modo avanzado** el diálogo SHALL autorizar una **lista ordenada de condiciones** y SHALL enviarla como el campo `conditions`; los 14 campos SHALL seguir enviándose y almacenándose, pero SHALL NO participar del matcheo.

Por cada condición, en modo avanzado, el diálogo SHALL ofrecer la colección, la clave —solo cuando la colección la exige—, las transformaciones y el patrón, y SHALL mostrar el operador como `@rx` fijo, no editable.

En modo avanzado el diálogo SHALL deshabilitar las pestañas de los 14 campos y SHALL mostrar una nota de que esos campos se almacenan pero no participan del matcheo.

Una regla **nueva** SHALL abrirse en modo simple.

En modo avanzado el diálogo SHALL NO permitir guardar mientras alguna condición de una colección que exige clave carezca de clave, y SHALL señalar la condición pendiente.

#### Scenario: Una regla nueva abre en modo simple y no envía condiciones

- **GIVEN** el diálogo de creación de una regla
- **THEN** SHALL abrirse en modo simple
- **AND** al guardar, el cuerpo NO SHALL incluir `conditions`

#### Scenario: El modo avanzado envía la lista de condiciones

- **GIVEN** el diálogo en modo avanzado con dos condiciones
- **WHEN** se guarda
- **THEN** el cuerpo SHALL incluir `conditions` con esas dos condiciones, en orden

#### Scenario: El modo avanzado con lista vacía envía `conditions` vacía

- **GIVEN** el diálogo en modo avanzado sin ninguna condición
- **WHEN** se guarda
- **THEN** el cuerpo SHALL incluir `conditions` como lista vacía
- **AND** el conjunto persistido SHALL quedar vacío

#### Scenario: Los 14 campos se deshabilitan en modo avanzado

- **GIVEN** el diálogo en modo avanzado
- **THEN** las pestañas de los 14 campos SHALL estar deshabilitadas
- **AND** SHALL mostrarse una nota de que no participan del matcheo

#### Scenario: Una colección con clave sin clave impide guardar

- **GIVEN** el diálogo en modo avanzado con una condición de la colección `request_cookies` sin clave
- **THEN** el diálogo SHALL impedir guardar
- **AND** SHALL señalar la condición que falta por completar

### Requirement: El cambio de modo gestiona el conjunto de condiciones

Al pasar de **simple** a **avanzado**, el diálogo SHALL sembrar la lista de condiciones proyectando los 14 campos rellenos, con el mismo criterio que la proyección de la API: una condición por cada campo con patrón no vacío, en el orden canónico de los 14, con la colección del campo, el patrón del campo y las transformaciones de la regla. `is_tor` NO SHALL proyectarse.

Al pasar de **avanzado** a **simple**, el diálogo SHALL pedir confirmación y, si se confirma, SHALL descartar la lista y SHALL enviar la regla sin `conditions`.

#### Scenario: Pasar a avanzado proyecta los 14 campos rellenos

- **GIVEN** una regla en modo simple con `path` y `user_agent` rellenos
- **WHEN** se cambia a modo avanzado
- **THEN** la lista SHALL contener una condición por cada campo relleno, con su colección y su patrón

#### Scenario: `is_tor` no se proyecta

- **GIVEN** una regla en modo simple con `is_tor` activo y ningún campo relleno
- **WHEN** se cambia a modo avanzado
- **THEN** la lista SHALL quedar vacía

#### Scenario: Volver a simple descarta la lista tras confirmar

- **GIVEN** el diálogo en modo avanzado con condiciones
- **WHEN** se cambia a modo simple y se confirma
- **THEN** la lista SHALL descartarse
- **AND** al guardar, el cuerpo NO SHALL incluir `conditions`

### Requirement: El modo de una regla existente se infiere de sus condiciones

Al editar una regla existente, el diálogo SHALL abrirse en el modo inferido de las condiciones que recibe la regla: **simple** si coinciden exactamente con la proyección de sus 14 campos, y **avanzado** en caso contrario. En modo avanzado la lista SHALL sembrarse con las condiciones recibidas.

La inferencia NO SHALL alterar la regla almacenada: en ambos modos el conjunto que se vuelve a enviar reproduce el conjunto cargado.

#### Scenario: Una regla simple abre en modo simple

- **GIVEN** una regla cuyas condiciones recibidas son la proyección de sus 14 campos
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo simple

#### Scenario: Una regla avanzada abre en modo avanzado con sus condiciones

- **GIVEN** una regla cuyas condiciones recibidas difieren de la proyección de sus 14 campos
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo avanzado
- **AND** la lista SHALL sembrarse con las condiciones recibidas

## MODIFIED Requirements

### Requirement: Las condiciones de una regla se persisten en `rule_conditions`

El sistema SHALL persistir el **conjunto ordenado de condiciones** de cada regla en la tabla `rule_conditions`, con una fila por condición: `(rule_id, position, collection, key, transformations, operator, pattern, negate)`.

`rule_id` SHALL referenciar `rules(id)` con `ON DELETE CASCADE`; el par `(rule_id, position)` SHALL ser único; `position` SHALL fijar el orden de evaluación de las condiciones dentro de la regla.

El vocabulario de colecciones SHALL incluir los 14 campos actuales y la colección booleana `is_tor`. La columna `key` SHALL ser nula para las colecciones que no exigen clave y SHALL llevar el nombre de la cookie, la cabecera o el argumento para las tres colecciones con clave. La columna `operator` SHALL ser `@rx` para las colecciones de texto y `@truthy` para la colección `is_tor`. `negate` SHALL ser siempre `0`.

El sistema SHALL persistir **una** condición por cada campo de los 14 que la regla restrinja con un patrón **no vacío y compilable**, más **una** condición para la colección `is_tor` cuando la columna `is_tor` de la regla sea verdadera, con `collection = is_tor`, `operator = @truthy`, `pattern` vacío, `key` nulo y `negate` 0. Un campo ausente, en blanco o cuyo patrón no compile NO SHALL generar fila —su ausencia equivale al brazo neutral del matcheo—, y una columna `is_tor` falsa o nula NO SHALL generar fila.

Escribir una regla SHALL escribir las 14 columnas y la columna `is_tor` **y** proyectarlas a `rule_conditions`, de modo que ambas representaciones describan la misma regla.

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

#### Scenario: La columna `is_tor` genera su fila
- **GIVEN** una regla con la columna `is_tor` verdadera
- **WHEN** se crea por la API
- **THEN** SHALL existir una fila en `rule_conditions` con `collection` igual a `is_tor`, `operator` igual a `@truthy`, `pattern` vacío, `key` nulo y `negate` 0
- **AND** esa fila SHALL ir después de las filas de los 14 campos

#### Scenario: La migración rellena las reglas con `is_tor`
- **GIVEN** una base con reglas almacenadas con `is_tor = 1` antes de esta sub-entrega
- **WHEN** se aplica la migración
- **THEN** cada una de esas reglas SHALL recibir una fila `is_tor`/`@truthy` con `pattern` vacío, `key` nulo y `negate` 0
- **AND** las reglas con `is_tor` falso o nulo NO SHALL recibir fila alguna

### Requirement: Las 14 columnas se conservan como representación simple

Las 14 columnas de `rules` SHALL seguir almacenándose y siendo la entrada y la salida de la API en modo simple; export e import SHALL seguir usándolas.

La columna `is_tor` SHALL conservarse en la misma representación simple: SHALL seguir almacenándose, SHALL seguir siendo entrada y salida de la API y del export/import, y SHALL proyectarse a `rule_conditions` al escribir la regla.

#### Scenario: El ciclo export/import es estable
- **GIVEN** un conjunto de reglas
- **WHEN** se exportan y se importan
- **THEN** las reglas SHALL quedar descritas igual que antes
- **AND** sus condiciones persistidas SHALL coincidir con las de antes

#### Scenario: El modo simple se conserva
- **GIVEN** una regla creada por la API con los 14 campos
- **THEN** SHALL comportarse exactamente como antes de esta sub-entrega

#### Scenario: `is_tor` sobrevive el ciclo simple
- **GIVEN** una regla creada por la API en modo simple con `is_tor` verdadero
- **WHEN** se lee y se reenvía
- **THEN** su columna `is_tor` SHALL conservarse
- **AND** su condición persistida `is_tor`/`@truthy` SHALL coincidir con la de antes

### Requirement: La autoría avanzada acepta una lista de condiciones

La creación y la edición de una regla SHALL aceptar un campo opcional `conditions` con una lista ordenada de condiciones.

- Ausente o nulo → **modo simple**: el conjunto de condiciones SHALL proyectarse desde los 14 campos y la columna `is_tor`, como hasta ahora.
- Presente → **modo avanzado**: el conjunto persistido SHALL ser exactamente la lista dada, en su orden, y los 14 campos y la columna `is_tor` NO SHALL participar del matcheo.

Cada condición de la lista SHALL declarar una colección conocida; la clave SHALL ser obligatoria exactamente cuando la colección la exige y NO SHALL admitirse cuando no la exige; el operador SHALL ser ausente o `@rx` para las colecciones de texto y `@truthy` para `is_tor`; el patrón SHALL compilar; y las transformaciones SHALL resolverse. Una condición de `is_tor` con operador `@truthy` SHALL exigir `pattern` vacío, `key` ausente y `transformations` vacías.

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

#### Scenario: Una condición de `is_tor` con `@truthy` se acepta

- **GIVEN** una petición con una condición de la colección `is_tor`, operador `@truthy`, sin clave, sin transformaciones y con `pattern` vacío
- **THEN** la API SHALL aceptarla
- **AND** la condición persistida SHALL quedar como `is_tor`/`@truthy`

#### Scenario: `@truthy` en una colección de texto se rechaza (400)

- **GIVEN** una petición con una condición de la colección `path` y operador `@truthy`
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

#### Scenario: `@rx` sobre `is_tor` se rechaza (400)

- **GIVEN** una petición con una condición de la colección `is_tor` y operador `@rx`
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

### Requirement: El diálogo de regla ofrece la autoría simple y la avanzada

El diálogo de regla SHALL ofrecer una pestaña de condiciones con un control conmutador entre el **modo simple** y el **modo avanzado**.

En **modo simple** el diálogo SHALL autorizar los 14 campos y SHALL NO enviar el campo `conditions`; el conjunto de condiciones SHALL proyectarse desde los 14 campos y la columna `is_tor`, como hasta ahora.

En **modo avanzado** el diálogo SHALL autorizar una **lista ordenada de condiciones** y SHALL enviarla como el campo `conditions`; los 14 campos SHALL seguir enviándose y almacenándose, pero SHALL NO participar del matcheo.

Por cada condición, en modo avanzado, el diálogo SHALL ofrecer la colección, la clave —solo cuando la colección la exige—, las transformaciones y el patrón, y SHALL fijar el operador: `@rx` para las colecciones de texto y `@truthy` para `is_tor`. Para una condición de `is_tor` el diálogo SHALL fijar `@truthy` y SHALL NO ofrecer patrón, clave ni transformaciones.

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

#### Scenario: El operador se fija para `is_tor`

- **GIVEN** el diálogo en modo avanzado con una condición de la colección `is_tor`
- **THEN** el diálogo SHALL fijar el operador `@truthy`
- **AND** NO SHALL ofrecer patrón, clave ni transformaciones para esa condición

### Requirement: El cambio de modo gestiona el conjunto de condiciones

Al pasar de **simple** a **avanzado**, el diálogo SHALL sembrar la lista de condiciones proyectando los 14 campos rellenos y la columna `is_tor`, con el mismo criterio que la proyección de la API: una condición por cada campo con patrón no vacío, en el orden canónico de los 14, con la colección del campo, el patrón del campo y las transformaciones de la regla; y, tras esas, la condición `is_tor`/`@truthy` cuando la regla tenga `is_tor` activo. Una regla con `is_tor` inactivo NO SHALL sembrar la condición de `is_tor`.

Al pasar de **avanzado** a **simple**, el diálogo SHALL pedir confirmación y, si se confirma, SHALL descartar la lista y SHALL enviar la regla sin `conditions`.

#### Scenario: Pasar a avanzado proyecta los 14 campos rellenos

- **GIVEN** una regla en modo simple con `path` y `user_agent` rellenos
- **WHEN** se cambia a modo avanzado
- **THEN** la lista SHALL contener una condición por cada campo relleno, con su colección y su patrón

#### Scenario: `is_tor` activo se proyecta

- **GIVEN** una regla en modo simple con `is_tor` activo y ningún campo relleno
- **WHEN** se cambia a modo avanzado
- **THEN** la lista SHALL contener una condición `is_tor`/`@truthy`

#### Scenario: `is_tor` no se proyecta

- **GIVEN** una regla en modo simple con `is_tor` inactivo
- **WHEN** se cambia a modo avanzado
- **THEN** la lista NO SHALL contener la condición de `is_tor`

#### Scenario: Volver a simple descarta la lista tras confirmar

- **GIVEN** el diálogo en modo avanzado con condiciones
- **WHEN** se cambia a modo simple y se confirma
- **THEN** la lista SHALL descartarse
- **AND** al guardar, el cuerpo NO SHALL incluir `conditions`

### Requirement: El modo de una regla existente se infiere de sus condiciones

Al editar una regla existente, el diálogo SHALL abrirse en el modo inferido de las condiciones que recibe la regla: **simple** si coinciden exactamente con la proyección de sus 14 campos y de su columna `is_tor`, y **avanzado** en caso contrario. En modo avanzado la lista SHALL sembrarse con las condiciones recibidas.

La inferencia NO SHALL alterar la regla almacenada: en ambos modos el conjunto que se vuelve a enviar reproduce el conjunto cargado.

#### Scenario: Una regla simple abre en modo simple

- **GIVEN** una regla cuyas condiciones recibidas son la proyección de sus 14 campos y de su columna `is_tor`
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo simple

#### Scenario: Una regla simple con `is_tor` no degrada a avanzado

- **GIVEN** una regla simple con `is_tor` activo y cuyas condiciones recibidas son la proyección de sus 14 campos más la condición `is_tor`/`@truthy`
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo simple

#### Scenario: Una regla avanzada abre en modo avanzado con sus condiciones

- **GIVEN** una regla cuyas condiciones recibidas difieren de la proyección de sus 14 campos y de su columna `is_tor`
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo avanzado
- **AND** la lista SHALL sembrarse con las condiciones recibidas

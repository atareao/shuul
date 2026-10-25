## MODIFIED Requirements

### Requirement: Las condiciones de una regla se persisten en `rule_conditions`

El sistema SHALL persistir el **conjunto ordenado de condiciones** de cada regla en la tabla `rule_conditions`, con una fila por condición: `(rule_id, position, collection, key, transformations, operator, pattern, negate)`.

`rule_id` SHALL referenciar `rules(id)` con `ON DELETE CASCADE`; el par `(rule_id, position)` SHALL ser único; `position` SHALL fijar el orden de evaluación de las condiciones dentro de la regla.

El vocabulario de colecciones SHALL incluir los 14 campos actuales y la colección booleana `is_tor`. La columna `key` SHALL ser nula para las colecciones que no exigen clave y SHALL llevar el nombre de la cookie, la cabecera o el argumento para las tres colecciones con clave. La columna `operator` SHALL llevar el **nombre canónico** del operador de la condición —`@rx`, `@truthy`, `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` o `@unconditionalMatch`— y la columna `pattern` SHALL llevar su **patrón canónico**: el patrón tal como se compiló para `@rx` y para los operadores de cadena, la lista de elementos recortados unida por coma sin espacios para `@within`, y la cadena vacía para `@truthy` y `@unconditionalMatch`. `negate` SHALL ser siempre `0`.

El round-trip de una condición SHALL ser **estable**: persistir lo leído y volver a leerlo SHALL dar el mismo operador y el mismo patrón, aunque la forma canónica no sea byte a byte la que se escribió —`@within` canoniza `GET, POST` y `GET ,POST` a `GET,POST`—.

El sistema SHALL persistir **una** condición por cada campo de los 14 que la regla restrinja con un patrón **no vacío y compilable**, más **una** condición para la colección `is_tor` cuando la columna `is_tor` de la regla sea verdadera, con `collection = is_tor`, `operator = @truthy`, `pattern` vacío, `key` nulo y `negate` 0. Un campo ausente, en blanco o cuyo patrón no compile NO SHALL generar fila —su ausencia equivale al brazo neutral del matcheo—, y una columna `is_tor` falsa o nula NO SHALL generar fila. La proyección desde la representación simple SHALL seguir produciendo **solo** los operadores `@rx` y `@truthy`: los operadores de cadena NO SHALL ser alcanzables desde los 14 campos ni desde la columna `is_tor`.

Escribir una regla SHALL escribir las 14 columnas y la columna `is_tor` **y** proyectarlas a `rule_conditions`, de modo que ambas representaciones describan la misma regla.

Al leer, el sistema SHALL reconstruir el operador desde el nombre y el patrón persistidos y SHALL **descartar con un diagnóstico que la nombre** una fila irresoluble: operador desconocido, patrón que no compile, patrón vacío en un operador que lo exige, patrón no vacío en `@unconditionalMatch`, `@truthy` fuera de `is_tor`, `is_tor` con un operador distinto de `@truthy`, u operador de cadena sobre `is_tor`. Descartar en vez de aplicar mal SHALL ser el comportamiento frente al rollback a una imagen anterior, que no conoce los operadores nuevos.

Esta sub-entrega NO SHALL requerir migración: la columna `operator` ya existe, admite texto libre y su valor por defecto es `@rx`.

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

#### Scenario: Cada operador de cadena sobrevive el ciclo escritura-lectura

- **GIVEN** una regla avanzada con una condición por cada operador de cadena —`@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`—
- **WHEN** se crea y se recarga
- **THEN** cada fila SHALL llevar su nombre canónico en `operator` y su patrón canónico en `pattern`
- **AND** el operador y el patrón leídos SHALL ser los mismos que los escritos

#### Scenario: El patrón de `@within` se canoniza y el round-trip es estable

- **GIVEN** una regla avanzada con una condición de operador `@within` y patrón `GET, POST, HEAD`
- **WHEN** se crea y se recarga
- **THEN** el patrón persistido SHALL ser `GET,POST,HEAD`
- **AND** reenviar la condición leída SHALL persistir el mismo patrón

#### Scenario: Un operador desconocido se descarta con diagnóstico

- **GIVEN** una fila de `rule_conditions` con un `operator` que el matcheo no conoce —el caso de una fila escrita por una versión posterior—
- **WHEN** se cargan las reglas
- **THEN** esa condición NO SHALL formar parte del conjunto de la regla
- **AND** SHALL emitirse el diagnóstico que nombra la regla y el operador descartado

#### Scenario: Una fila incoherente con su operador se descarta con diagnóstico

- **GIVEN** una fila de `rule_conditions` con operador `@contains` y `pattern` vacío, y otra con operador `@unconditionalMatch` y `pattern` no vacío
- **WHEN** se cargan las reglas
- **THEN** ninguna de las dos condiciones SHALL formar parte del conjunto de su regla
- **AND** SHALL emitirse el diagnóstico que nombra cada fila descartada

#### Scenario: La proyección simple solo produce `@rx` y `@truthy`

- **GIVEN** una regla descrita con los 14 campos y la columna `is_tor`
- **WHEN** se escribe por la API
- **THEN** sus filas SHALL llevar el operador `@rx` —o `@truthy` para la condición de `is_tor`—
- **AND** NO SHALL generarse ninguna fila con un operador de cadena

### Requirement: La autoría avanzada acepta una lista de condiciones

La creación y la edición de una regla SHALL aceptar un campo opcional `conditions` con una lista ordenada de condiciones.

- Ausente o nulo → **modo simple**: el conjunto de condiciones SHALL proyectarse desde los 14 campos y la columna `is_tor`, como hasta ahora.
- Presente → **modo avanzado**: el conjunto persistido SHALL ser exactamente la lista dada, en su orden, y los 14 campos y la columna `is_tor` NO SHALL participar del matcheo.

Cada condición de la lista SHALL declarar una colección conocida; la clave SHALL ser obligatoria exactamente cuando la colección la exige y NO SHALL admitirse cuando no la exige; el operador SHALL ser ausente o uno de los **ocho** del vocabulario —`@rx`, `@truthy`, `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`—; y las transformaciones SHALL resolverse.

El operador SHALL determinar el patrón que se exige: `@rx` y los operadores de cadena SHALL exigir un patrón **no vacío** —que SHALL compilar como expresión regular cuando el operador sea `@rx`, y que SHALL usarse como literal, sin compilar, cuando sea de cadena—, `@unconditionalMatch` SHALL exigir un patrón **vacío**, y `@truthy` SHALL ser exclusivo de la colección `is_tor`, con `pattern` vacío, `key` ausente y `transformations` vacías. Un operador de cadena sobre `is_tor` NO SHALL admitirse, y `is_tor` SHALL seguir exigiendo `@truthy`.

Una entrada que no cumpla SHALL rechazarse con `400 Bad Request` nombrando el motivo, sin almacenar la regla. `ConditionInput::compile` SHALL seguir siendo el **único** punto de validación de la autoría avanzada, compartido por la creación, la edición y la importación.

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

#### Scenario: Un operador de cadena con patrón se acepta

- **GIVEN** una petición con una condición de la colección `user_agent`, operador `@contains` y patrón `nikto`
- **THEN** la API SHALL aceptarla
- **AND** la condición persistida SHALL quedar con operador `@contains` y ese patrón

#### Scenario: Un operador de cadena con patrón vacío se rechaza (400)

- **GIVEN** una petición con una condición de la colección `path`, operador `@beginsWith` y patrón vacío
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

#### Scenario: `@within` con una lista de elementos se acepta

- **GIVEN** una petición con una condición de la colección `method`, operador `@within` y patrón `GET,POST`
- **THEN** la API SHALL aceptarla

#### Scenario: `@unconditionalMatch` con patrón vacío se acepta

- **GIVEN** una petición con una condición de la colección `referer`, operador `@unconditionalMatch` y patrón vacío
- **THEN** la API SHALL aceptarla

#### Scenario: `@unconditionalMatch` con patrón se rechaza (400)

- **GIVEN** una petición con una condición de la colección `referer`, operador `@unconditionalMatch` y patrón `algo`
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

#### Scenario: Un operador desconocido se rechaza (400)

- **GIVEN** una petición con una condición de la colección `path` y operador `@ipMatch`
- **THEN** la API SHALL responder `400` nombrando el operador no admitido
- **AND** la regla NO SHALL almacenarse

#### Scenario: Un operador de cadena sobre `is_tor` se rechaza (400)

- **GIVEN** una petición con una condición de la colección `is_tor` y operador `@contains`
- **THEN** la API SHALL responder `400`
- **AND** la regla NO SHALL almacenarse

### Requirement: El diálogo de regla ofrece la autoría simple y la avanzada

El diálogo de regla SHALL ofrecer una pestaña de condiciones con un control conmutador entre el **modo simple** y el **modo avanzado**.

En **modo simple** el diálogo SHALL autorizar los 14 campos y SHALL NO enviar el campo `conditions`; el conjunto de condiciones SHALL proyectarse desde los 14 campos y la columna `is_tor`, como hasta ahora.

En **modo avanzado** el diálogo SHALL autorizar una **lista ordenada de condiciones** y SHALL enviarla como el campo `conditions`; los 14 campos SHALL seguir enviándose y almacenándose, pero SHALL NO participar del matcheo.

Por cada condición, en modo avanzado, el diálogo SHALL ofrecer la colección, la clave —solo cuando la colección la exige—, las transformaciones, el **operador** y el patrón.

El selector de operador SHALL ofrecer un catálogo de **siete** operadores: `@rx` —el valor por defecto de una fila nueva—, `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`. El patrón SHALL escribirse en el mismo campo en todos los casos: la expresión regular para `@rx`, la subcadena para `@contains`, `@streq`, `@beginsWith` y `@endsWith`, y la **lista separada por comas** para `@within`.

Para una condición de `is_tor` el diálogo SHALL fijar el operador `@truthy` y SHALL NO ofrecer el selector, ni patrón, ni clave ni transformaciones.

Al elegir `@unconditionalMatch` el diálogo SHALL **ocultar** el campo de patrón y SHALL enviar el patrón vacío, de modo que no viaje un patrón residual que la validación rechazaría.

En modo avanzado el diálogo SHALL deshabilitar las pestañas de los 14 campos y SHALL mostrar una nota de que esos campos se almacenan pero no participan del matcheo.

Una regla **nueva** SHALL abrirse en modo simple.

En modo avanzado el diálogo SHALL NO permitir guardar mientras alguna condición de una colección que exige clave carezca de clave **o** mientras alguna condición cuyo operador exige patrón lo tenga vacío, y SHALL señalar la condición pendiente. La **inferencia de modo simple/avanzado NO SHALL cambiar**: SHALL seguir comparando las condiciones recibidas con la proyección de los 14 campos y de la columna `is_tor`, que solo produce `@rx` y `@truthy`.

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
- **AND** NO SHALL ofrecer el selector de operador, ni patrón, ni clave ni transformaciones para esa condición

#### Scenario: El selector ofrece los siete operadores y una fila nueva trae `@rx`

- **GIVEN** el diálogo en modo avanzado
- **WHEN** se añade una condición
- **THEN** su operador SHALL ser `@rx`
- **AND** el selector SHALL ofrecer `@rx`, `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`

#### Scenario: `@unconditionalMatch` oculta el patrón y lo envía vacío

- **GIVEN** una fila del diálogo en modo avanzado con un patrón escrito
- **WHEN** se cambia su operador a `@unconditionalMatch`
- **THEN** el campo de patrón SHALL ocultarse
- **AND** al guardar, el patrón de esa condición SHALL viajar vacío

#### Scenario: Un patrón vacío impide guardar salvo en `@unconditionalMatch`

- **GIVEN** el diálogo en modo avanzado con una condición de operador `@streq` y patrón vacío
- **THEN** el diálogo SHALL impedir guardar
- **AND** SHALL señalar la condición pendiente
- **AND** con el operador `@unconditionalMatch` y patrón vacío SHALL permitir guardar

#### Scenario: Una regla avanzada con operadores de cadena abre en modo avanzado

- **GIVEN** una regla cuyas condiciones recibidas llevan operadores de cadena
- **WHEN** se abre su diálogo
- **THEN** SHALL abrirse en modo avanzado
- **AND** la lista SHALL sembrarse con las condiciones recibidas, conservando el operador de cada fila

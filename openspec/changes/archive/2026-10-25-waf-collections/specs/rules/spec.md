## ADDED Requirements

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

## ADDED Requirements

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

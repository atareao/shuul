# app-version Specification

## Purpose

Cómo se declara la versión publicada en el build de producción del frontend y dónde se
muestra dentro del panel de administración. Cubre las tres piezas que tienen que contar la
misma versión —el fichero que gobierna el versionado del proyecto, el entorno del build de
producción y la interfaz— y la red de seguridad que evita que la interfaz imprima
`undefined` cuando la declaración falta.

## Requirements

### Requirement: La versión publicada se muestra y nunca aparece como undefined

El build de producción del frontend SHALL declarar la versión publicada, y esa declaración SHALL coincidir con la versión que gobierna el versionado del proyecto.

La versión SHALL mostrarse en el título del panel de administración y en el pie de página del panel.

Cuando la versión no estuviera disponible, la interfaz NO SHALL imprimir `undefined`: SHALL mostrar el nombre de la aplicación sin la versión.

#### Scenario: El build de producción declara la versión
- **WHEN** se inspecciona el frontend
- **THEN** SHALL existir una declaración de la versión para el modo de producción
- **AND** SHALL declarar la misma versión que el fichero que gobierna el versionado del proyecto
- **AND** el versionado del proyecto SHALL mantener esa declaración en sincronía

#### Scenario: El panel muestra la versión
- **WHEN** se renderiza el panel de administración
- **THEN** el título del panel y su pie de página SHALL mostrar la versión publicada

#### Scenario: Sin versión no se imprime undefined
- **WHEN** la versión no está disponible
- **THEN** el título SHALL mostrar el nombre de la aplicación sin paréntesis
- **AND** NO SHALL aparecer la cadena `undefined`

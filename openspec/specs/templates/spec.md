# templates Specification

## Purpose
Define el catálogo completo de templates WAF y JAIL, su estructura, orden de prioridad y comportamiento esperado para facilitar la aplicación de reglas de seguridad preconfiguradas.

## Requirements

### Requirement: Template structure

Cada template SHALL tener nombre, descripción, categoría, severidad, pipeline (waf/jail), peso (weight), modo allow, y filtros opcionales (path, query, country_code, user_agent, method, etc.). Los templates SHALL usar un builder pattern para eliminar boilerplate.

#### Scenario: Template with only path filter
- **WHEN** se define un template WAF con solo filtro path
- **THEN** los 13 filtros restantes SHALL ser `None` por defecto (sin repetirlos explícitamente)

#### Scenario: Template with multiple filters
- **WHEN** se define un template con path y query
- **THEN** solo esos dos filtros SHALL especificarse; el builder SHALL inicializar el resto a `None`

### Requirement: WAF template ordering by priority

Los templates WAF SHALL listarse en orden descendente de necesidad: primero los must-have (críticos de seguridad), luego los específicos por CMS/infra, y finalmente los de geo/bajo impacto.

#### Scenario: Must-have templates appear first
- **WHEN** se listan los templates WAF
- **THEN** los templates con `must_have: true` SHALL aparecer antes que los que tienen `must_have: false`

#### Scenario: Priority ordering within must-have
- **WHEN** se listan los templates WAF must-have
- **THEN** SHALL aparecer en este orden: SQL injection, XSS, Command injection, Log4j, Path traversal, Archivos sensibles, Directorios de sistema, Docker socket, phpinfo, UA vacío, UA bots, Apache/Nginx status, PHPUnit probe

#### Scenario: Priority ordering within specific templates
- **WHEN** se listan los templates WAF específicos
- **THEN** SHALL aparecer en este orden: WordPress uploads, Adminer+phpMyAdmin, Laravel, Swagger/API docs, API admin, Drupal install, Symfony, PrestaShop install, xmlrpc, Geo blocking

### Requirement: JAIL template ordering by priority

Los templates JAIL SHALL listarse en orden descendente de necesidad: primero los catch-all/scanners, luego los de protección de servicios, y finalmente los de escenarios específicos.

#### Scenario: Scanner templates appear first
- **WHEN** se listan los templates JAIL
- **THEN** los templates de scanner/catch-all SHALL aparecer antes que los de protección de servicios

#### Scenario: Priority ordering within JAIL
- **WHEN** se listan los templates JAIL
- **THEN** SHALL aparecer en este orden: Scanner 404, Scanner vulnerabilidades, Scanner directorios+configs+paneles, Global Shield, Login endpoints, Admin panels, xmlrpc, Paneles gestión, Auth endpoints, API REST, GraphQL, Search, File upload, Comment spam, Webmail, Scraping país, Health & Webhooks, Recidive

### Requirement: Weight alignment with pipeline

Los pesos (weight) de los templates SHALL estar alineados con el flujo weight ASC del pipeline WAF (primera regla que matchea gana). Los templates del pipeline JAIL SHALL declarar peso 0: el pipeline de Jail itera todas las reglas que casan, sin `break`, así que su orden no decide nada y un peso distinto de 0 sería una decisión sin efecto que la interfaz, además, oculta.

#### Scenario: WAF must-have have lowest weights
- **WHEN** se examinan los templates WAF must-have
- **THEN** sus pesos SHALL estar en rango 10-100 (evalúan primero)

#### Scenario: WAF specific templates have medium weights
- **WHEN** se examinan los templates WAF específicos
- **THEN** sus pesos SHALL estar en rango 100-300

#### Scenario: JAIL catch-all have lowest weights
- **WHEN** se examinan los templates JAIL catch-all/scanner
- **THEN** sus pesos SHALL ser 0 (el mínimo posible: el pipeline de Jail no ordena, así que no hay rango que respetar)

#### Scenario: JAIL service protection have medium weights
- **WHEN** se examinan los templates JAIL de protección de servicios
- **THEN** sus pesos SHALL ser 0, igual que los catch-all, porque el pipeline de Jail no ordena y el peso no decide

### Requirement: Pipeline consistency

Ningún template con pipeline `jail` SHALL tener `allow: false`. Los templates JAIL SHALL siempre tener `allow: true` (el bloqueo lo determina el rate limiter, no la regla en sí).

#### Scenario: Jail templates always allow
- **WHEN** se define un template con `pipeline: "jail"`
- **THEN** `allow` SHALL ser `true`

#### Scenario: PHPUnit probe moved to WAF
- **WHEN** se define el template PHPUnit probe
- **THEN** su pipeline SHALL ser `"waf"` y `allow` SHALL ser `false`

#### Scenario: Drupal xmlrpc moved to WAF
- **WHEN** se define el template Drupal xmlrpc
- **THEN** su pipeline SHALL ser `"waf"` y `allow` SHALL ser `false`

#### Scenario: WordPress xmlrpc moved to WAF
- **WHEN** se define el template WordPress xmlrpc
- **THEN** su pipeline SHALL ser `"waf"` y `allow` SHALL ser `false`

### Requirement: Template count

El número total de templates SHALL reducirse significativamente eliminando redundancia.

#### Scenario: WAF template count
- **WHEN** se cuentan los templates WAF
- **THEN** SHALL haber exactamente 24 templates (23 existentes + 1 nuevo: Tor exit nodes)

#### Scenario: JAIL template count
- **WHEN** se cuentan los templates JAIL
- **THEN** SHALL haber exactamente 18 templates (16 existentes + 2 nuevos: Health & Webhooks, Recidive)

### Requirement: Severity simplification

La severidad en Rust SHALL usar strings cortos (`"critico"`, `"alto"`, `"medio"`, `"bajo"`) en lugar de emojis. El frontend SHALL mapear estos valores a emojis para visualización.

#### Scenario: Severity values in Rust
- **WHEN** se define un template en Rust
- **THEN** severity SHALL ser uno de: `"critico"`, `"alto"`, `"medio"`, `"bajo"`

#### Scenario: Frontend maps severity to emoji
- **WHEN** el frontend recibe severity `"critico"`
- **THEN** SHALL mostrar `🔥 Crítico`
- **WHEN** el frontend recibe severity `"alto"`
- **THEN** SHALL mostrar `🔴 Alto`
- **WHEN** el frontend recibe severity `"medio"`
- **THEN** SHALL mostrar `🟡 Medio`
- **WHEN** el frontend recibe severity `"bajo"`
- **THEN** SHALL mostrar `🟢 Bajo`

### Requirement: New templates

Se SHALL añadir dos nuevos templates JAIL que actualmente no existen en el catálogo.

#### Scenario: Health & Webhooks template
- **WHEN** se lista el catálogo JAIL
- **THEN** SHALL existir un template "Health & Webhooks" con `rate_limit_profile_id: 6` (P6) y path `^/(health|healthz|ready|webhook|api/webhook)`

#### Scenario: Recidive template
- **WHEN** se lista el catálogo JAIL
- **THEN** SHALL existir un template "Recidive" con `rate_limit_profile_id: 7` (P7) y sin filtros de path (catch-all para reincidentes)

### Requirement: Tor exit nodes template

El catálogo WAF SHALL incluir un template "Tor exit nodes" que permita bloquear tráfico proveniente de nodos de salida de la red Tor.

#### Scenario: Tor exit nodes template exists
- **WHEN** se lista el catálogo WAF
- **THEN** SHALL existir un template "Tor exit nodes" con `is_tor: true`, `allow: false`, `pipeline: "waf"`, `must_have: false`, `weight: 300`, y categoría `"tor"`

#### Scenario: Tor exit nodes template appears after Geo blocking
- **WHEN** se listan los templates WAF específicos
- **THEN** "Tor exit nodes" SHALL aparecer después de "Geo blocking" (último en la lista de específicos, peso 300)

### Requirement: Templates are presented as a single filterable listing

La biblioteca de plantillas SHALL presentarse como **un único listado tabular**, sin
pestañas ni acordeones, con una fila por plantilla de regla y columnas de nombre,
pipeline, categoría, severidad, acción, perfil de rate limit (solo JAIL) y una acción
de aplicación.

El listado SHALL ofrecer filtros por pipeline, severidad, categoría y acción, además
de la búsqueda por texto. La condición `must_have` SHALL ser **un filtro**, no un
grupo, sección o acordeón.

El detalle de los filtros de una plantilla SHALL mostrarse en la propia fila
(expandible) y NO SHALL ocupar el espacio de la lista con etiquetas de resumen.

El orden por defecto del listado SHALL respetar el orden del catálogo: primero las
plantillas `must_have` y después el resto por peso.

#### Scenario: All rule templates appear in one list
- **WHEN** el usuario abre la biblioteca de plantillas
- **THEN** SHALL ver 42 filas en un único listado (24 WAF y 18 JAIL)
- **AND** NO SHALL haber pestañas que separen WAF de JAIL
- **AND** NO SHALL haber ninguna sección ni acción de perfiles de rate limit en la página

#### Scenario: Filtering by pipeline
- **WHEN** el usuario filtra por pipeline JAIL
- **THEN** SHALL listarse solo las 18 plantillas JAIL
- **AND** al quitar el filtro SHALL volver a verse el catálogo completo

#### Scenario: Filtering by severity
- **WHEN** el usuario filtra por severidad `"critico"`
- **THEN** SHALL listarse solo las plantillas con `severity: "critico"`

#### Scenario: Must-have is a filter
- **WHEN** el usuario activa el filtro "must have"
- **THEN** SHALL listarse las 16 plantillas con `must_have: true`
- **AND** el catálogo completo SHALL seguir accesible desactivando el filtro

#### Scenario: Filters combine with free-text search
- **WHEN** el usuario filtra por pipeline JAIL y escribe "scanner" en la búsqueda
- **THEN** SHALL listarse solo las plantillas JAIL cuyo nombre o descripción contenga "scanner"

### Requirement: The templates endpoint no longer serves rate limit profiles

`GET /api/v1/templates` SHALL devolver únicamente las plantillas de regla agrupadas
por pipeline (`waf` y `jail`). La respuesta NO SHALL incluir una clave `profiles`: la
gestión de perfiles de rate limit SHALL vivir exclusivamente en
`GET /api/v1/rate-limit-profiles` y en su página dedicada.

#### Scenario: Response contains only rule templates
- **WHEN** se llama a `GET /api/v1/templates`
- **THEN** la respuesta SHALL incluir las claves `waf` y `jail`
- **AND** NO SHALL incluir la clave `profiles`

#### Scenario: Profile templates no longer exist in the backend code
- **WHEN** se inspecciona `backend/src/templates.rs`
- **THEN** NO SHALL existir la función `all_rate_limit_profile_templates()`
- **AND** NO SHALL existir el tipo `RateLimitProfileTemplate`
- **AND** los 9 seeds de perfiles SHALL seguir existiendo únicamente en las migraciones SQL de `rate_limit_profiles`

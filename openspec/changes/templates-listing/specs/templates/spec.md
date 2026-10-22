## ADDED Requirements

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

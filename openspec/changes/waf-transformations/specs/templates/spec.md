# templates Specification (delta)

## ADDED Requirements

### Requirement: Una plantilla declara su pipeline de transformaciones

Cada plantilla SHALL declarar su especificación de transformaciones, con la cadena vacía como valor por defecto. El endpoint `GET /api/v1/templates` SHALL servirla junto al resto de los campos de la plantilla.

Aplicar una plantilla SHALL trasladar su pipeline a la regla creada, de modo que la regla resultante SHALL declarar exactamente la misma especificación que la plantilla.

Las cuatro plantillas de detección SHALL declarar la pipeline que hace que su patrón alcance las evasiones codificadas:

| Plantilla | Filtro | Pipeline |
|---|---|---|
| `Path traversal probes` | `path` | `urlDecodeUni,removeNulls` |
| `SQL injection probes` | `query` | `urlDecodeUni,replaceNulls,compressWhitespace` |
| `XSS probes` | `query` | `urlDecodeUni,htmlEntityDecode,removeNulls` |
| `Command injection probes` | `query` | `urlDecodeUni,removeNulls` |

Las plantillas de Jail NO SHALL declarar pipeline alguna: SHALL quedarse con la cadena vacía.

#### Scenario: El endpoint sirve la pipeline de cada plantilla
- **GIVEN** una petición a `GET /api/v1/templates`
- **WHEN** el handler responde
- **THEN** cada plantilla de `waf` y de `jail` SHALL llevar el campo `transformations`

#### Scenario: Aplicar una plantilla traslada su pipeline
- **GIVEN** la plantilla `Path traversal probes`
- **WHEN** se aplica para crear una regla
- **THEN** la regla creada SHALL declarar `urlDecodeUni,removeNulls`

#### Scenario: La plantilla de detección casa una evasión codificada
- **GIVEN** una regla creada desde `Path traversal probes`
- **WHEN** se evalúa contra una petición cuyo `path` es `%2e%2e%2f`
- **THEN** la regla SHALL casar

#### Scenario: Las plantillas de Jail no declaran pipeline
- **GIVEN** el catálogo de plantillas
- **WHEN** se recorren las plantillas cuyo pipeline es `jail`
- **THEN** todas SHALL declarar la cadena vacía como especificación

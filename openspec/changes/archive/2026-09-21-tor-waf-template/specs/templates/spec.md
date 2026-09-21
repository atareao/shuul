# templates Specification

## MODIFIED Requirements

### Requirement: Template count

El número total de templates SHALL reducirse significativamente eliminando redundancia.

#### Scenario: WAF template count
- **WHEN** se cuentan los templates WAF
- **THEN** SHALL haber exactamente 24 templates (23 existentes + 1 nuevo: Tor exit nodes)

#### Scenario: JAIL template count
- **WHEN** se cuentan los templates JAIL
- **THEN** SHALL haber exactamente 18 templates (16 existentes + 2 nuevos: Health & Webhooks, Recidive)

## ADDED Requirements

### Requirement: Tor exit nodes template

El catálogo WAF SHALL incluir un template "Tor exit nodes" que permita bloquear tráfico proveniente de nodos de salida de la red Tor.

#### Scenario: Tor exit nodes template exists
- **WHEN** se lista el catálogo WAF
- **THEN** SHALL existir un template "Tor exit nodes" con `is_tor: true`, `allow: false`, `pipeline: "waf"`, `must_have: false`, `weight: 300`, y categoría `"tor"`

#### Scenario: Tor exit nodes template appears after Geo blocking
- **WHEN** se listan los templates WAF específicos
- **THEN** "Tor exit nodes" SHALL aparecer después de "Geo blocking" (último en la lista de específicos, peso 300)
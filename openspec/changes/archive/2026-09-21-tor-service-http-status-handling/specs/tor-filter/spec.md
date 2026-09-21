## ADDED Requirements

### Requirement: TorService HTTP status check

El `TorService.refresh()` SHALL verificar el código de estado HTTP de la respuesta antes de procesar el body. Si el status no es 2xx, SHALL preservar el conjunto de exit nodes existente (stale data) y loggear un warning. No SHALL reemplazar el conjunto con uno vacío ni intentar parsear el body.

#### Scenario: Refresh with non-2xx response preserves stale data
Given TorService has a previously loaded set of exit nodes
And a subsequent refresh returns a non-2xx HTTP status (e.g., 429 Too Many Requests)
When `refresh()` completes
Then the existing set of exit nodes SHALL be preserved (not replaced with empty set)
And a warning SHALL be logged

#### Scenario: Refresh with 2xx response updates data normally
Given TorService has a previously loaded set of exit nodes
And a subsequent refresh returns a 200 OK status with new data
When `refresh()` completes
Then the set of exit nodes SHALL be updated with the new data
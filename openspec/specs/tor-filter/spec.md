# tor-filter Specification

## Purpose
Define el servicio de detección de IPs de Tor exit nodes, su ciclo de vida (refresh periódico, inicialización lazy), y cómo se integra en la construcción de `NewRequest` para poblar el campo `is_tor`.

## Requirements

### Requirement: TorService lazy initialization

El `TorService` SHALL inicializarse con un conjunto vacío de exit nodes. El primer refresh SHALL ocurrir a los 30 segundos del arranque. Si el refresh falla, el servicio SHALL mantener el último conjunto válido (o vacío si nunca se ha completado).

#### Scenario: TorService starts with empty set
Given TorService is initialized
When `is_exit_node()` is called before any successful refresh
Then it returns `None` (unknown)

#### Scenario: TorService returns Some after first refresh
Given TorService has completed its first successful refresh
When `is_exit_node()` is called with a known Tor exit node IP
Then it returns `Some(true)`

#### Scenario: TorService returns false for non-Tor IP
Given TorService has completed a successful refresh
When `is_exit_node()` is called with a non-Tor IP
Then it returns `Some(false)`

#### Scenario: TorService refresh failure keeps stale data
Given TorService has a previously loaded set of exit nodes
And a subsequent refresh fails (network error)
When `is_exit_node()` is called
Then it returns the value based on the stale set (graceful degradation)

### Requirement: TorService refresh interval

El `TorService` SHALL refrescar su lista de exit nodes cada 1800 segundos (30 minutos) mediante un background task.

#### Scenario: Refresh interval is 1800s
Given TorService is running
When 1800 seconds have elapsed since the last refresh
Then a new HTTP request SHALL be made to `https://check.torproject.org/torbulkexitlist`

### Requirement: TorService integration in NewRequest

El `NewRequest::from_request()` SHALL aceptar un parámetro `Option<&TorService>` y poblar el campo `is_tor` si el servicio está disponible y el IP está presente.

#### Scenario: TorService available and IP present
Given TorService is available and has loaded exit nodes
And the request has an IP address `185.220.101.1` (a Tor exit node)
When `from_request()` is called with `Some(tor_service)`
Then `request.is_tor` SHALL be `Some(true)`

#### Scenario: TorService not available
Given TorService is `None`
When `from_request()` is called with `None`
Then `request.is_tor` SHALL be `None`

#### Scenario: No IP in request
Given TorService is available
And the request has no IP address
When `from_request()` is called
Then `request.is_tor` SHALL be `None`

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

# Proposal

## Why

El `TorService.refresh()` no verifica el código de estado HTTP antes de procesar el body de la respuesta. Cuando el servidor de Tor devuelve un 429 Too Many Requests (o cualquier otro código no-2xx), el body contiene un mensaje de error ("Too Many Requests") en lugar de IPs. `parse_exit_list` intenta parsear ese mensaje como IP, falla ruidosamente, y **reemplaza el conjunto válido de exit nodes con un conjunto vacío**, perdiendo la capacidad de detectar tráfico Tor hasta el próximo refresh exitoso.

## What Changes

- `TorService.refresh()` SHALL verificar `resp.status().is_success()` antes de procesar el body
- Si el status no es 2xx, SHALL loggear un warning y preservar el conjunto stale (return early sin modificar el estado)
- Se añade un nuevo escenario en los tests: refresh con respuesta HTTP no-2xx preserva datos stale

## Capabilities

### New Capabilities

None — no se introduce una nueva capability, solo se modifica el comportamiento existente.

### Modified Capabilities

- `tor-filter`: Se añade un requirement: "TorService SHALL check HTTP response status before processing body. Non-2xx responses SHALL preserve the existing set."

## Impact

- `backend/src/models/tor_service.rs`: modificar `refresh()` para verificar status code
- Tests existentes se mantienen verdes; se añade un nuevo test
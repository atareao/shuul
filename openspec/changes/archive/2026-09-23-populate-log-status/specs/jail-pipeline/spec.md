## MODIFIED Requirements

### Requirement: Jail audit log categories

El pipeline Jail registra eventos de auditoría con los siguientes nombres:

- `cleared` (pipe: jail): No hay reglas Jail que matcheen, o matchean pero status_code ∉ fail_codes
- `registered` (pipe: jail): Regla matcheó + status_code ∈ fail_codes → stats registradas, sin ban aún
- `pending` (pipe: jail): Threshold de rate limit excedido → PendingBan creado

Todos los eventos de auditoría del pipeline Jail deben incluir `status_code` con el valor del `ReportPayload.status_code`.

#### Scenario: Cleared when no rules match
- **GIVEN** un reporte con status_code `200`
- **AND** ninguna regla Jail activa matchea
- **WHEN** el Jail pipeline procesa
- **THEN** se logea `cleared` con pipe `jail`

#### Scenario: Cleared when status not in fail_codes
- **GIVEN** una regla Jail activa matchea con `rate_limit_profile_id = 3`
- **AND** el perfil 3 tiene `fail_codes = [403, 404]`
- **AND** el status_code del reporte es `200`
- **WHEN** el Jail pipeline procesa
- **THEN** se logea `cleared` con pipe `jail`
- **AND** no se registra stats ni se evalúa rate limit

#### Scenario: Registered when status in fail_codes, no ban
- **GIVEN** una regla Jail activa matchea con `rate_limit_profile_id = 3`
- **AND** el perfil 3 tiene `max_retry = 20`, `find_time_seconds = 60`
- **AND** el status_code del reporte es `404` (∈ fail_codes)
- **AND** la IP ha hecho 5 requests en los últimos 60s (menos de 20)
- **WHEN** el Jail pipeline procesa
- **THEN** se registra stats (record_blocked)
- **AND** se logea `registered` con pipe `jail`
- **AND** NO se logea `sanctioned`
- **AND** la IP NO es baneada

#### Scenario: Sanctioned when threshold exceeded
- **GIVEN** una regla Jail activa matchea con `rate_limit_profile_id = 3`
- **AND** el perfil 3 tiene `max_retry = 20`, `find_time_seconds = 60`
- **AND** el status_code del reporte es `404` (∈ fail_codes)
- **AND** la IP ha hecho 20+ requests en los últimos 60s (excede threshold)
- **WHEN** el Jail pipeline procesa
- **THEN** se registra stats (record_blocked)
- **AND** se logea `sanctioned` con pipe `jail`
- **AND** la IP es baneada via BanManager
- **AND** NO se logea `registered`

#### Scenario: Cleared log includes status_code
- **GIVEN** un reporte con status_code `404`
- **AND** ninguna regla Jail activa matchea
- **WHEN** el Jail pipeline procesa y logea `cleared`
- **THEN** el `LogEntry` tiene `status_code = 404`

#### Scenario: Registered log includes status_code
- **GIVEN** un reporte con status_code `500`
- **AND** una regla Jail matchea con fail_codes que incluye 500
- **AND** el rate limit NO se excede
- **WHEN** el Jail pipeline procesa y logea `registered`
- **THEN** el `LogEntry` tiene `status_code = 500`

#### Scenario: Pending log includes status_code
- **GIVEN** un reporte con status_code `403`
- **AND** una regla Jail matchea con fail_codes que incluye 403
- **AND** el rate limit se excede
- **WHEN** el Jail pipeline procesa y logea `pending`
- **THEN** el `LogEntry` tiene `status_code = 403`
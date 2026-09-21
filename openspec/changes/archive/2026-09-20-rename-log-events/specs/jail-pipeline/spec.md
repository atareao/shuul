# Spec Delta

## Purpose

Define el pipeline Jail (rate limiting post-factum), sus eventos de auditoría y el flujo de rate limiting fail2ban-style.

## ADDED Requirements

### Requirement: Jail pipeline flow

El endpoint `/api/v1/report` recibe reportes del plugin Traefik en modo fire-and-forget. Siempre devuelve 200 OK. Itera TODAS las reglas activas (sin break) que tengan `rate_limit_profile_id`.

#### Scenario: Report received and processed
- **GIVEN** un reporte con IP `1.2.3.4`, path `/test`, status_code `404`
- **WHEN** el Jail pipeline procesa el reporte
- **THEN** itera todas las reglas activas con `rate_limit_profile_id`
- **AND** devuelve 200 OK siempre

#### Scenario: No jail rules match
- **GIVEN** un reporte con IP `1.2.3.4`, path `/test`, status_code `200`
- **AND** ninguna regla Jail activa matchea el request
- **WHEN** el Jail pipeline procesa el reporte
- **THEN** se logea un evento `cleared` con pipe `jail`
- **AND** devuelve 200 OK

#### Scenario: WAF-only rules are skipped in Jail
- **GIVEN** una regla con `pipeline = "waf"` y `rate_limit_profile_id = Some(3)`
- **WHEN** el Jail pipeline itera las reglas
- **THEN** la regla WAF es ignorada (no se evalúa en Jail)

### Requirement: Jail audit log categories

El pipeline Jail registra eventos de auditoría con los siguientes nombres:

- `cleared` (pipe: jail): No hay reglas Jail que matcheen, o matchean pero status_code ∉ fail_codes
- `registered` (pipe: jail): Regla matcheó + status_code ∈ fail_codes → stats registradas, sin ban aún
- `sanctioned` (pipe: jail): Threshold de rate limit excedido → IP baneada

`registered` y `sanctioned` son mutuamente excluyentes por cada par (request, regla). Si hay ban, solo se logea `sanctioned`.

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

### Requirement: Rate limiting behavior

Cada regla Jail con `rate_limit_profile_id` es un "jail" independiente. Para cada match:
1. Carga el perfil desde DB
2. Si `status_code ∈ profile.fail_codes`:
   - `record_blocked()` en stats
   - Obtiene/crea `RateLimiter` para ese profile_id
   - `rl.record(ip)`
   - Si excede threshold → `ban_manager.ban(ip, rule_id, reason, duration)`
   - Persiste el ban en DB

#### Scenario: Rate limiter is per-profile, not per-rule
- **GIVEN** dos reglas Jail distintas que usan el mismo `rate_limit_profile_id = 3`
- **WHEN** ambas matchean el mismo reporte
- **THEN** comparten el mismo `RateLimiter` (entrada en `rate_limiters` HashMap por profile_id)

#### Scenario: Ban is persisted to database
- **GIVEN** un rate limit threshold excedido para IP `1.2.3.4`
- **WHEN** el Jail pipeline banea la IP
- **THEN** el ban se persiste en la tabla `bans` via `BanManager::persist_ban()`
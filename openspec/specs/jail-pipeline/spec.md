# jail-pipeline Specification

## Purpose
Define el pipeline Jail (rate limiting post-factum), sus eventos de auditoría y el flujo de rate limiting fail2ban-style.

## Requirements

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

### Requirement: Jail pipeline marks pending bans, WAF pipeline executes them

The Jail pipeline SHALL NOT ban IPs directly. When `RateLimiter::record(ip)` returns `true` (threshold exceeded), the Jail pipeline SHALL create a `PendingBan` with the IP, rule_id, reason, ban duration, and escalation level, and push it to `pending_bans` in AppState.

The WAF pipeline SHALL check `pending_bans` when no WAF rule matches a request. For each `PendingBan` matching the request's IP, the WAF pipeline SHALL verify if the request matches the rule associated with the ban (using `CacheRule::matches()`). If it matches, the WAF pipeline SHALL execute the ban via `BanManager::ban()` and return 403 FORBIDDEN.

This ensures that:
- The WAF pipeline is the sole authority that executes bans
- WAF allow rules are respected (ban check only occurs when no WAF rule matches)
- Bans are associated with specific rules (not just IPs)

#### Scenario: Jail marks pending ban, WAF executes it
- **GIVEN** a Jail rule with `rate_limit_profile_id = 3` (max_retry=3, find_time=60s)
- **AND** IP `1.2.3.4` has made 3 requests that match the rule with status codes in fail_codes
- **WHEN** the Jail pipeline receives the 3rd report for IP `1.2.3.4`
- **THEN** `RateLimiter::record()` returns `true`
- **AND** the Jail pipeline creates a `PendingBan` for IP `1.2.3.4` with the rule's data
- **AND** the Jail pipeline does NOT call `BanManager::ban()`
- **AND** the Jail pipeline returns 200 OK

- **GIVEN** a `PendingBan` exists for IP `1.2.3.4` with `rule_id = 5`
- **AND** a WAF rule with `path = "/admin"` and `allow = false` exists (rule_id = 5)
- **AND** no WAF rule matches the current request from IP `1.2.3.4` for path `/admin`
- **WHEN** the WAF pipeline processes the request
- **THEN** the WAF pipeline finds the `PendingBan` for IP `1.2.3.4`
- **AND** verifies the request matches rule_id = 5 (path = "/admin")
- **AND** calls `BanManager::ban(ip, rule_id, reason, duration)`
- **AND** returns 403 FORBIDDEN

#### Scenario: WAF allow rule prevents pending ban execution
- **GIVEN** a `PendingBan` exists for IP `1.2.3.4` with `rule_id = 5`
- **AND** a WAF rule exists with `ip_address = "1.2.3.4"` and `allow = true` and `weight = 1`
- **WHEN** the WAF pipeline processes a request from IP `1.2.3.4`
- **THEN** the WAF rule matches first (weight 1)
- **AND** the request is ALLOWED with 200 OK
- **AND** the `PendingBan` is NOT executed (ban check is skipped because WAF rule matched)

#### Scenario: Pending ban only applies when request matches the rule
- **GIVEN** a `PendingBan` exists for IP `1.2.3.4` with `rule_id = 5` (path = "/admin")
- **AND** no WAF rule matches the current request from IP `1.2.3.4` for path `/public`
- **WHEN** the WAF pipeline processes the request
- **THEN** the WAF pipeline finds the `PendingBan` for IP `1.2.3.4`
- **AND** verifies the request does NOT match rule_id = 5 (path is `/public`, not `/admin`)
- **AND** does NOT execute the ban
- **AND** returns 200 OK

#### Scenario: Multiple pending bans for same IP
- **GIVEN** two `PendingBan`s exist for IP `1.2.3.4` (rule_id = 5 for path `/admin`, rule_id = 7 for path `/api`)
- **AND** no WAF rule matches the current request for path `/admin`
- **WHEN** the WAF pipeline processes the request
- **THEN** the WAF pipeline finds both `PendingBan`s
- **AND** executes the one matching path `/admin` (rule_id = 5)
- **AND** returns 403 FORBIDDEN

### Requirement: PendingBan cleanup

Pending bans SHALL be cleaned up periodically to prevent memory leaks. A background task SHALL remove `PendingBan` entries that are older than a configurable timeout (default: 60 seconds) without being executed.

#### Scenario: Expired pending ban is removed
- **GIVEN** a `PendingBan` for IP `1.2.3.4` was created 120 seconds ago
- **AND** the cleanup timeout is 60 seconds
- **WHEN** the cleanup task runs
- **THEN** the expired `PendingBan` is removed
- **AND** the IP `1.2.3.4` is NOT banned

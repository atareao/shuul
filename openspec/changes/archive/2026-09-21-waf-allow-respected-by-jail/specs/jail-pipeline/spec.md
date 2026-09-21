# Spec Delta

## ADDED Requirements

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
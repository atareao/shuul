# WAF Pipeline

## ADDED Requirements

### Requirement: Pipeline evaluation order
The WAF endpoint evaluates requests in four sequential phases: WHITELIST → BLACKLIST → BANNED IP → WAF RULES. Each phase either short-circuits (returns immediately) or passes to the next.

#### Scenario: Whitelist overrides blacklist
Given an IP `10.0.0.1` is in both whitelist and blacklist
When a request arrives from that IP
Then the request is ALLOWED (whitelist evaluated first)

#### Scenario: Whitelist overrides banned IP
Given an IP `10.0.0.1` is whitelisted and also banned by JAIL
When a request arrives from that IP
Then the request is ALLOWED (whitelist evaluated before banned IP check)

#### Scenario: Blacklist blocks before banned IP check
Given an IP `10.0.0.1` is blacklisted and also banned by JAIL
When a request arrives from that IP
Then the request is DENIED with 403 FORBIDDEN (blacklist evaluated first, banned IP check never reached)

#### Scenario: Request matches banned IP
Given an IP `10.0.0.5` is banned by JAIL
When a request arrives from IP `10.0.0.5`
And no whitelist or blacklist entry matches
Then the request is DENIED with 403 FORBIDDEN

#### Scenario: Request matches WAF rule (block)
Given a WAF rule exists with `path = "/admin"` and `allow = false`
When a request arrives for path `/admin`
And no whitelist, blacklist, or banned IP matches
Then the request is DENIED with 403 FORBIDDEN

#### Scenario: Request matches WAF rule (allow)
Given a WAF rule exists with `path = "/api/public"` and `allow = true`
When a request arrives for path `/api/public`
And no whitelist, blacklist, or banned IP matches
Then the request is ALLOWED with 200 OK

#### Scenario: No rule matches (pass)
Given no whitelist, blacklist, banned IP, or WAF rule matches
When a request arrives
Then the request is ALLOWED with 200 OK

### Requirement: Settings simplification
The Settings struct is simplified to remove safe_paths, trusted_ips, and trusted_user_agents fields.

#### Scenario: Settings no longer contains bypass lists
Given an administrator views the settings
Then the settings only contain `default_rule_mode`, `log_retention_days`, and `log_all_requests`
And `safe_paths`, `trusted_ips`, and `trusted_user_agents` are no longer present

### Requirement: Audit log categories for new phases
The WAF pipeline records audit log events for whitelist and blacklist matches.

#### Scenario: Whitelist match is logged
Given a request matches a whitelist entry
Then the audit log records a `whitelist` event with the matched entry details

#### Scenario: Blacklist match is logged
Given a request matches a blacklist entry
Then the audit log records a `blacklist` event with the matched entry details
# waf-pipeline Specification (Delta)

## Modified Requirements

### Requirement: Pipeline evaluation order

The WAF endpoint evaluates requests in two sequential phases: WAF RULES → BANNED IP CHECK. Each phase either short-circuits (returns immediately) or passes to the next.

#### Scenario: WAF rule allows before ban check
Given a WAF rule exists with `path = "/health"` and `allow = true` and `mode = "enforce"`
And the requesting IP `10.0.0.1` is banned by JAIL
When a request arrives from IP `10.0.0.1` for path `/health`
Then the request is ALLOWED with 200 OK (WAF rule evaluated first, ban check never reached)

#### Scenario: WAF rule blocks before ban check
Given a WAF rule exists with `path = "/admin"` and `allow = false` and `mode = "enforce"`
When a request arrives for path `/admin`
Then the request is DENIED with 403 FORBIDDEN (WAF rule evaluated first)

#### Scenario: No WAF rule matches, IP is banned
Given no WAF rule matches the request
And the requesting IP `10.0.0.5` is banned by JAIL
When a request arrives from IP `10.0.0.5`
Then the request is DENIED with 403 FORBIDDEN (ban check after WAF)

#### Scenario: No WAF rule matches, IP is not banned
Given no WAF rule matches the request
And the requesting IP is not banned
When a request arrives
Then the request is ALLOWED with 200 OK

#### Scenario: WAF rule with log_only mode
Given a WAF rule exists with `path = "/test"` and `mode = "log_only"`
When a request arrives for path `/test`
Then the request is ALLOWED with 200 OK (log_only always allows)
And the audit log records a `log_only` event

#### Scenario: WAF rule with off mode
Given a WAF rule exists with `path = "/test"` and `mode = "off"`
When a request arrives for path `/test`
Then the rule is skipped (treated as no match)

### Requirement: Settings simplification

The Settings struct is simplified to remove `safe_paths`, `trusted_ips`, and `trusted_user_agents` fields.

#### Scenario: Settings no longer contains bypass lists
Given an administrator views the settings
Then the settings only contain `default_rule_mode`, `log_retention_days`, and `log_all_requests`
And `safe_paths`, `trusted_ips`, and `trusted_user_agents` are no longer present

### Requirement: Audit log categories

The WAF pipeline records audit log events for WAF rule matches and ban checks. Whitelist and blacklist audit categories are removed.

#### Scenario: WAF rule allow is logged
Given a WAF rule matches with `allow = true`
When the request is allowed
Then the audit log records an `allow` event with the matched rule details

#### Scenario: WAF rule block is logged
Given a WAF rule matches with `allow = false`
When the request is denied
Then the audit log records a `block` event with the matched rule details

#### Scenario: Banned IP is logged
Given no WAF rule matches and the IP is banned
When the request is denied
Then the audit log records a `banned` event with the ban reason
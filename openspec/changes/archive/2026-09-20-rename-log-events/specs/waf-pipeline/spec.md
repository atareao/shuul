# Spec Delta

## Purpose
Renames WAF audit log event categories from `allow`/`block` to `admitted`/`denied`, clarifies the `banned` event pipe, and adds a scenario for no-logging when no rule matches and IP is not banned.

## MODIFIED Requirements

### Requirement: Audit log categories

The WAF pipeline records audit log events for WAF rule matches and ban checks. Whitelist and blacklist audit categories are removed. Event names SHALL be `admitted`, `denied`, and `banned`.

#### Scenario: WAF rule allow is logged
Given a WAF rule matches with `allow = true` or `mode = "log_only"`
When the request is allowed with 200 OK
Then the audit log records an `admitted` event with `pipe = "waf"` and the matched rule details

#### Scenario: WAF rule block is logged
Given a WAF rule matches with `allow = false` and `mode = "enforce"`
When the request is denied with 403 FORBIDDEN
Then the audit log records a `denied` event with `pipe = "waf"` and the matched rule details

#### Scenario: Banned IP is logged
Given no WAF rule matches and the IP is banned
When the request is denied with 403 FORBIDDEN
Then the audit log records a `banned` event with `pipe = "waf"` and the ban reason

#### Scenario: No match and not banned logs nothing
Given no WAF rule matches and the IP is not banned
When the request is allowed with 200 OK
Then no audit log event is recorded
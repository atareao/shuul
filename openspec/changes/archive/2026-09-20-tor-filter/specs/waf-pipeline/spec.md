# waf-pipeline Specification

## Purpose
Defines the evaluation order and behavior of the WAF pipeline for request filtering.

## ADDED Requirements

### Requirement: Tor filter matching

The WAF pipeline SHALL support `is_tor` as a boolean filter on rules. When `is_tor = true` on a rule, the rule SHALL only match requests where `request.is_tor = Some(true)`. When `is_tor` is not set (None), the filter SHALL be neutral (passes).

#### Scenario: Tor filter matches Tor traffic
Given a WAF rule exists with `is_tor = true` and `path = "/admin"` and `allow = false` and `mode = "enforce"`
And the request IP `185.220.101.1` is a Tor exit node
When a request arrives from that IP for path `/admin`
Then the rule SHALL match
And the request SHALL be DENIED with 403 FORBIDDEN

#### Scenario: Tor filter does not match non-Tor traffic
Given a WAF rule exists with `is_tor = true` and `path = "/admin"` and `allow = false` and `mode = "enforce"`
And the request IP `8.8.8.8` is NOT a Tor exit node
When a request arrives from that IP for path `/admin`
Then the rule SHALL NOT match

#### Scenario: Tor filter is neutral when not set
Given a WAF rule exists with no `is_tor` set (None) and `path = "/admin"` and `allow = false` and `mode = "enforce"`
And the request IP `185.220.101.1` is a Tor exit node
When a request arrives from that IP for path `/admin`
Then the rule SHALL match (Tor filter is neutral)
And the request SHALL be DENIED with 403 FORBIDDEN

#### Scenario: Tor filter with allow = true
Given a WAF rule exists with `is_tor = true` and `path = "/public"` and `allow = true` and `mode = "enforce"`
And the request IP `185.220.101.1` is a Tor exit node
When a request arrives from that IP for path `/public`
Then the rule SHALL match
And the request SHALL be ALLOWED with 200 OK

#### Scenario: Tor filter with log_only mode
Given a WAF rule exists with `is_tor = true` and `path = "/test"` and `mode = "log_only"`
And the request IP `185.220.101.1` is a Tor exit node
When a request arrives from that IP for path `/test`
Then the rule SHALL match
And the request SHALL be ALLOWED with 200 OK (log_only always allows)
And the audit log records a `log_only` event
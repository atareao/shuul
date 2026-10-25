# waf-pipeline Specification

## Purpose
Defines the evaluation order and behavior of the WAF pipeline for request filtering.

## Requirements

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

The WAF pipeline records audit log events for WAF rule matches, ban checks, and unmatched pass-through. Event names SHALL be `admitted`, `denied`, `banned`, and `unmatched`.

#### Scenario: WAF rule allow is logged
Given a WAF rule matches with `allow = true` or `mode = "log_only"`
When the request is allowed with 200 OK
Then the audit log records an `admitted` event with `pipe = "waf"` and the matched rule details

#### Scenario: WAF rule block is logged
Given a WAF rule matches with `allow = false` and `mode = "enforce"`
When the request is denied with 403 FORBIDDEN
Then the audit log records a `denied` event with `pipe = "waf"` and the matched rule details

#### Scenario: Banned IP is logged with rule info
Given no WAF rule matches and the IP is banned
And the ban was created by rule "Scanner Aggressive" (rule_id = 9)
When the request is denied with 403 FORBIDDEN
Then the audit log records a `banned` event with `pipe = "waf"` and the ban reason
And the audit log SHALL include `rule_id = 9` and `rule_name = "Scanner Aggressive"`

#### Scenario: No match and not banned logs unmatched
Given no WAF rule matches and the IP is not banned
When the request is allowed with 200 OK
Then the audit log records an `unmatched` event with `pipe = "waf"`

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

### Requirement: El evento de auditoría del WAF anota el valor transformado que casó

Cuando la regla que casa en el pipeline del WAF declare una pipeline de transformaciones **no vacía** **y** restrinja **al menos uno** de los 14 campos de la petición, el evento de auditoría —tanto `admitted` como `denied`— SHALL añadir la clave `"matched"`, un objeto que mapea **cada campo restringido que casó** a su valor **transformado**, junto a los valores crudos que el evento ya lleva.

La forma de la clave SHALL ser un objeto anidado cuyas claves son los nombres canónicos de los 14 campos y cuyos valores son los valores transformados: `"matched": {"path": "../"}`. La clave NO SHALL ser una cadena ni un array.

El valor transformado SHALL ser el que de verdad se comparó con el patrón: es lo que permite afinar una regla sin adivinar por qué casó.

Cuando la regla declare la pipeline **vacía**, el evento NO SHALL llevar la clave `"matched"`: el valor transformado es el crudo, que el evento ya lleva, y añadirlo cambiaría la forma de todos los eventos existentes sin aportar información.

Cuando la regla declare la pipeline **no vacía** pero **no restrinja ninguno** de los 14 campos —solo `is_tor`, o solo `allow`—, el evento NO SHALL llevar la clave `"matched"`: no hay ningún valor transformado que anotar, y un objeto vacío no aportaría información.

Las dos condiciones SHALL cumplirse a la vez: la clave existe **solo** con la pipeline no vacía **y** al menos un campo de los 14 restringido.

#### Scenario: Una denegación con pipeline anota el valor transformado
- **GIVEN** una regla con patrón `\.\./`, pipeline `urlDecodeUni` y filtro sobre `path`
- **WHEN** deniega una petición cuyo `path` es `%2e%2e%2f`
- **THEN** el evento SHALL ser `denied`
- **AND** SHALL llevar la clave `"matched"` con la forma `{"path": "../"}`
- **AND** SHALL seguir llevando el valor crudo del `path`

#### Scenario: Una denegación sin pipeline no anota nada nuevo
- **GIVEN** una regla sin pipeline
- **WHEN** deniega una petición
- **THEN** el evento NO SHALL llevar la clave `"matched"`

#### Scenario: Una admisión con pipeline también lo anota
- **GIVEN** una regla con `allow = true` y pipeline no vacía
- **WHEN** admite una petición
- **THEN** el evento SHALL ser `admitted`
- **AND** SHALL llevar la clave `"matched"` con el valor transformado

#### Scenario: El evento nombra los campos que la regla restringe
- **GIVEN** una regla con pipeline no vacía que restringe únicamente `query`
- **WHEN** casa
- **THEN** la clave `"matched"` SHALL llevar el valor transformado de `query`
- **AND** NO SHALL llevar el de ningún campo no restringido

#### Scenario: Una pipeline sin campo restringido no anota nada
- **GIVEN** una regla con pipeline no vacía que no restringe ningún campo de los 14 —solo `is_tor`—
- **WHEN** casa
- **THEN** el evento NO SHALL llevar la clave `"matched"`

## Purpose

Define el matcheo de una regla contra una petición: qué significa que una regla
casa cuando restringe varios campos a la vez, y —de forma señalada— qué ocurre
cuando la regla restringe un campo que la petición no trae.

## ADDED Requirements

### Requirement: Rule matching with absent request fields

Una regla SHALL casar una petición cuando **todos** sus filtros casen, evaluados
sobre los 14 campos disponibles: `ip_address`, `protocol`, `fqdn`, `path`,
`query`, `city_name`, `country_name`, `country_code`, `user_agent`, `method`,
`referer`, `content_type`, `accept_language` y `x_request_id`. Un filtro que la
regla **no** restringe SHALL ser **neutral** y NO SHALL impedir el match.

La evaluación de cada campo SHALL ser:

```rust
fn check_match(filter: Option<&Regex>, value: Option<&str>) -> bool {
    match (filter, value) {
        (Some(re), Some(v)) => re.is_match(v), // decide el regex
        (Some(_), None) => false,              // la regla restringe y el valor falta
        (None, _) => true,                     // la regla no restringe: neutral
    }
}
```

Es decir:

- si la regla restringe el campo y la petición trae valor, SHALL decidir el
  regex: `true` si casa, `false` si no;
- si la regla restringe el campo y la petición **no** trae valor, la condición
  SHALL considerarse **NO cumplida**, de modo que la regla NO SHALL casar. Casar
  sin evidencia es el fallo peligroso: un filtro de `user_agent` casaría el 100 %
  de las peticiones sin esa cabecera, y en el WAF una regla de denegación
  denegaría tráfico legítimo mientras en el Jail acumularía observaciones de rate
  limit contra él;
- si la regla **no** restringe el campo, la condición SHALL cumplirse tanto si la
  petición trae valor como si no.

Un valor de cabecera **ausente** y uno **presente pero vacío** SHALL ser
indistinguibles a efectos del matcheo: ambos cuentan como ausentes. Una regla NO
SHALL poder expresar «casar por ausencia de valor»: no es una capacidad
soportada.

Cuando un match se descarte **porque la regla restringe un campo y la petición
no lo trae**, el matcher SHALL emitir un `debug!` que nombre el campo y la regla.
El diagnóstico NO SHALL emitirse cuando el valor esté presente y el regex
simplemente no case —ese es un no-match legítimo, información distinta— ni cuando
la regla no restrinja el campo. El diagnóstico NO SHALL ser un error ni SHALL
interrumpir la evaluación.

Este contrato SHALL ser el mismo para los dos pipelines: el WAF, que construye la
petición desde las cabeceras vivas del forwardAuth, y el Jail, que la construye
desde el access log.

#### Scenario: Restricted field present and matching
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "curl/8.5"`
- **WHEN** la regla se evalúa
- **THEN** el campo casa
- **AND** la regla casa si el resto de sus filtros también casan

#### Scenario: Restricted field present and not matching
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "Mozilla/5.0"` que no casa con ese regex
- **WHEN** la regla se evalúa
- **THEN** el campo NO casa
- **AND** la regla NO casa

#### Scenario: Restricted field absent makes the rule not match
- **GIVEN** una regla con un regex de `user_agent` y una petición cuyo `user_agent` es ausente
- **WHEN** la regla se evalúa
- **THEN** el campo NO casa
- **AND** la regla NO casa
- **AND** se emite un `debug!` que nombra el campo `user_agent` y la regla

#### Scenario: One absent field discards a rule that matches on the others
- **GIVEN** una regla con un regex de `path` y un regex de `x_request_id`, y una petición con `path = "/admin"` (que casa) y `x_request_id` ausente
- **WHEN** la regla se evalúa
- **THEN** la regla NO casa
- **AND** el `debug!` nombra `x_request_id`

#### Scenario: An unrestricted field is neutral
- **GIVEN** una regla que sólo restringe `path` y una petición sin `user_agent`, sin `referer` ni `x_request_id`
- **WHEN** la regla se evalúa
- **THEN** los campos no restringidos son neutrales
- **AND** la regla casa si su regex de `path` casa

#### Scenario: A present but empty header counts as absent
- **GIVEN** una regla con un regex de `referer` y una petición cuya cabecera `Referer` está presente pero vacía
- **WHEN** la regla se evalúa
- **THEN** el campo se trata como ausente
- **AND** la regla NO casa

#### Scenario: The same semantics apply to both pipelines
- **GIVEN** una regla con filtro de `user_agent`, alcanzable desde el WAF (forwardAuth) y desde el Jail (access log)
- **WHEN** llega una petición sin cabecera `User-Agent`
- **THEN** la regla NO casa en ninguno de los dos pipelines
- **AND** en el Jail no se acumula ninguna observación de rate limit por esa petición

#### Scenario: A legitimate regex no-match is not diagnosed as an absent field
- **GIVEN** una regla con un regex de `user_agent` y una petición con `user_agent = "Mozilla/5.0"` que no casa con el regex
- **WHEN** la regla se evalúa
- **THEN** la regla NO casa
- **AND** NO se emite el `debug!` de campo ausente, porque el valor estaba presente
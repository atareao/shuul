# request-extraction Specification (delta)

## ADDED Requirements

### Requirement: Todo consumidor de la dirección del cliente usa la dirección resuelta

`NewRequest::from_request()` SHALL resolver la dirección del cliente **una sola vez** y SHALL usar ese mismo valor en **todos** los consumidores que dependan de ella: el campo `ip_address`, la consulta al servicio GeoIP y la consulta al filtro de Tor. NO SHALL entregar la cabecera `X-Forwarded-For` sin resolver a ninguno de ellos.

El valor que SHALL llegar a esos consumidores es el de `ip_address`: **exactamente una** dirección, ya recortada y validada como `IpAddr`, según el requisito «La dirección del cliente se resuelve a una sola dirección».

Cuando `ip_address` sea `None` —cabecera ausente, vacía o irresoluble— la consulta GeoIP y la consulta de Tor NO SHALL ejecutarse, y `city_name`, `country_name`, `country_code` e `is_tor` SHALL ser `None`. NO SHALL emitirse un diagnóstico de sintaxis de dirección procedente de los consumidores.

#### Scenario: Una cadena de saltos con GeoIP disponible
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, 203.0.113.7` y hay un servicio GeoIP disponible
- **THEN** la consulta GeoIP SHALL hacerse contra `203.0.113.7`
- **AND** NO SHALL hacerse contra `"192.0.2.1, 203.0.113.7"`

#### Scenario: Una cadena de saltos con el filtro de Tor disponible
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, 203.0.113.7` y hay un servicio Tor con su lista cargada
- **THEN** la consulta de Tor SHALL hacerse contra `203.0.113.7`
- **AND** `is_tor` SHALL ser `Some(_)`
- **AND** NO SHALL ser `None` por culpa de la cadena

#### Scenario: Una dirección irresoluble no se consulta
- **WHEN** la petición trae `X-Forwarded-For: no-soy-una-direccion`
- **THEN** la consulta GeoIP NO SHALL ejecutarse
- **AND** la consulta de Tor NO SHALL ejecutarse
- **AND** `country_code`, `country_name`, `city_name` e `is_tor` SHALL ser `None`

#### Scenario: Un solo valor sigue resolviéndose igual
- **WHEN** la petición trae `X-Forwarded-For: 8.8.8.8`
- **THEN** la consulta GeoIP y la consulta de Tor SHALL hacerse contra `8.8.8.8`

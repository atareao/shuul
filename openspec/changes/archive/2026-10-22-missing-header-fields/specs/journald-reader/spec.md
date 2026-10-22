## MODIFIED Requirements

### Requirement: Traefik access log mapping

`parse_traefik_access_log` SHALL convertir el `MESSAGE` de una entrada del
journal (el access log JSON de Traefik) en un `ReportPayload`.

```rust
pub fn parse_traefik_access_log(
    json_str: &str,
) -> Result<Option<ReportPayload>, JournalError>;
```

Mapeo de campos (nombres reales del formato JSON de Traefik v3):

| Campo Traefik (JSON) | ReportPayload | Notas |
|---|---|---|
| `ClientHost` | `ip_address` | IP remota. Fallback a `ClientAddr` sin el puerto (IPv4 e IPv6). |
| `OriginStatus` | `status_code` | Status devuelto **por el backend**. |
| `RequestPath` | `path` + `query` | Se divide en el primer `?`. |
| `RequestMethod` | `method` | |
| `RequestHost` | `fqdn` | Host sin puerto. |
| `RequestScheme` | `protocol` | `http` / `https`. |
| `request_User-Agent` | `user_agent` | Los headers llevan prefijo `request_`. |
| `request_Referer` | `referer` | |
| `request_Content-Type` | `content_type` | |
| `request_Accept-Language` | `accept_language` | |
| `request_X-Request-Id` | `x_request_id` | |

De los campos de la tabla, sólo los **cinco derivados de cabecera** —`user_agent`,
`referer`, `content_type`, `accept_language` y `x_request_id`— dependen de la
configuración del origen. Traefik sólo emite las claves `request_User-Agent`,
`request_Referer`, `request_Content-Type`, `request_Accept-Language` y
`request_X-Request-Id` si su bloque `accessLog.fields.headers.names` las declara;
sin ese bloque el `defaultMode` es `drop` y las claves **no aparecen** en el JSON.
Ni la API de Docker ni `systemd-journal-gatewayd` aportan esos campos: ambos
transportan el mismo access log JSON, de modo que la disponibilidad depende del
**origen** y no del **transporte**, y ningún backend puede inventarlos.

En consecuencia, cuando la clave de origen de un campo falte, el mapeo SHALL
resolver ese campo a `None` y NO SHALL descartar la línea por ello: un access log
sin claves `request_*` sigue siendo un access log válido, con su IP, su status,
su path y su método. La ausencia de esos campos es una **degradación del
filtrado**, no un error de parseo, y el lector SHALL NOT confundir las dos cosas.

Traefik reporta dos status: `OriginStatus` (resultado del proxy al backend) y
`DownstreamStatus` (lo devuelto al cliente tras la cadena de middlewares). Según
la documentación de Traefik, cuando `OriginStatus` es `0` la request **nunca
llegó al backend** (la respuesta viene del routing o de un middleware:
autenticación, rate limiting, redirección). Por tanto el reader SHALL descartar
esas entradas, lo que preserva el comportamiento del plugin Go anterior (que
sólo se ejecutaba en requests que habían pasado el WAF forwardAuth).

#### Scenario: Parse a full Traefik access log
- **GIVEN** un access log con `ClientHost`, `OriginStatus: 401`, `RequestPath: "/api/login?next=%2Fdash"`, `RequestMethod: "POST"`, `RequestHost: "example.com"`, `RequestScheme: "https"` y los headers `request_User-Agent`, `request_Referer`, `request_Content-Type`, `request_Accept-Language`, `request_X-Request-Id`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` con `status_code = 401`, `path = "/api/login"`, `query = "next=%2Fdash"`, `protocol = "https"` y el resto de campos mapeados

#### Scenario: Missing optional fields become None
- **GIVEN** un access log con sólo `ClientHost`, `OriginStatus: 200`, `RequestPath: "/"`, `RequestMethod: "GET"` y `RequestHost`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` con `status_code = 200`
- **AND** `query`, `user_agent`, `referer`, `content_type`, `accept_language`, `x_request_id` y `protocol` son `None`

#### Scenario: An access log without the request_ keys keeps the line and maps the header fields to None
- **GIVEN** un access log con `ClientHost`, `OriginStatus: 401`, `RequestPath: "/admin"`, `RequestMethod: "GET"`, `RequestHost` y **ninguna** clave que empiece por `request_`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(Some(payload))` —la línea NO se descarta—
- **AND** `user_agent`, `referer`, `content_type`, `accept_language` y `x_request_id` son `None`
- **AND** el resto de campos mapeados (`ip_address`, `status_code`, `path`, `method`, `fqdn`) conservan su valor

#### Scenario: An access log with only a subset of the request_ keys maps the present ones
- **GIVEN** un access log con `OriginStatus: 200` y sólo las claves `request_User-Agent: "Mozilla/5.0"` y `request_X-Request-Id: "abc-123"`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `user_agent = "Mozilla/5.0"` y `x_request_id = "abc-123"`
- **AND** `referer`, `content_type` y `accept_language` son `None`
- **AND** la línea NO se descarta

#### Scenario: Path without query
- **GIVEN** un access log con `RequestPath: "/missing"` (sin `?`)
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `path = "/missing"` y `query` es `None`

#### Scenario: ClientAddr fallback strips the port
- **GIVEN** un access log sin `ClientHost` pero con `ClientAddr: "192.168.1.5:54321"`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `ip_address = "192.168.1.5"`

#### Scenario: IPv6 ClientAddr fallback strips brackets and port
- **GIVEN** un access log con `ClientAddr: "[2001:db8::1]:443"` y sin `ClientHost`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** `ip_address = "2001:db8::1"`

#### Scenario: Request never proxied to a backend is discarded
- **GIVEN** un access log con `OriginStatus: 0` y `DownstreamStatus: 403`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Missing OriginStatus is discarded
- **GIVEN** un access log sin el campo `OriginStatus`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Missing client IP is discarded
- **GIVEN** un access log sin `ClientHost` ni `ClientAddr`
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Ok(None)`

#### Scenario: Invalid JSON returns an error
- **GIVEN** un texto que no es JSON válido
- **WHEN** `parse_traefik_access_log()` se ejecuta
- **THEN** devuelve `Err(JournalError::TraefikParse)`
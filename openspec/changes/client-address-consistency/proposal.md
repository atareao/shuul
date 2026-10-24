# Proposal: client-address-consistency

## Why

**La dirección del cliente se resuelve, pero no todos la usan.** El change `waf-input-integrity` cerró la resolución: `NewRequest::ip_address` ya contiene **una sola** dirección —el último elemento de la cadena de `X-Forwarded-For`— y ya no la cadena entera. Pero `from_request()` tiene **tres** consumidores de esa cabecera y solo uno se arregló:

- `ip_address` — **arreglado**: resuelve a una sola dirección y valida como `IpAddr`.
- la consulta **GeoIP** (`geoip.lookup(ip)`, `new_request.rs`) — **sin arreglar**: sigue recibiendo la variable cruda `ip`, la cabecera entera.
- la consulta del filtro de **Tor** (`tor_service.is_exit_node(ip)`, `new_request.rs`) — **sin arreglar**: recibe igualmente la variable cruda `ip`.

**Medido en producción el 2026-10-24, tras desplegar v0.23.0.** Una petición con `X-Forwarded-For: 192.0.2.1, 203.0.113.7` al handler del WAF deja este registro:

```
ERROR backend::models::ipdata: Look data for ip: "192.0.2.1, 203.0.113.7": invalid IP address syntax
```

y el campo `ip` del registro de auditoría sale correcto (`203.0.113.7`). Es decir: la resolución funciona, pero la cadena cruda sigue llegando al GeoIP.

**Por qué importa, y por qué ahora.** Con la cadena cruda, los dos consumidores caen en **fail-open**:

- `GeoIpService::lookup` → `IPData::complete` no resuelve una cadena, así que `city_name`, `country_name` y `country_code` quedan en `None`. Según el contrato de `rule-matching`, un campo restringido que llega `None` **no casa**: las reglas por país y por ciudad **dejan de aplicarse**. La plantilla «Geo blocking» —que en producción deniega ~907 peticiones cada 6 h— se queda muda.
- `TorService::is_exit_node` hace `ip.parse::<IpAddr>().ok()?`; con una cadena el `parse` falla y devuelve `None`, así que `is_tor` queda en `None` y las reglas del filtro de Tor **dejan de aplicarse**.

Hoy no se dispara porque el despliegue tiene `trustForwardHeader` en `false` y Traefik sustituye la cabecera: llega un solo valor. **Pero el propio change `waf-input-integrity` recomienda activar `trustForwardHeader: true`** —su tarea 5.3— para callar el aviso de Traefik. Activarlo **introduce cadenas** y, con el código actual, **rompería GeoIP y Tor en fail-open**. El orden que aquel change predica —«resolución primero, confianza después»— está a medias: falta que **todos** los consumidores usen la dirección resuelta.

**Esto no es una capacidad nueva, es el mismo defecto sin terminar.** El `Why` de `waf-input-integrity` enumeraba literalmente estos dos daños («el GeoIP no resuelve… el filtro de Tor no consulta»), pero la implementación solo cubrió `ip_address` y los tests de cadena pasaban `geoip = None` y `tor = None`, así que no lo detectaron.

## What Changes

1. **La dirección se resuelve una sola vez y se pasa a todos.** `from_request()` deja de entregar la variable cruda `ip` a ningún consumidor: la consulta GeoIP y la consulta de Tor reciben el mismo valor que `ip_address`, ya resuelto, recortado y validado.
2. **Sin dirección no hay consulta.** Cuando `ip_address` es `None` —cabecera ausente, vacía o irresoluble— la consulta GeoIP y la consulta de Tor **no se ejecutan**, y `city_name`, `country_name`, `country_code` e `is_tor` quedan en `None`. No se emite un diagnóstico de sintaxis desde los consumidores: el único diagnóstico es el del resolvedor, que ya nombra el valor descartado.
3. **Se desbloquea la confianza en la cabecera.** Cerrar esto es la condición previa para retomar la tarea 5.3 de `waf-input-integrity`: activar `trustForwardHeader: true` con `forwardedHeaders.trustedIPs` sin romper GeoIP ni Tor.

## Capabilities

**Modified Capabilities**

- `request-extraction` — se añade un requisito que obliga a que **todo** consumidor de la dirección del cliente use la dirección resuelta, no la cabecera cruda. El requisito existente «La dirección del cliente se resuelve a una sola dirección» no cambia: cambia **quién la consume**.

**New Capabilities**

- Ninguna.

## Impact

- **Backend**: `backend/src/models/new_request.rs`, `from_request()` — el único fichero de código que cambia. Pasar `ip_address.as_deref()` a `geoip.lookup` y a `tor_service.is_exit_node`, y consultar solo cuando sea `Some`.
- **Docs**: ninguna obligatoria. Se anota el hallazgo medido en `WAF-ROADMAP.md` (diagnóstico) si procede.
- **Spec**: `request-extraction` gana un requisito.
- **Datos y API**: sin migración, sin cambio de API, sin cambio de esquema. El contrato de `ip_address` no se toca; se estrecha el de los campos derivados (`country_*`, `city_name`, `is_tor`) al hacerlos depender de una dirección válida.
- **Fuera de alcance**: el endurecimiento de Traefik en sí (vive en `co1`, fuera del repo), la medición 0.1 de `waf-input-integrity` (si Traefik añade o antepone su elemento) y las transformaciones (`F1` del roadmap).

## Decisions taken when opening the change

- **Un solo resolvedor, un solo valor.** La dirección se resuelve en un punto y se pasa a los tres consumidores. No se duplica la lógica de `rsplit`/`trim`/`parse` en el GeoIP ni en el Tor: eso sería un segundo contrato que mantener y volvería a divergir.
- **Sin dirección válida, no se consulta.** No se llama al GeoIP ni al Tor con un valor que no es una dirección. Además de ser inútil, el GeoIP emite un `ERROR` por cada intento: consultar con basura convierte un caso normal en ruido de log.
- **El diagnóstico sigue siendo uno.** El resolvedor ya nombra el valor descartado en `debug`; los consumidores no añaden el suyo. Una cabecera ausente o vacía sigue sin producir diagnóstico.
- **No se toca `journald_reader.rs`.** Su parser de `ClientHost`/`ClientAddr` es la referencia de corrección y no consume `X-Forwarded-For` del ForwardAuth.
- **Este change no archiva `waf-input-integrity`.** Aquel mantiene abierta su verificación 5.3, que depende de este. La secuencia es: cerrar y desplegar `client-address-consistency`, y **después** retomar la 5.3.

## Límites que se aceptan

- **El orden de archivado respecto a `waf-input-integrity` importa.** Ambos changes añaden requisitos a la misma capability `request-extraction`, que todavía **no existe** en `openspec/specs/` (la materializa el archivado de `waf-input-integrity`). Los requisitos de este change son ADDED y su header NO coincide con ninguno del delta de `waf-input-integrity`, para que el archivado de cualquiera de los dos no rompa por coincidencia de headers.
- **El endpoint `GET /api/v1/util/complete?ip=` no entra.** Es una utilidad de administración que resuelve la IP que el operador le pasa; no consume `X-Forwarded-For`. Si el operador le pasa una cadena, el fallo es de la entrada, no del contrato.
- **La medición 0.1 sigue pendiente y no se resuelve aquí.** Si Traefik antepone su elemento en vez de añadirlo al final, la elección del último elemento habría que revisarla — pero esa decisión la fijó el operador y no la cambia una medición por sí sola.

# Proposal: journald-no-history-replay

## Why

Al arrancar, el reader abre `GET /entries?follow` **sin `Range`**, y gatewayd
devuelve el journal completo del contenedor desde el principio. En el host de
producción eso son más de 6000 entradas de access logs de Traefik.

`RateLimiter::record()` sella cada fallo con `Instant::now()`
(`rate_limiter.rs:103-109`) — el instante de **procesarlo**, no el de la
petición. `ReportPayload` no transporta timestamp (`report.rs:11-24`).

Consecuencia: al arrancar, cientos de 404/401/403 históricos entran al rate
limiter como si acabaran de ocurrir. Un IP con historial de escaneo se lleva un
baneo inmediato con los perfiles `Path Scanning` (20 fallos / 60 s) o
`Global Shield` (300 / 60 s), sin haber hecho nada en ese momento. El plugin Go
que este reader sustituye solo veía tráfico en vivo, así que es una regresión de
comportamiento, no una mejora pendiente.

## What Changes

- El reader deja de alimentar el pipeline Jail con entradas anteriores al
  **inicio del intento de conexión**. El filtro se aplica en cliente, sobre el
  campo `__REALTIME_TIMESTAMP` que ya viene en cada entrada.
- El reader deja de reprocesar la **última entrada ya procesada**: descarta la
  entrada cuyo `__CURSOR` coincide con el último registrado.
- Se documenta que `Range: realtime=` **no es utilizable** con este gatewayd:
  verificado contra systemd 249, se ignora cuando la petición lleva `follow`.
- Se documenta que `Range: entries=<cursor>:1` **no aplica el `skip`** sin
  `num_entries`, que es la razón de que exista el filtro por cursor en cliente.

No cambia la configuración, ni los transportes, ni el formato de las entradas,
ni el comportamiento del pipeline WAF.

## Capabilities

**New Capabilities:**

None.

**Modified Capabilities:**

- `journald-reader` — nueva requirement `No history replay`: nada anterior al
  intento de conexión llega al pipeline, y la reconexión no reprocesa la última
  entrada.

## Impact

- `backend/src/journald_reader.rs`: filtro por `__REALTIME_TIMESTAMP` y dedupe
  por `__CURSOR` antes de `run_jail_pipeline`, con sus tests.
- No hay cambios de esquema, ni de variables de entorno, ni de `compose.yml`.
- Efecto observable para el operador: tras un reinicio, el lector empieza en
  blanco. Lo ocurrido mientras shuul estaba caído no se cuenta — es el mismo
  comportamiento que tenía el plugin y encaja con el fail-open del diseño.
- El coste de arranque no baja: las entradas antiguas se siguen transfiriendo y
  parseando, solo se descartan antes de tocar el rate limiter.

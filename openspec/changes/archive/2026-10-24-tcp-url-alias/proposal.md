# Proposal: tcp-url-alias

## Why

Shuul lee los access logs de Traefik por TCP contra una URL base configurada en
`SHUUL_DOCKER_URL` (API de Docker) o `SHUUL_JOURNALD_URL` (`systemd-journal-gatewayd`).
Hoy esas variables aceptan **sólo** el esquema `http://`: el parseo de la URL vive en
`backend/src/journald_reader.rs` y rechaza cualquier otro esquema antes de conectar.

Docker, en cambio, nombra sus endpoints con `tcp://`: es la sintaxis de `DOCKER_HOST`
(`DOCKER_HOST=tcp://localhost:2375`). Quien configura shuul copiando esa convención
—lo natural, porque el proxy que hay delante *es* un proxy de la API de Docker— se
encuentra con un fallo de configuración que no describe ningún problema real:

```
unsupported scheme tcp in gateway URL tcp://localhost:2375
```

El alias falta, no el transporte: los **dos modos de lectura que se quieren mantener ya
existen** y no se tocan en este change:

| Modo | Variable | Requisitos |
|---|---|---|
| Socket directo del motor | `SHUUL_DOCKER_SOCKET=/var/run/docker.sock` | montar el socket y añadir el grupo `docker` del host a `group_add` |
| TCP contra un proxy | `SHUUL_DOCKER_URL=...` | un proxy (p. ej. `docker-socket-proxy`) en red interna |

Este change añade **la ortografía que falta**, y sólo eso.

## What Changes

Requisito nuevo en el módulo de spec `journald-reader`: los transportes TCP aceptan
`tcp://` como **sinónimo** de `http://`, en las dos variables.

- `tcp://host:puerto` y `http://host:puerto` SHALL producir el **mismo** transporte, con
  la misma autoridad y la misma petición.
- `https://` SHALL conservar su rechazo explícito —que lo nombra—, porque los
  transportes TCP **no implementan TLS**.
- Cualquier otro esquema SHALL rechazarse nombrando el esquema recibido, y una URL sin
  esquema SHALL rechazarse nombrando la variable. El rechazo es recuperable: warning y
  reintento tras `retry_delay`, nunca la caída del proceso.
- `endpoint_display()` SHALL mostrar el **esquema efectivo** y no el escrito: una URL
  configurada como `tcp://host:2375` SHALL aparecer en los logs como
  `http://host:2375` —o `docker:http://host:2375 ({contenedor})` en el backend de
  Docker—, coherente con la regla ya vigente de mostrar el endpoint efectivo y no la
  URL configurada.
- Documentación: se actualizan las tablas de variables y los modos de
  `SELF-HOSTING.md` y el apartado de variables de `AGENTS.md`.

## Non-goals

- **NO se añade TLS ni `https://`.** Se evaluó —la idea inicial era cifrar el
  transporte TCP— y se descartó: el proxy de referencia
  (`tecnativa/docker-socket-proxy`) no incluye TLS «by design» y su PR de TLS/mTLS
  sigue sin fusionar, así que el cifrado exigiría un terminador TLS delante
  (Traefik, Caddy, HAProxy). Si se retoma, va como change aparte, con su propio
  análisis.
- **NO se lee la variable `DOCKER_HOST` de Docker.** Se sigue usando
  `SHUUL_DOCKER_URL`. Honrar `DOCKER_HOST` introduciría una fuente de configuración
  implícita que otro *tooling* dentro del contenedor puede traer puesta sin que nadie
  la haya declarado para shuul. Lo que falta es el esquema, no la variable.
- **NO se migra de proxy.** El cambio de `tecnativa/docker-socket-proxy` a una
  alternativa con permisos mínimos (`CONTAINERS=1` concede hoy lectura del filesystem
  de todos los contenedores, y shuul sólo necesita leer los logs de uno) queda fuera.
- No se cambia la precedencia de las cinco variables de transporte, ni el
  comportamiento de streaming, reconexión, shutdown o fail-open.
- No se toca el WAF, ni el pipeline Jail, ni la base de datos, ni el frontend.

## Impact

- `backend/src/journald_reader.rs` — parseo de la URL base (`tcp_endpoint`) y
  `endpoint_display()`
- Tests de `journald_reader.rs`
- `SELF-HOSTING.md` (tabla de variables y modos del backend de Docker) y `AGENTS.md`
- Sin cambios de esquema de base de datos, ni de API, ni de migraciones, ni de frontend

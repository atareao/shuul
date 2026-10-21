# Proposal: docker-logs-transport

## Why

El pipeline Jail lee hoy los access logs de Traefik de
`systemd-journal-gatewayd`. Eso arrastra una cadena de acoplamientos que sólo
existe para servir el journal por HTTP: habilitar el servicio, un drop-in de
socket con `SocketMode`, montar el **directorio** `/run/systemd/journal` en el
contenedor, un `group_add` con el gid de `systemd-journal` —que varía por host y
que en Podman hay que dar como número porque el nombre no se resuelve contra la
imagen— y, en la versión anterior, un cliente `reqwest` incapaz de hablar
`AF_UNIX`.

La API de Docker sirve los mismos logs y ya está en la máquina: no hay que
habilitar nada en systemd ni depender de dónde haya quedado el socket del
journal. Lo único que hace falta es un cliente HTTP que sepa hablar por un
socket `AF_UNIX` o por TCP, y ese cliente ya existe en
`backend/src/journald_reader.rs`.

> ⚠️ **Lo que se gana y lo que no.** Con el driver de logging en `journald`
> —que es el que usa `co1` hoy, verificado en el plan §0.2— la API de Docker
> sirve los logs, pero **el journal sigue estando debajo**: el daemon corre como
> root y lee el journal para responder. Lo que desaparece es
> `systemd-journal-gatewayd`, el montaje de `/run/systemd/journal` y el
> `group_add` de `systemd-journal`; **no** es independizarse del journal. Si
> algún día el driver de Traefik pasa a `json-file`, entonces sí desaparece
> journald del camino.

Montar el socket de Docker es una **reversión deliberada** de una decisión
anterior de este proyecto, que lo había rechazado por ser equivalente a root en
el host. Se acepta el riesgo sólo en la variante de menor superficie —el proxy—
y se documenta la otra para quien prefiera prescindir del contenedor extra.

## What Changes

- Dos entradas nuevas hacia la **API de logs de Docker**, detrás de la misma
  abstracción de transporte que ya usa el lector:
  - **socket directo** `AF_UNIX` contra `/var/run/docker.sock`
    (`SHUUL_DOCKER_SOCKET`);
  - **proxy** por TCP (`SHUUL_DOCKER_URL`).

  No son dos implementaciones: el cliente `hyper` sobre `AsyncRead + AsyncWrite`
  que ya sirve para gatewayd sirve para las dos sin tocarse.

- **Precedencia de transporte** sobre cinco niveles:
  `SHUUL_DOCKER_SOCKET` → `SHUUL_DOCKER_URL` → `SHUUL_JOURNALD_SOCKET` →
  `SHUUL_JOURNALD_URL` → TCP por defecto. Un valor vacío o en blanco cuenta como
  no definido. Dentro de cada backend, el socket gana a su URL y se emite un
  `warn!` nombrando la variable ignorada, igual que hace hoy el lector.

- **Petición** contra la API de Docker:
  `GET /v1.44/containers/{container}/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since=<epoch>`.
  **Nunca** se envía la cabecera `Upgrade`: si se pide, el motor contesta
  `101 UPGRADED` en vez de servir el stream.

- **No reproducir histórico**, que es lo que banearía IPs con escaneos viejos,
  con el mismo reparto de responsabilidades que ya usa el backend de journald:
  **el corte se fija en cada intento de conexión**, en el instante en que el
  intento empieza, y sólo se entrega al pipeline lo estrictamente posterior a él.
  `since` es el **filtro grueso del servidor** —granularidad de segundo— y una
  mera **pista**, igual que el `Range: entries=<cursor>` de gatewayd; la
  corrección la da la comparación exacta con el instante que precede a cada carga,
  que es lo que pone `timestamps=1`. La consecuencia se escribe sin adornos: lo
  que ocurra mientras el reader está caído, o entre dos intentos, **no se cuenta**
  en el rate limiter —fail-open deliberado, idéntico al que journald ya tiene hoy;
  el WAF sigue protegiendo porque es síncrono y no depende de este transporte—.
  Un dedupe por contenido se descartó: dos peticiones reales pueden compartir IP,
  path, status y segundo.

- **Parser incremental de los frames** de la API de Docker: cabecera de 8 bytes
  (stream, tres bytes de relleno y longitud big-endian) seguida de la carga. Un
  frame puede llegar partido entre dos chunks y un chunk puede traer varios
  frames; el parser mantiene el estado entre chunks. **No** se parte el cuerpo
  por líneas antes de desmultiplexar.

- **Un contenedor con TTY queda fuera del contrato.** El lector asume un
  contenedor sin TTY: con `tty: true` el stream va crudo y sin cabeceras, y el
  decodificador interpretaría bytes del JSON como si fueran una cabecera, dejando
  al lector ciego sin un solo error. Para que sea un diagnóstico y no una avería
  muda, un identificador de stream fuera de `0`, `1` o `2` (un JSON crudo empieza
  por `{`) descarta la conexión con un warning que nombra la causa probable. El
  `compose.yaml` de despliegue no activa `tty`.

- **El parseo de la línea no cambia**: cada carga sigue siendo una línea JSON
  del access log de Traefik y desemboca en la misma
  `run_jail_pipeline(app_state, payload)` que alimenta `POST /api/v1/report`.
  No se duplica la lógica del pipeline.

- **El pipeline WAF no se toca**: sigue siendo forwardAuth síncrono y no
  depende de este transporte.

- **Se retira del despliegue** (no del código, en este cambio): el gatewayd, el
  volumen `/run/systemd/journal`, el `group_add` de `systemd-journal` y el
  puerto `19531`. Las variables `SHUUL_JOURNALD_*` **siguen funcionando** con la
  misma semántica, así que no hay **BREAKING changes** y el rollback es
  restaurarlas.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `journald-reader` — dos requirements entran en el delta:
  - *Journald reader transport* pasa a describir los dos backends (la API de
    Docker y `systemd-journal-gatewayd`), la precedencia completa entre las
    cuatro variables de entorno y la petición de logs de Docker;
  - *No history replay* pasa a cubrir los dos backends: journald por el corte
    con `__REALTIME_TIMESTAMP` más el dedupe por `__CURSOR`, y Docker por el
    filtro grueso `since` más la comparación exacta con el corte del intento de
    conexión en curso. En ambos casos el corte se fija en **cada intento**, la
    corrección la da la comparación en cliente y el filtro del servidor es sólo
    una pista.

  Se añade además el requirement *Docker log stream frame decoding* para el
  desmultiplexado del stream.

## Impact

- `backend/src/journald_reader.rs`: variantes de transporte para la API de
  Docker, resolución de la precedencia, target y cabeceras de la petición de
  logs, el decodificador incremental de frames por delante del parseo NDJSON
  actual, y `from_values` pasa de tres a **cinco** parámetros (los cuatro de
  transporte en el orden de la precedencia) con la migración de sus ~15 llamadas
  de `#[cfg(test)]`. El camino de Docker **no** pasa por `extract_report_payload`
  ni por `is_traefik_entry` —exigen el campo `CONTAINER_NAME`, que las cargas del
  stream no traen— sino directo a `parse_traefik_access_log`, con el filtrado por
  contenedor en el segmento del path. **No cambian** el mapeo de access logs de
  Traefik, el dedupe por cursor del backend de journald, el corte por
  `__REALTIME_TIMESTAMP` ni `run_jail_pipeline`.
- `openspec/changes/docker-logs-transport/design.md`: las decisiones que no se
  leen desde las specs (contenedor, variantes aditivas, `stderr`, reparto
  `since`/`timestamps`, TTY y asimetrías aceptadas).
- `backend/src/main.rs`: sin cambios — el reader se sigue construyendo con
  `JournaldReader::from_env()`.
- `backend/Cargo.toml`: sin dependencias nuevas; `hyper`, `hyper-util` y
  `http-body-util` ya están declaradas.
- Variables de entorno nuevas: `SHUUL_DOCKER_SOCKET`, `SHUUL_DOCKER_URL` y
  `SHUUL_DOCKER_CONTAINER`. Las `SHUUL_JOURNALD_*` se mantienen.
- Configuración de despliegue — dos modos, intercambiables sin recompilar:
  - **Modo A, socket directo**: `SHUUL_DOCKER_SOCKET` más el montaje
    `-v /var/run/docker.sock:/var/run/docker.sock:ro` y un `group_add` con el
    gid del grupo `docker` del host (en `co1`, `999`; **no hardcodear**).
    `:ro` **no** limita la API: el socket es un canal de comunicación, no un
    fichero. El contenedor puede hablar con **todo** el Engine.
  - **Modo B, proxy (recomendado)**: `SHUUL_DOCKER_URL` apuntando a un
    `docker-socket-proxy` con `CONTAINERS=1` y el resto de permisos a `0`, en una
    red interna. No exige ningún permiso del host y sólo deja pasar lo permitido.
- Documentación: `SELF-HOSTING.md` §2 (incluida la advertencia de que un
  `tty: true` deja el lector ciego) y `AGENTS.md` §V.
- Tests: servidores `hyper` de prueba sobre un socket Unix temporal **y** sobre
  TCP, emitiendo los mismos frames multiplexados construidos a mano —con un
  frame partido entre dos chunks, un chunk con dos frames y cargas con el prefijo
  de instante—, más los casos de no-replay (corte por intento, frontera del corte,
  dos líneas idénticas con instantes distintos) y detección de TTY. Los escenarios
  existentes del lector
  deben seguir en verde.
- Quedan intactos: pipeline WAF, frontend, migraciones SQLite y
  `run_jail_pipeline`.

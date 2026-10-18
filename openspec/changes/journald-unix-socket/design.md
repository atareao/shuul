# Design

## Context

Ver `proposal.md` — Why. El estado actual relevante:

- `JournaldReader` guarda `gateway_url: String` y usa `reqwest::Client` para
  `GET {gateway_url}/entries?follow&CONTAINER_NAME=…` con `Accept: application/json`.
- El cuerpo se consume con `response.chunk()` y se alimenta a
  `drain_ndjson_lines()`, que mantiene las líneas parciales en un `Vec<u8>`.
- El bucle exterior reconecta, guarda `__CURSOR` y duerme `retry_delay` entre
  intentos, interrumpible por el canal `watch`.
- `reqwest` no soporta `AF_UNIX` y no permite inyectar un conector propio sin
  abandonar su API pública.

## Goals / Non-Goals

**Goals:**

- Añadir el transporte socket Unix sin duplicar la lógica de petición,
  respuesta, parseo, cursor y reconexión.
- Mantener `SHUUL_JOURNALD_URL` funcionando y los defaults sin cambios.
- Que el fallo de conexión siga siendo fail-open: warning + reintento, nunca
  caída del proceso ni del WAF.

**Non-Goals:**

- TLS. gatewayd se consulta en local, sin `--cert`; si algún día hace falta,
  sería un tercer transporte.
- Invocar `journalctl` o leer `/run/log/journal` directamente.
- Cambiar `ReportPayload`, `run_jail_pipeline`, el pipeline WAF o la BD.
- Soportar sockets `AF_UNIX` abstractos (`@nombre`): sólo paths del sistema de
  ficheros.

## Decisions

### D1. `hyper` para ambos transportes, en lugar de `reqwest` + algo para Unix

`hyper::client::conn::http1::handshake` es genérico sobre cualquier
`AsyncRead + AsyncWrite`, así que el mismo código sirve para `TcpStream` y para
`UnixStream`. Con `reqwest` habría que mantener dos rutas (una con reqwest para
TCP y otra con hyper para Unix) y duplicar cabeceras, cursor y manejo de status.

Alternativas descartadas:

- **`reqwest` + `hyperlocal`**: `hyperlocal` no está pensado para `hyper` 1.x y
  añade una dependencia poco mantenida.
- **Sidecar `socat` que exponga TCP→Unix**: funciona sin tocar Rust, pero mete
  un contenedor de fontanería y sigue habiendo un puerto TCP dentro de la red.
- **Loopback interno**: abrir un listener TCP en el propio proceso que haga de
  puente al socket Unix. Absurdo cuando `hyper` ya resuelve el problema.

Coste: refactorizar la ruta TCP existente. Mitigado por los tests de `run()`
que ya cubren streaming, cursor, shutdown y rechazo de cursor.

### D2. URI en forma origen (`/entries?…`) + cabecera `Host`

La misma petición vale para los dos transportes y evita que `entries_url()`
tenga que devolver algo con sentido distinto en cada uno. `hyper` emite
correctamente la forma origen cuando la URI es sólo path+query.

`entries_url()` se sustituye por `request_target()` (path+query, idéntico en
ambos) y `endpoint_display()` (sólo para logs: `http://host:19531` o
`unix:/run/…/gatewayd.sock`).

### D3. Precedencia de configuración: socket > URL > default

Evita un estado ambiguo sin romper despliegues existentes. Si las dos están
definidas se usa el socket y se avisa una vez. Alternativa descartada: fallar
al arrancar con configuración ambigua — demasiado agresivo para un componente
fail-open.

### D4. `CONNECT_TIMEOUT` de 10 s

Sin timeout, un handshake que se cuelga deja el reader bloqueado
indefinidamente sin más señal que la ausencia de logs. 10 s es holgadamente
mayor que un handshake local y mucho menor que el `retry_delay`.

### D5. Montar el directorio, no el fichero del socket

Un bind mount de fichero queda atado al inodo: si `systemd` recrea el socket
(reinicio del servicio o del host), el contenedor sigue viendo el inodo muerto.
Montar `/run/systemd/journal` propaga la recreación. Es el mismo gotcha que
sufre `docker.sock`.

### D6. Dependencias

`hyper` (`client`, `http1`), `hyper-util` (`tokio`, por `TokioIo`) y
`http-body-util` (por `BodyExt::frame`). Todas están ya en el árbol vía `axum`,
así que el coste de compilación es nulo. `bytes` no se declara: el módulo usa
`hyper::body::Bytes`, que es el re-export de `bytes::Bytes`.

## Risks / Trade-offs

- **Regresión en el transporte TCP al cambiar de cliente** → mitigado por los
  escenarios existentes de `run()` (streaming, shutdown, cursor, rechazo de
  cursor) y por una verificación manual contra un gatewayd real antes de
  fusionar.
- **El socket no existe en el host** (paquete sin instalar o drop-in mal
  aplicado) → el reader entra en bucle de warning cada 30 s. Mitigación: el
  warning incluye el path completo, y `SELF-HOSTING.md` documenta cómo
  verificarlo con `ls -l`.
- **Permisos del socket** (`SocketMode=0660` root:root) → el contenedor corre
  como root, así que no aplica; queda documentado por si alguien añade `user:`.
- **`hyper` sin `Connection: close`** → la `Connection` se queda viva tras el
  cierre del stream; por eso se hace `tokio::spawn` y se descarta al salir del
  bucle, igual que hacía el cliente de `reqwest`.

## Migration Plan

1. Host: drop-in de `systemd-journal-gatewayd.socket` con
   `ListenStream=/run/systemd-journal-gatewayd.sock` y `SocketMode=0660`;
   `daemon-reload` + `restart`; verificar con
   `ls -l /run/systemd-journal/gatewayd.sock` y
   `curl --unix-socket … http://localhost/machine`.
2. Stack: añadir el volumen `/run/systemd/journal:/run/systemd/journal:ro`,
   poner `SHUUL_JOURNALD_SOCKET` y quitar `extra_hosts`.
3. Recrear el contenedor y comprobar en logs
   `journald reader: streaming access logs from unix:/run/…`.
4. Rollback: quitar el volumen, restaurar `SHUUL_JOURNALD_URL` y
   `extra_hosts`, recrear. El WAF no se ve afectado en ningún momento.

## Open Questions

None.

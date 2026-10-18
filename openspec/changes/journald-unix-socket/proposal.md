# Proposal: journald-unix-socket

## Why

El reader del pipeline Jail sólo puede alcanzar `systemd-journal-gatewayd` por TCP
(`SHUUL_JOURNALD_URL`), lo que obliga a exponer el puerto 19531 del host, abrir
reglas de firewall y depender del alias `host.docker.internal`, que no existe
fuera de Docker y resuelve a direcciones distintas según el driver de red. Ese
es exactamente el punto donde falló el primer despliegue.

`systemd-journal-gatewayd` también puede escuchar en un socket Unix `AF_UNIX`
(es el modo que usa Home Assistant OS: `ListenStream=/run/systemd-journal-gatewayd.sock`).
Montar ese socket en el contenedor elimina el puerto, las reglas de firewall y
la dependencia del alias DNS, y sobrevive a los reinicios del daemon de
contenedores porque es un fichero del sistema de ficheros del host.

La limitación real no es Docker: el reader usa `reqwest` con una URL `http://` y
`reqwest` no soporta `AF_UNIX`. Sin cambiar el cliente HTTP, montar el socket no
sirve de nada.

## What Changes

- Nuevo transporte **socket Unix** en el reader, seleccionable con
  `SHUUL_JOURNALD_SOCKET=/run/systemd/journal/gatewayd.sock`.
- El transporte **TCP** (`SHUUL_JOURNALD_URL`) se conserva por compatibilidad y
  sigue siendo el default (`http://localhost:19531`).
- El reader deja de usar `reqwest` y pasa a `hyper` directamente, para tener
  **una sola** ruta de construcción de petición y lectura de respuesta sobre
  ambos transportes.
- Cuatro dependencias directas nuevas: `hyper`, `hyper-util`, `http-body-util`
  y `bytes`. Las cuatro ya están en el árbol de dependencias vía `axum`, así que
  no añaden tiempo de compilación.
- `compose.yml` pasa a montar `/run/systemd/journal` en solo lectura y a usar el
  socket; se eliminan `extra_hosts` y `SHUUL_JOURNALD_URL`.
- `SELF-HOSTING.md` y `AGENTS.md` documentan el modo socket como recomendado y
  dejan el modo TCP como alternativa para gatewayd remoto.

No hay **BREAKING changes**: `SHUUL_JOURNALD_URL` sigue funcionando con la misma
semántica y los valores por defecto del reader no cambian.

## Capabilities

**New Capabilities:**

None.

**Modified Capabilities:**

- `journald-reader` — el requirement *Journald reader transport* pasa a describir
  los dos transportes (TCP y socket Unix), la precedencia de configuración entre
  `SHUUL_JOURNALD_SOCKET` y `SHUUL_JOURNALD_URL`, y el comportamiento fail-open
  cuando el socket no existe.

## Impact

- `backend/src/journald_reader.rs`: nuevo enum de transporte, conexión por
  `UnixStream` o `TcpStream`, petición HTTP/1.1 vía `hyper` y lectura del cuerpo
  por frames. **No cambian** el parseo NDJSON, el filtrado por contenedor, el
  mapeo de access logs de Traefik, la reanudación por `__CURSOR` ni el
  `run_jail_pipeline`.
- `backend/Cargo.toml`: cuatro dependencias directas nuevas.
- `compose.yml`: volumen de solo lectura del socket, nuevas variables de
  entorno, sin `extra_hosts`.
- `SELF-HOSTING.md` §2 y `AGENTS.md`.
- Tests: nuevo gateway falso sobre `UnixListener` en `journald_reader.rs`. Los
  escenarios TCP existentes deben seguir en verde tras el cambio de cliente.
- Quedan intactos: pipeline WAF, frontend, migraciones SQLite y
  `run_jail_pipeline`.

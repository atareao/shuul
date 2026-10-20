# Tasks: journald-socket-permissions

> **Auditoría (rust-reviewer):** APTO.

## 1. Documentación del permiso del socket

- [x] 1.1 `SELF-HOSTING.md` §2.3: drop-in con `SocketGroup=systemd-journal` y `SocketMode=0660`, explicando que el contenedor corre como `app` (uid 1000) y no como root. Verificación: el bloque de código incluye `SocketGroup` y el texto ya no afirma que el contenedor sea root.
- [x] 1.2 `SELF-HOSTING.md` §2.6: añadir `group_add: [systemd-journal]` al snippet del compose y explicar que es el permiso que abre el socket. Verificación: el snippet lo incluye y menciona que hay que recrear el contenedor.
- [x] 1.3 `SELF-HOSTING.md`: corregir las dos afirmaciones de que el contenedor corre como root (§2.3 y §2.10) y la de §3 sobre el owner `root:root`. Verificación: `grep -n "runs as root" SELF-HOSTING.md` no devuelve nada.
- [x] 1.4 `SELF-HOSTING.md` §2.10: reescribir la entrada de `Permission denied` con la causa real (`USER app`), el arreglo (`SocketGroup` + `group_add`) y cómo comprobar el gid con `docker exec shuul id`; incluir el fallback a gid numérico si el grupo no existe. Verificación: la entrada nombra `group_add` y `systemd-journal`.
- [x] 1.5 `compose.yml`: añadir `group_add: [systemd-journal]` al servicio `shuul`. Verificación: `docker-compose config` sale con código 0 y muestra el grupo.
- [x] 1.6 `AGENTS.md`: una línea en la descripción del despliegue indicando que el contenedor necesita el grupo suplementario `systemd-journal`. Verificación: `grep -n systemd-journal AGENTS.md`.

## 2. Diagnóstico accionable del path en la variable equivocada

- [x] 2.1 RED: test de `TcpEndpoint::parse("/run/systemd/journal/gatewayd.sock")` que exija el valor en el mensaje y la mención a `SHUUL_JOURNALD_SOCKET`. Verificación: el test falla.
- [x] 2.2 GREEN: añadir la pista al mensaje cuando el valor no tenga esquema y empiece por `/`. Verificación: el test pasa.
- [x] 2.3 RED/GREEN: test de que `localhost:19531` sigue dando "has no scheme" **sin** la pista. Verificación: el test pasa y no menciona `SHUUL_JOURNALD_SOCKET`.
- [x] 2.4 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.

## 3. Verificación

- [x] 3.1 `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.
- [x] 3.2 Comprobar que el procedimiento documentado es coherente con `compose.yml`: mismo nombre de grupo en drop-in, `group_add` y troubleshooting. Verificación: `grep -rn "systemd-journal" SELF-HOSTING.md compose.yml AGENTS.md` muestra los tres ficheros.

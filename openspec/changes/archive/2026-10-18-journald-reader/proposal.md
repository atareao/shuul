# Journald Reader: replace Go reporter plugin with direct journald socket streaming

## Intent

Reemplazar el plugin Go `traefik-shuul-reporter` por un lector directo del socket Unix de systemd-journald. En lugar de que Traefik haga una petición HTTP `POST /api/v1/report` a través de un plugin, Shuul se conecta directamente al socket `/run/systemd/journal/socket` y recibe en streaming los access logs de Traefik (emitidos en JSON por el LogDriver journald de Docker).

Esto elimina:
- El binario Go `traefik-shuul-reporter` (mantenimiento aparte, cross-compile, timeout 500ms)
- Una llamada HTTP extra por cada request (plugin → POST /api/v1/report)
- El punto de fallo del timeout del plugin
- La escritura continua a disco de logs de acceso (journald es RAM-only via IPC socket)

## Scope

- `backend/src/`: Nuevo módulo `journald_reader.rs` con la lógica de conexión al socket, parseo del formato nativo de journald, filtrado por contenedor Traefik y extracción del JSON de access log.
- `backend/src/main.rs`: Nuevo background task que arranca el reader.
- `backend/src/http/report.rs`: Refactor menor para exponer la lógica del Jail pipeline como función reutilizable (llamable tanto desde HTTP como desde el reader).
- `Dockerfile` / `compose.yml`: Montar `/run/systemd/journal/socket:ro` y `/etc/machine-id:ro`.
- `traefik-shuul-reporter`: Deprecado (se elimina del ecosistema).

NO se modifica:
- El pipeline WAF (sigue siendo forwardAuth síncrono)
- El frontend React
- Las tablas SQLite
- `ReportPayload` (se reusa)

## Impact

- Shuul ahora requiere acceso al socket journald del host (bind mount :ro)
- El WAF sigue siendo forwardAuth; el cambio solo afecta al pipeline Jail
- La lógica de rate limiting, bans, fail_codes, stats NO cambia
- En desarrollo sin Docker, el reader falla silenciosamente si no hay socket

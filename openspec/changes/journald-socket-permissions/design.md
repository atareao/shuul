# Design

## Context

Ver `proposal.md` — Why. Restricciones que condicionan la solución:

- La imagen publica declara `USER app` (`Dockerfile:74`), uid 1000. El proceso no es root.
- `systemd-journal-gatewayd` crea el socket con el owner y modo que dicten
  `SocketUser`, `SocketGroup` y `SocketMode` del `.socket` unit. Por defecto `root:root 0660`.
- El contenedor monta `/run/systemd/journal` en solo lectura y se conecta al socket por
  `AF_UNIX`. La decisión de conectarse por `AF_UNIX` no se revisa en este cambio.

## Goals / Non-Goals

**Goals:**

- Que el procedimiento documentado funcione copiado y pegado en un host limpio.
- Conceder al contenedor el mínimo privilegio necesario: leer el journal, nada más.
- Que el permiso concedido sea visible y auditable desde el propio stack.

**Non-Goals:**

- Cambiar de transporte ni volver a TCP.
- Usar el socket de Docker. Descartado: montar `/var/run/docker.sock` equivale a dar
  root en el host y además reproduce el mismo problema de grupo (`root:docker 0660`),
  así que no reduce la fricción, la agrava.
- `SocketMode=0666`. Descartado: deja el journal legible por cualquier usuario local.
- Cambiar el uid de la imagen.

## Decisions

### D1. `SocketGroup=systemd-journal` en el drop-in y `group_add` en el compose

El socket queda `root:systemd-journal 0660` y el contenedor recibe ese grupo
suplementario. Se reutiliza el grupo que el sistema ya define para "puede leer el
journal", en lugar de inventar una correspondencia con el uid de la imagen.

Alternativas descartadas:

- **`SocketGroup=<uid de app>`**: funciona (verificado), pero codifica un uid de la
  imagen en el host y no expresa la intención. Si el `Dockerfile` cambia de uid, el
  host queda desincronizado en silencio.
- **`SocketMode=0666`**: cualquier usuario local puede leer todo el journal.
- **`SocketUser=app`**: obliga a que el usuario exista en el host, acoplando host e imagen.
- **Socket de Docker**: ver Non-Goals.

Ventaja adicional: el permiso queda declarado en `compose.yml` (`group_add`), junto al
volumen que lo necesita, en lugar de repartido entre host y stack.

### D2. La pista del error se deduce del valor, no de otra variable

Si la URL no tiene esquema y empieza por `/`, es un path absoluto: se añade la pista
nombrando `SHUUL_JOURNALD_SOCKET`. Si no empieza por `/`, no se añade nada, porque
`localhost:19531` sin esquema es un error distinto y la pista sería engañosa.

Se implementa en el formateo del error de `TcpEndpoint::parse`, sin cambiar el tipo ni el
flujo: el fallo sigue siendo `JournalError::Connection` y sigue siendo fail-open.

### D3. La documentación es parte del arreglo, no un extra

El defecto reportado es documental. El cambio se considera incompleto si `SELF-HOSTING.md`
sigue afirmando que el contenedor corre como root, porque esa frase es la que indujo el
error de despliegue.

## Risks / Trade-offs

- **Host sin grupo `systemd-journal`** (journald gestionado sin systemd, contenedores
  mínimos): `SocketGroup=systemd-journal` falla al aplicar el drop-in. Mitigación:
  documentar el fallback con gid numérico en §2.10.
- **`group_add` no se aplica en caliente** → requiere recrear el contenedor. Documentado
  en el Migration Plan.
- **Nombre de grupo distinto según distro** (`systemd-journal` es el estándar, pero un
  host puede haberlo renombrado). Mitigación: el troubleshooting explica cómo comprobar
  el gid real.
- **La pista solo cubre el caso del path absoluto.** Un usuario que pegue una URL
  `unix:///run/...` seguirá viendo "unsupported scheme". Aceptable: el mensaje ya nombra
  el esquema rechazado.

## Migration Plan

1. Actualizar el drop-in del host con `SocketGroup=systemd-journal`; `daemon-reload` y
   `restart` del `.socket`.
2. Añadir `group_add` al servicio en el stack.
3. `docker compose up -d --force-recreate shuul` (obligatorio: el grupo suplementario se
   fija al crear el contenedor).
4. Verificar `docker exec shuul id` (debe listar el gid de `systemd-journal`) y los logs
   del reader.
5. Rollback: quitar `group_add`, `restart` del `.socket` sin el drop-in, recrear. El WAF
   no se ve afectado en ningún momento.

## Open Questions

None.

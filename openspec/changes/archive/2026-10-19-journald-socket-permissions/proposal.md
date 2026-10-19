# Proposal: journald-socket-permissions

## Why

La versión 0.15.0 documenta el drop-in del socket de `systemd-journal-gatewayd` con
`SocketMode=0660` y owner `root:root`, y afirma dos veces que "el contenedor de shuul
corre como root". No es cierto: el `Dockerfile` declara `USER app` (uid 1000). Seguir
`SELF-HOSTING.md` §2.3 al pie de la letra produce
`connect: Permission denied (os error 13)` en cuanto el reader arranca.

Además, cuando el path del socket acaba por error en `SHUUL_JOURNALD_URL` en lugar de
`SHUUL_JOURNALD_SOCKET`, el error es `gateway URL … has no scheme`, que no sugiere la
corrección. Es el error que se produjo en el primer despliegue real.

## What Changes

- `SELF-HOSTING.md` §2.3: el drop-in pasa a usar `SocketGroup=systemd-journal` con
  `SocketMode=0660`, y se explica por qué (el contenedor corre como `app`, no como root).
- `SELF-HOSTING.md` §2.6: el snippet del compose incluye `group_add: [systemd-journal]`.
- Se corrigen las dos afirmaciones falsas de que el contenedor corre como root (§2.3 y
  §2.10) y la de §3 que da por hecho un owner `root:root`.
- `SELF-HOSTING.md` §2.10: la entrada de `Permission denied` describe la causa real y el
  arreglo, en lugar de descartarlo como un caso que "nunca ocurre".
- `compose.yml`: añade `group_add`.
- `AGENTS.md`: una línea con el `group_add` en la descripción del despliegue.
- Nuevo diagnóstico accionable: si `SHUUL_JOURNALD_URL` recibe un path absoluto, el
  mensaje de error menciona `SHUUL_JOURNALD_SOCKET`.

No cambia el comportamiento para un despliegue ya configurado correctamente, ni la
precedencia socket → URL → default.

## Capabilities

**New Capabilities:**

None.

**Modified Capabilities:**

- `journald-reader` — nueva requirement `Configuration diagnostics`: los errores de
  configuración deben nombrar el valor rechazado y, cuando la corrección sea evidente, la
  variable correcta.

## Impact

- `backend/src/journald_reader.rs`: mensaje de error de `TcpEndpoint::parse` y su test.
- `compose.yml`, `SELF-HOSTING.md`, `AGENTS.md`.
- Operación: `group_add` no se aplica en caliente, hay que recrear el contenedor.
- Despliegues que ya hubieran puesto `SocketMode=0666` o dejado `root:root` deben
  actualizar el drop-in; con `0666` sigue funcionando pero es más permisivo.

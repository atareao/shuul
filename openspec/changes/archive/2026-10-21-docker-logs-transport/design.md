# Design

## Context

Ver `proposal.md` — Why. El estado actual relevante:

- `JournaldReader` guarda un `JournaldTransport` (`Tcp { base_url }` o
  `UnixSocket { path }`) y habla HTTP/1.1 contra `systemd-journal-gatewayd` con
  `hyper` sobre `AsyncRead + AsyncWrite` (`journald_reader.rs:169-176`, el trait
  `Io` y el alias `BoxedIo`). El cliente es **genérico a propósito**: la misma
  ruta sirve para un socket Unix y para un TCP.
- `from_values(socket, url, container_name)` resuelve hoy la precedencia de
  journald (`journald_reader.rs:282`), con ~15 llamadas en `#[cfg(test)]`
  (`journald_reader.rs:3299-3572`).
- El bucle de `run()` reconecta, guarda `last_cursor` y duerme `retry_delay`
  entre intentos, interrumpible por el canal `watch`. `stream_once` fija el
  corte con `now_micros()` y alimenta `drain_ndjson_lines` con los frames del
  cuerpo.
- El camino de una entrada es `extract_report_payload(entry, container_name)` →
  `is_traefik_entry()` → `parse_traefik_access_log()` → `run_jail_pipeline()`.
- Medido en `co1` (plan §0.2): driver de logging **`journald`** en Traefik y en
  Shuul, API del motor **1.50**, gid del grupo `docker` **999**, gid de
  `systemd-journal` **101**.

## Goals / Non-Goals

**Goals:**

- Añadir el backend de la API de Docker con sus **dos entradas** (socket directo
  y proxy TCP) **sin duplicar** la lógica de petición, lectura, parseo, dedupe,
  reconexión ni fail-open.
- Mantener `SHUUL_JOURNALD_*` funcionando con la misma semántica y los defaults
  sin cambios.
- Que el no-replay siga siendo una garantía de verdad, no una aproximación: un
  reinicio no puede banear a nadie.
- Que un contenedor mal configurado (TTY) se **diagnostique**, no que deje al
  reader ciego en silencio.

**Non-Goals:**

- TLS contra la API de Docker. Se entra por el socket local o por una red
  interna; `https` no está soportado (igual que hoy en gatewayd).
- Soportar el modo TTY del stream (crudo, sin cabeceras). Queda fuera del
  contrato por decisión, no por olvido.
- Cambiar `ReportPayload`, `run_jail_pipeline`, el pipeline WAF o la BD.
- Retirar el backend de journald en este change.

## Decisions

### D1. Contenedor: `SHUUL_DOCKER_CONTAINER` → `SHUUL_JOURNALD_CONTAINER` → `traefik`

El plan §0.3 introduce `SHUUL_DOCKER_CONTAINER` (default `traefik`) sin decir qué
pasa con la variable de journald que ya existe. Se resuelve con un **fallback**,
no duplicando el nombre en silencio: quien ya tiene `SHUUL_JOURNALD_CONTAINER`
definida y cambia de backend sigue apuntando al mismo contenedor, y quien quiera
distinguirlos puede definir la de Docker. Ambas en blanco equivalen a `traefik`.

### D2. Variantes **aditivas** del enum, sin renombrar las existentes

Se añaden `DockerTcp { base_url }` y `DockerUnixSocket { path }` y se dejan
`Tcp` y `UnixSocket` **intactas**.

Alternativa descartada: renombrar a `JournaldTcp`/`JournaldUnixSocket`. Leería
mejor —`Tcp` al lado de `DockerTcp` es ambiguo a primera vista—, pero obliga a
tocar el texto de los escenarios heredados que ya nombran `UnixSocket` y `Tcp`
(«el transporte es `UnixSocket` con ese path») y las llamadas de test. El
requisito documenta que las dos variantes sin prefijo son las de gatewayd; el
coste de la ambigüedad es menor que el de mover texto heredado en un delta.

`new()` y `unix()` mantienen su firma: siguen construyendo el transporte de
gatewayd y los tests existentes no cambian por ellos.

### D3. `stderr` se pide, pero se descarta

El plan se contradice en este punto: §0.3 fija `stderr=1` en la query y §0.5.3
dice «stderr descartado». Se hacen las dos cosas porque no se excluyen: se pide
para poder diagnosticar (un error de Traefik en stderr aparece en los logs del
reader), y se descarta a nivel `debug` porque el access log va a stdout y una
línea de arranque no es un access log.

Consecuencia asumida: si algún día Traefik escribiera access logs por stderr, se
perderían. Hoy no lo hace.

### D4. `timestamps=1` + corte por intento, y reparto con `since`

**Lo que se propuso y se descartó** (queda el rastro, no sólo el resultado):

1. **Dedupe por contenido** de la última línea. Se descartó porque dejaba pasar
   las líneas del mismo segundo de la reanudación —y por tanto podía banear por
   histórico, que es justo lo que evita el requisito *No history replay*— y porque
   confunde dos peticiones reales con el mismo texto: un escáner que reintenta
   comparte IP, path, status y segundo.
2. **El no-replay como «último instante procesado»** (reanudar desde la última
   carga entregada). Se descartó por dos razones:
   - `RateLimiter::record()` sella cada evento con `Instant::now()`, **no** con el
     instante del log (`journald_reader.rs:506`). Tras una caída de dos horas,
     reanudar desde la última carga entregada reproduciría dos horas de tráfico de
     golpe y el rate limiter las contaría **todas como «ahora»**: un pico
     artificial de miles de eventos contra una ventana de 60 s → **baneos en masa
     de tráfico legítimo**. Es exactamente el motivo por el que existe el corte por
     intento de conexión, y no una cuestión de simetría con el otro backend;
   - la única forma de que fuese correcto —pasar el instante del log al rate
     limiter, para que la ventana se evalúe contra el tiempo real del evento— toca
     `run_jail_pipeline` y `RateLimiter`, y este change declara explícitamente que
     **no los toca** (`proposal.md` §Impact). Queda fuera de alcance: sería otro
     change.

**Diseño final: el corte se fija en CADA intento**, igual que ya hace hoy el
backend de journald —`stream_once` fija `cutoff_micros = now_micros()` antes de
pedir nada—, y sólo se entrega al pipeline lo estrictamente posterior a ese corte.
Es el **mismo reparto de responsabilidades** que journald, y esta vez el
paralelismo es exacto: `Range` es pista y el corte por `__REALTIME_TIMESTAMP` es
corrección; `since` es pista y la comparación exacta con el instante de la carga
es corrección.

| | journald | Docker |
|---|---|---|
| Filtro del servidor (pista) | `Range: entries=<cursor>:1` | `since=<corte del intento, en segundos>` |
| Corrección en cliente | corte por `__REALTIME_TIMESTAMP` + dedupe por `__CURSOR` | comparación exacta con el instante de la carga |
| Granularidad de la pista | entrada del journal | **segundo** |
| Consecuencia del hueco | el skip no se aplica sin `num_entries` | el motor entrega el segundo del corte y la comparación exacta descarta lo que no lo supera |

`timestamps=1` hace que cada carga llegue como
`<instante RFC3339Nano> <línea JSON>`. El lector extrae el instante, descarta el
prefijo —el parseo del JSON no cambia— y sólo entrega al pipeline la carga cuyo
instante sea **estrictamente posterior** al corte del intento en curso. El hueco
del segundo desaparece y no hace falta comparar contenido.

Ni `Range` ni `since` son la corrección: reducen lo que viaja, y es la comparación
en cliente la que garantiza que nada se cuenta dos veces.

**Nota: la frontera no es la misma en los dos backends, y es a propósito.** El
backend de journald **conserva** la entrada cuyo `__REALTIME_TIMESTAMP` **iguala**
el corte (descarta sólo lo estrictamente anterior: `timestamp < cutoff_micros`,
`journald_reader.rs:596`); el de Docker **descarta** la carga que lo iguala, porque
`since` sólo da el
**segundo** y hay que tirar la frontera entera para no dejar pasar lo escrito
dentro de ese segundo. El alcance real del desacuerdo es **una** línea: un access
log cuyo instante cae exactamente en el microsegundo del corte y cero nanosegundos.
No se unifica porque el corte de journald es **heredado** y este change lo declara
intocable (`proposal.md` §Impact): volver estricta su comparación es un cambio de
journald y va en otro change. Dejarlo escrito aquí es lo que evita que alguien lo
«arregle» dentro de tres meses como si fuera un descuido.

### D5. El camino de Docker **no** pasa por `extract_report_payload`

`extract_report_payload(entry, container_name)` empieza por `is_traefik_entry()`,
que **exige el campo `CONTAINER_NAME`** (`journald_reader.rs:1078-1082`). Las
cargas del stream de Docker son sólo el mensaje —la API ya extrajo el envoltorio
del driver de logging—, así que reutilizar esa función tal cual descartaría el
**100 %** de las líneas, en silencio.

El plan lo resumía en «se parsea igual que hoy», que no se sostiene contra el
código. El camino de Docker va **directo a `parse_traefik_access_log`**, y el
filtrado por contenedor lo hace el servidor por el segmento `{container_name}`
del path. El requisito lo dice explícitamente para que nadie lo «arregle»
añadiendo la comprobación de vuelta.

Lo que **sí** se comparte, y es lo que importa: `parse_traefik_access_log` y
`run_jail_pipeline`. No hay pipeline duplicado.

### D6. Un contenedor con TTY queda fuera del contrato, y se detecta

Sin TTY el motor multiplexa con cabecera de 8 bytes; con `tty: true` el stream
es **crudo**, sin cabeceras, y el decodificador interpretaría bytes del JSON como
si fueran una cabecera: «no llega ni una línea», sin un solo error. Es el mismo
tipo de fallo silencioso que la Fase 3 fue a cazar.

Dos cierres:

1. **Condición de despliegue explícita:** el lector asume un contenedor sin TTY,
   el `compose.yaml` no activa `tty` y activarlo queda fuera del contrato.
2. **Detección barata:** el identificador de stream de una cabecera válida es
   `0`, `1` o `2`; un JSON crudo empieza por `{` (`0x7B`). Un valor fuera de ese
   conjunto descarta la conexión con un warning que **nombra la causa probable**
   en lugar de seguir decodificando.

### D7. Asimetrías aceptadas a conciencia

No son descuidos; se aceptan con el motivo delante:

- **El header `### Requirement: Journald reader transport` no se renombra**
  aunque pase a cubrir también la API de Docker. `openspec archive` casa los
  headers de forma literal, y renombrarlo obligaría a renombrar también el de la
  spec destino: la herramienta no lleva bien ese baile.
- **`DOCKER_API_VERSION = "v1.44"` con un motor 1.50** en `co1`. Se fija **por
  debajo** a propósito: el motor negocia hacia abajo, así que la misma imagen
  funciona también contra motores más antiguos que el de `co1`. Un `v1.50`
  habría atado el reader a la versión medida en una sola máquina.
- **`endpoint_display()` usa formatos distintos por backend**:
  `docker:unix:/var/run/docker.sock (traefik)` lleva el contenedor entre
  paréntesis y `unix:/run/systemd/journal/gatewayd.sock` no. Se mantiene la
  asimetría antes que tocar el escenario heredado que ya fija el formato de
  journald. Son dos formatos en la misma línea de log; el contenedor es
  información útil en el caso de Docker, donde el endpoint por sí solo no dice
  qué se está leyendo.

### D8. journald se mantiene como backend de menor prioridad

No se retira en este change (plan §0.7.4). Coste reconocido: dos backends son dos
juegos de tests de integración que mantener. A cambio, el rollback es cambiar
variables de entorno, no volver a una imagen anterior, y `SHUUL_JOURNALD_*` sigue
funcionando con la misma semántica.

### D9. Firma de `from_values` con los cuatro parámetros de transporte

Con dos parámetros de transporte es **imposible** distinguir el backend, así que
la firma pasa a cinco parámetros, en el orden de la precedencia:

```rust
pub fn from_values(
    docker_socket: Option<String>, docker_url: Option<String>,
    journald_socket: Option<String>, journald_url: Option<String>,
    container_name: Option<String>,
) -> Self
```

Es un cambio de firma dentro del mismo crate: las ~15 llamadas de `#[cfg(test)]`
se migran en el GREEN (trabajo mecánico, previsto en `tasks.md` 4.1). El
`from_env()` no cambia por fuera.

## Risks / Trade-offs

- **Montar el socket de Docker equivale a root en el host.** Es la reversión
  deliberada de una decisión anterior del proyecto. Mitigación: el modo B (proxy
  con `CONTAINERS=1`) es el recomendado y no exige ningún permiso del host; el
  modo A se documenta con la advertencia de que `:ro` no limita la API.
- **El driver de logging sigue siendo `journald`**: la API de Docker lo sirve,
  pero el journal sigue debajo porque el daemon lo lee como root. Lo que se gana
  es quitar `systemd-journal-gatewayd`, el volumen y el `group_add`; **no** es
  independizarse del journal. Escrito en el `proposal.md` para no vender humo.
- **Lo que ocurre con el reader caído no se cuenta** (fail-open **deliberado**, el
  mismo que journald ya tiene hoy). Como el corte se fija en **cada** intento, el
  tráfico escrito entre dos intentos —caída, reinicio, reconexión o intento
  fallido— no llega al rate limiter y no puede banear a nadie por él. Se prefiere
  **no contarlo** a banear tráfico legítimo reproduciéndolo de golpe contra una
  ventana de segundos. El WAF sigue protegiendo durante esa ventana porque es
  síncrono y no depende de este transporte. Contarlo exigiría pasar el instante del
  log al rate limiter: **es otro change**.
- **El desfase de reloj con un daemon remoto: riesgo de primera clase, no un
  matiz.** La comparación exacta del backend de Docker enfrenta el instante que
  precede a cada carga —sellado por el **motor** con `timestamps=1`— contra el
  corte fijado por el reader con `now_micros()` en el **host de Shuul**. Con el
  reloj del emisor **por detrás** del de Shuul, las cargas vivas caen por debajo
  del corte y se descartan **en silencio**, sin un solo error: exactamente la clase
  de avería muda que la Fase 3 se dedicó a cazar. Con el reloj **por delante**, el
  `since` —que el motor interpreta en su propio reloj— deja pasar los últimos
  segundos del backlog y la comparación estricta no los para, porque sus instantes
  caen por encima del corte: un pequeño **replay** proporcional al desfase. Las dos
  direcciones son la **misma limitación ya documentada** para el transporte TCP de
  journald (`journald_reader.rs:40-43`: «un desfase NTP entre hosts descarta
  tráfico legítimo si el remoto va atrasado, o deja pasar histórico si va
  adelantado»), no una propiedad nueva que introduzca Docker.
  Se **acepta**, no se tolera, a conciencia: con el daemon en la **misma máquina**
  —el caso de los modos A y B, mismo-host por definición— emisor y lector comparten
  reloj, igual que el `__REALTIME_TIMESTAMP` local de journald hoy;
  `SHUUL_DOCKER_URL` contra un daemon de **otra máquina** queda **fuera del
  contrato** de este change. La alternativa —relajar la comparación aceptando todo
  el segundo del corte o una ventana de holgura— **reabriría el no-replay**, que
  es la garantía que este change existe para extender: la frontera estricta es lo
  que impide contar dos veces la línea del segundo compartido.
- **`since` con driver `journald`**: está medido con `json-file` en la
  documentación de Docker, no en `co1`. Hay que confirmarlo allí antes de dar la
  fase por buena (tarea `8.3`).
- **Un `tty: true` deja el reader ciego**: el warning lo nombra, pero se repetirá
  cada `retry_delay` mientras siga puesto. Es preferible a la avería muda.
- **Regresión en el transporte de gatewayd** al tocar `from_values` y el enum:
  mitigado por los ~15 tests existentes de la precedencia y por los escenarios de
  `run()` (streaming, cursor, rechazo de cursor, corte por instante, shutdown).

## Migration Plan

1. Elegir modo en `co1` (§0.4 del plan): **B (proxy)** por recomendación, A si se
   prefiere prescindir del contenedor extra. El código soporta las dos.
2. Editar el `compose.yaml` de `co1` —vive fuera del repositorio— con el bloque
   del modo elegido, sin poner `tty: true`.
3. Desplegar con `--force-recreate` y comprobar en los logs la línea de arranque
   con el endpoint de Docker.
4. Verificar: entradas leídas con normalidad, **cero** `BANNED` en el primer
   minuto (no se reproduce histórico) y el WAF intacto.
5. Retirar gatewayd, el volumen `/run/systemd/journal`, el `group_add` de
   `systemd-journal` y el puerto `19531`.
6. Rollback: restaurar `SHUUL_JOURNALD_URL` o `SHUUL_JOURNALD_SOCKET` (y, para el
   socket, el volumen y el `group_add`) y recrear. El WAF no se ve afectado en
   ningún momento.

## Open Questions

Ninguna abierta. Las dos que el plan dejaba abiertas en §0.7 quedan cerradas: se
implementan las dos entradas, y journald se mantiene (D8).

**Deuda de nomenclatura, decidida a conciencia** — se deja escrita para que sea
una decisión consciente y no un descuido que alguien herede sin saber por qué:

- El **requisito** `### Requirement: Journald reader transport` ya no describe
  sólo su nombre, y **no se renombra** aquí: `openspec archive` casa los headers
  de forma literal (ver **D7**; no se repite el motivo).
- El **módulo** se llama `journald_reader` (`backend/src/journald_reader.rs`) y la
  **capacidad** `journald-reader`, y en adelante sirven también la **API de
  Docker**. Renombrar módulo, fichero y capacidad es un **cambio aparte**: toca
  el `mod journald_reader` y la ruta del fichero en `main.rs`, los `path` de la
  spec y `AGENTS.md`. Este change **no** lo hace —colarlo aquí mezclaría un
  renombrado con una funcionalidad en el mismo diff y en el mismo archivado—.
- **Criterio:** cuando toque, va con su propio change proposal, su propio delta de
  spec y su propio `openspec archive`; nunca colado en medio de una
  funcionalidad.

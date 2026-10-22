# Tasks: docker-logs-transport

> **Base:** `develop` @ `ec09456` (árbol limpio). Baseline de tests: **284**.
> **Alcance:** sólo el pipeline Jail. El WAF sigue siendo forwardAuth síncrono y
> no se toca.

## 1. Preparación

- [x] 1.1 `just check-spec` en verde con este change activo. Verificación: la receta sale con exit 0 (antes de crear el change salía con exit 1 y el mensaje `No active change proposal found`).
- [x] 1.2 Confirmar que las variables y la precedencia de §0.3 del plan son las que implementan los escenarios de `specs/journald-reader/spec.md`. Verificación: `openspec validate docker-logs-transport --strict` sin errores y `deltaCount` 3 (dos MODIFIED y un ADDED).
- [x] 1.3 Decisión registrada (plan §0.5.2): este change **mantiene** el soporte de `systemd-journal-gatewayd` como backend de menor prioridad; no se retira. Verificación: `design.md` §D8 lo dice y los escenarios de journald siguen en el delta.
- [x] 1.4 `design.md` escrito con las decisiones que no son evidentes desde las specs (contenedor, variantes aditivas, `stderr`, reparto `since`/`timestamps`, TTY, asimetrías aceptadas). Verificación: `openspec status --change docker-logs-transport` marca `4/4 artifacts complete`.

## 2. Decodificación de los frames (RED)

- [x] 2.1 RED: tests unitarios del decodificador incremental con frames construidos a mano: cabecera de 8 bytes (stream, 3 de relleno, longitud big-endian), un frame **partido entre dos chunks**, dos frames en un chunk, un frame con carga vacía y una cabecera truncada al cerrarse el stream. Verificación: `cargo test journald_reader` falla porque el decodificador todavía no existe. **Cerrado**: sus tests están hoy en verde (4.4).
- [x] 2.2 RED: tests del identificador de stream: `1` (stdout) llega al pipeline, `2` (stderr) se descarta. Verificación: el test falla por API inexistente. **Cerrado**: sus tests están hoy en verde (4.4).
- [x] 2.3 RED: test de **detección de TTY**: un primer byte que no sea `0`, `1` ni `2` (stream crudo, un JSON empieza por `{`) descarta la conexión con un warning que nombra la causa probable, en vez de seguir decodificando. Verificación: el test falla; el warning incluye la palabra `TTY` o equivalente explícito. **Cerrado**: sus tests están hoy en verde (4.4).
- [x] 2.4 RED: test de extracción del prefijo de instante: una carga `<instante RFC3339Nano> {…}` se parte en instante + línea, y el `ReportPayload` llega al pipeline con los mismos campos que sin el prefijo. Verificación: el test falla. **Cerrado**: sus tests están hoy en verde (4.5).

## 3. Transporte Docker (RED)

- [x] 3.0 RED: **andamiaje de firma**, sin comportamiento: `from_values` pasa a
      los **cinco** parámetros (Docker socket, Docker URL, journald socket,
      journald URL, contenedor) en el orden de la precedencia, y se migran las
      **~15 llamadas de `#[cfg(test)]`** que hoy pasan tres
      (`journald_reader.rs:3299-3572`) —trabajo mecánico, en el mismo commit que
      el cambio de firma—. Los dos parámetros de Docker **se ignoran**: la
      resolución de journald se queda exactamente como está, de modo que los
      tests heredados siguen en verde y los de Docker caen **en su aserción**.
      Es andamiaje del RED y no comportamiento del GREEN porque un test que no
      compila no deja el crate en rojo: lo tumba entero —con los 284 heredados
      dentro— y se pierde justo la comprobación que exige el protocolo.
      Verificación: `cargo test` compila y los heredados de precedencia de
      journald siguen pasando. **Cerrado**: sus tests están hoy en verde (4.1/4.2).
- [x] 3.1 RED: test de precedencia con las **cuatro** variables de transporte y los cinco niveles, pasando los cinco argumentos a `from_values` en el orden de la precedencia: sólo `SHUUL_DOCKER_SOCKET`; sólo `SHUUL_DOCKER_URL`; socket y URL de Docker a la vez; las dos de Docker y las dos de journald; y el caso en blanco. Verificación: el test falla —y **no compila** contra la firma actual de tres parámetros, que es justamente la señal de que el contrato hay que cambiarlo. **Cerrado**: sus tests están hoy en verde (4.1/4.2).
- [x] 3.2 RED: test del target y de las cabeceras de la petición de Docker: `GET /v1.44/containers/{container}/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={epoch}`, `Accept: application/json`, `Host: docker` por socket y la autoridad del endpoint por TCP, y **ausencia** de `Upgrade`. Verificación: el test falla por API inexistente. **Cerrado**: sus tests están hoy en verde (4.3).
- [x] 3.3 RED: test de `endpoint_display()` para las dos entradas de Docker (`docker:unix:… (traefik)` y `docker:http://…:2375 (traefik)`). Verificación: el test falla. **Cerrado**: sus tests están hoy en verde (4.3).
- [x] 3.4 RED: servidor `hyper` de prueba sobre un socket Unix temporal **y** sobre TCP que responda con `200 OK` y `Content-Type: application/vnd.docker.raw-stream`, emitiendo en **ambos** los mismos frames multiplexados construidos a mano —incluido a propósito un frame partido entre dos chunks, un chunk con dos frames y una carga con el prefijo de instante— y exigiendo que las líneas de stdout lleguen al pipeline Jail. Verificación: el test falla; los tests existentes del transporte de gatewayd siguen en verde. **Cerrado**: sus tests están hoy en verde (4.4/4.5).
- [x] 3.5 RED: test de no-replay del backend de Docker: (a) el histórico anterior al corte no llega al pipeline; (b) tras reconectar, el `since` de la petición lleva el corte del **intento nuevo** —comprobado **como forma de la petición**, igual que en 3.2, y **no** como garantía de corrección: si un día se olvidara ponerlo, el lector seguiría siendo correcto porque es la comparación estricta la que descarta, sólo viajaría de más— y **ninguna** carga del segundo de ese corte se cuenta dos veces (esa no-duplicación la da la comparación estricta, no el `since`); (c) dos cargas de **contenido idéntico** con instantes distintos —los dos posteriores al corte— se cuentan las dos; (d) una carga con el instante **igual** al corte del intento se descarta; (e) una carga sin prefijo o con prefijo no parseable se descarta sin abortar el stream; (f) un intento fallido desplaza el corte, de modo que lo ocurrido durante la caída no llega al pipeline. Verificación: el test falla. **Cerrado**: sus tests están hoy en verde (4.6).
- [x] 3.6 RED: test de fail-open con el backend de Docker: socket ausente y proxy inalcanzable → warning `docker api not available`, reintento tras `retry_delay` y `run()` que devuelve `Ok(())` al recibir la señal de parada, sin dejar tareas colgadas. Verificación: el test falla. **Cerrado**: sus tests están hoy en verde (4.7).
      Nota (honestidad del registro): el tercer `GIVEN` del escenario *Docker
      API not available* —«o el contenedor no existe»— **no** se cubrió en el
      RED de 3.6. Quedó como hueco declarado y lo cierra 4.9, que añade su test
      en rojo junto a los dos hermanos de aquí.
- [x] 3.7 RED: test de shutdown a mitad de stream con el backend de Docker. Verificación: el test termina en menos de un segundo y devuelve `Ok(())`. **Cerrado**: sus tests están hoy en verde (4.7).
- [x] 3.8 RED (**cubierto** en el RED de 3.4–3.5): los dos
      comportamientos de `from_env()` que se quedan sin test en rojo. (a) La
      precedencia del contenedor de la decisión **D1** del `design.md`
      (`SHUUL_DOCKER_CONTAINER` → `SHUUL_JOURNALD_CONTAINER` → `traefik`), con la
      manipulación de entorno y `#[serial]` que ya usa el módulo. (b) El `warn!`
      que nombra la **URL ignorada** cuando el socket de Docker gana
      (`ignoring SHUUL_DOCKER_URL=…`), que la tarea 4.1 da por hecho sin que
      ningún test lo exija: el test de 3.1 con las dos variables de Docker a la
      vez sólo asienta el transporte. Verificación: los dos tests fallan en rojo
      y 4.1/4.2 los ponen en verde.
      Nota: el test del **fallback** de contenedor (`SHUUL_DOCKER_CONTAINER`
      ausente → `SHUUL_JOURNALD_CONTAINER`) queda **verde por construcción**
      mientras los parámetros de Docker se ignoren —`from_env()` lee la variable
      de journald como siempre—: es guardián de 4.2, no RED.

## 4. Implementación (GREEN)

- [x] 4.1 GREEN: resuelta en `from_values`/`from_env()` la precedencia de **cinco niveles** (socket de Docker → URL de Docker → socket de journald → URL de journald → `http://localhost:19531`) y el `warn!` que nombra `SHUUL_DOCKER_URL` cuando el socket de Docker gana. Verificación cumplida: los 5 tests de 3.1 pasan y no queda ninguna llamada a `from_values` con menos de cinco argumentos.
- [x] 4.2 GREEN: `DockerTcp` y `DockerUnixSocket` en `JournaldTransport`, construidas por `from_values`; y la decisión **D1** del contenedor en `from_env()` (`SHUUL_DOCKER_CONTAINER` → `SHUUL_JOURNALD_CONTAINER` → `traefik`). Verificación cumplida: los 5 tests de 3.1 y los 2 de 3.8 pasan, los guardianes siguen verdes, y los dos `#[allow(dead_code)]` de las variantes quedan retirados.
- [x] 4.3 GREEN: target, cabeceras y `endpoint_display()` del backend de Docker. `docker_request_target()` es el constructor **puro** del target —lo usan `build_request` y, a través de él, los tests—, y `host_header()` y `endpoint_display()` cubren los **cuatro** transportes en un solo `match` cada uno, así que no hay dos sitios que puedan desviarse. `connect()` es el mismo para los dos backends (`connect_unix`/`connect_tcp`). Verificación: los tests de 3.2 y 3.3 pasan; el de 3.4 **no** puede pasar hasta 4.4, que es quien entrega las líneas decodificadas al pipeline.
- [x] 4.4 GREEN: implementar el decodificador incremental de frames por delante de `drain_ndjson_lines`, respetando el corte por frontera de chunk, el descarte de `stderr` y la detección de TTY. Verificación cumplida: los tests de 2.1, 2.2, 2.3 y 3.4 pasan.
- [x] 4.5 GREEN: extraer el prefijo de instante de cada carga y entregar el resto a `parse_traefik_access_log`, sin pasar por `extract_report_payload`/`is_traefik_entry` en el camino de Docker (exigirían `CONTAINER_NAME` y descartarían el 100 % de las líneas). Verificación cumplida: los tests de 2.4 y 3.4 pasan y el pipeline recibe los payloads con sus campos.
- [x] 4.6 GREEN: corte por intento —**estrictamente posterior** al corte del intento en curso— y `since` como pista gruesa: el corte del intento en **cada** intento, también tras reconectar y tras un intento fallido; **nunca** el instante de la última carga procesada. **No** comparar contenido de línea. Verificación cumplida: los tests de 3.5 y los escenarios de *No history replay* pasan.
- [x] 4.7 GREEN: fail-open y shutdown del backend de Docker reutilizando el bucle de `run()` y `wait_for_shutdown`, sin duplicar el bucle. Verificación cumplida: los tests de 3.6 y 3.7 pasan, junto con los de gatewayd.
- [x] 4.8 GREEN: si `SHUUL_DOCKER_URL` trae un **path base**, éste se ignora al
      construir la petición —que va en **forma origen**, igual que en el socket— y
      NO SHALL aparecer en `endpoint_display()`, que muestra el endpoint
      **efectivo** `docker:scheme://host:puerto (contenedor)`, con un `warn!` que
      nombra el path ignorado. Es el mismo contrato que ya cumple
      `SHUUL_JOURNALD_URL`: el brazo `DockerTcp` de `endpoint_display()`
      **reutiliza** `tcp_endpoint` con el mismo `map_or_else` que su hermano de
      journald, y el aviso comparte `warn_ignored_base_path` —generalizada a
      `(url, variable, target)`— sin divergir entre los dos brazos.
      Verificación cumplida: el escenario *A base path in the Docker URL is
      ignored* pasa con
      `test_from_values_warns_and_hides_an_ignored_docker_base_path` y
      `test_from_values_does_not_warn_for_a_plain_docker_url`, y el texto de
      journald se conserva **literal** (su test heredado sigue verde).
- [x] 4.9 GREEN: la **causa** de un intento fallido por un status HTTP de error
      es **backend-aware**. Con la API de Docker un `404` se nombra como lo que
      es —el contenedor no existe— y no se culpa a un «gateway» que ese backend
      no tiene; el texto de gatewayd se conserva **literal** (`gateway returned
      HTTP {status}`) como guardián del camino heredado. Cierra el tercer
      `GIVEN` del escenario *Docker API not available* —el contenedor
      inexistente— que 3.6 dejó como hueco en el RED.
      Verificación cumplida: `test_run_survives_a_container_that_does_not_exist`
      pasa —aviso `docker api not available`, endpoint `docker:unix:… (traefik)`,
      causa con el `404` y «no such container», reintento tras `retry_delay` y
      `run()` que devuelve `Ok(())`—, junto con sus dos hermanos del escenario.

## 5. Refactor

- [x] 5.1 REFACTOR: sin `unwrap()`, `expect()` ni `panic!()` en el camino de producción; errores tipados en `JournalError` con la causa en `#[source]`, como el resto del módulo. Verificación cumplida: `grep -n 'unwrap()\|expect(\|panic!' backend/src/journald_reader.rs` no encuentra nada antes del primer `#[cfg(test)]` (línea 502); **todas** las coincidencias caen dentro de módulos de test (`tests_cutoff`, `tests`, `tests_task3`…`tests_task8`). No hubo nada que arreglar.
- [x] 5.2 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde. Verificación cumplida: los dos con exit 0 y **0** avisos (el gate estricto sobre el binario de producción; `fmt` sin reescrituras pendientes).
- [x] 5.3 REFACTOR: comprobar que el gate único de construcción de la petición y el del bucle de streaming siguen siendo **uno solo** para los dos backends. Verificación cumplida: el diff de la rama no añade una segunda `send_request` (`grep -c 'async fn send_request\|async fn send_once'` = **1**) ni un segundo `loop` de reconexión; el bucle de `run()` (reconexión) y el de `stream_once` (troceado) son comunes a los dos backends, y las decisiones por backend tienen un único punto: `StreamFraming::for_transport`, `build_request`, `is_docker` y `docker_request_target()`/`docker_logs_path()`. No había duplicación real que eliminar.
- [x] 5.4 REFACTOR: cerrar los dos cabos que deja el RED. (a) El estado RED
      **rompe a propósito** el gate estricto y el GREEN de 4.1–4.3 lo ha bajado de
      **18** a **11** avisos (exit 101 en los dos casos): los 7 del andamiaje de
      transporte se los llevó 4.3 al escribir sus cuerpos y unificar
      `host_header`/`endpoint_display` en un `match` por transporte. Los 11 que
      quedan son todos del andamiaje neutro todavía en pie —**10** del
      decodificador de frames (`unused_self` ×4, `missing_const_for_fn` ×3,
      `needless_pass_by_ref_mut` ×2, `unnecessary_wraps` ×1) y **1** de 3.5
      (`payload_is_after_cutoff`: `missing_const_for_fn`)—, y desaparecen solos
      cuando 4.4/4.5 escriban los cuerpos reales: **no se silencian** con
      `#[allow]`, porque el `#[allow]` seguiría ahí mintiendo después.
      (b) Los **7 `#[allow(dead_code)]` transitorios** del decodificador —los 2 de
      las variantes `DockerTcp`/`DockerUnixSocket` los retiró 4.2— SHALL **no
      existir** al cerrar el REFACTOR.
      Verificación cumplida: `cargo clippy -- -D warnings` sale con exit 0 y
      `grep -c 'allow(dead_code)' backend/src/journald_reader.rs` devuelve **3**
      (los tres heredados: `socket`, `traefik_parse` y `with_retry_delay`), no 10.
      (c) Se cierra el `redundant_clone` de `journald_reader.rs:2577` —en código
      de **test**— que tumbaba el paso (c) del CI (`cargo clippy --all-targets
      --all-features`, exit 101 → **0**): `docker_socket` no se reusa en el test,
      así que se **mueve** en vez de clonarse. Fue la única modificación de un
      test existente en esta ronda, sin tocar aserciones ni intención, y el test
      `test_docker_variables_win_over_every_journald_variable` sigue pasando.
- [x] 5.5 REFACTOR: cota superior para la longitud declarada en la cabecera de un
      frame de la API de Docker (`MAX_FRAME_LEN`, 8 MiB). La longitud viaja como
      un `u32` —hasta ~4 GiB— y la fija el par (el daemon o, en el modo B, un
      proxy de terceros), así que una cabecera declarando gigabytes es un vector
      de agotamiento de memoria: el decodificador SHALL rechazarla **antes de
      reservar nada**, con un error tipado cuyo `Display` nombra la longitud
      declarada y la cota, y descartar la conexión igual que con `TtyStream` (el
      bucle reconecta y el corte del intento nuevo impide cualquier replay). El
      error es **context-only** (sin `#[source]`), como los que ya existen.
      Verificación cumplida:
      `test_a_frame_declaring_more_than_the_limit_is_refused_without_buffering`
      pasa (RED en `d7da677`, GREEN en `1991820`) y `cargo clippy -- -D warnings`
      sigue con exit 0.

## 6. Documentación

- [x] 6.1 `AGENTS.md` §V: actualizar la tabla de transporte del `JournaldReader` (los dos backends y sus dos entradas), añadir `SHUUL_DOCKER_SOCKET`, `SHUUL_DOCKER_URL` y `SHUUL_DOCKER_CONTAINER` a la tabla de variables de entorno, y dejar escrito cómo pasar del modo A al B. Verificación: `grep -n 'SHUUL_DOCKER' AGENTS.md` encuentra las tres variables. Hecha en `9b7c0d7`.
- [x] 6.2 `SELF-HOSTING.md` §2: documentar las dos entradas, la precedencia, la tabla de seguridad A frente a B, la reversión de la decisión anterior sobre el socket de Docker, y el rollback a gatewayd (restaurar `SHUUL_JOURNALD_*`). Verificación: la sección dice de forma explícita que con el driver en `journald` el journal sigue estando debajo. Hecha en `9b7c0d7`.
- [x] 6.3 Documentar en `SELF-HOSTING.md` que el contenedor **no** puede llevar `tty: true`: con TTY el stream va crudo, sin cabeceras, y el lector descarta la conexión con un warning que lo nombra. Verificación: la sección lo dice junto a la configuración del contenedor, no en un anexo. Hecha en `9b7c0d7`.
- [x] 6.4 Dejar escrito en `SELF-HOSTING.md` que el `group_add` del modo A exige el gid **numérico** del grupo `docker` del host y que cambiarlo requiere `--force-recreate`. Verificación: los comandos son copiables y no hardcodean `999`. Hecha en `9b7c0d7`.
- [x] 6.5 `SELF-HOSTING.md` §2: dejar escrito que el contrato de no-replay **asume el mismo reloj** que el motor —con el daemon en la misma máquina (modos A y B) emisor y lector comparten reloj, como en journald— y que apuntar `SHUUL_DOCKER_URL` a un daemon de **otra máquina** exige relojes sincronizados y queda **fuera del contrato** de este change. Verificación: el texto menciona el desfase de reloj y el requisito de sincronía. Hecha en `9b7c0d7` (la suposición de reloj de §2 y la de la API de Docker).
- [x] 6.6 `backend/src/journald_reader.rs:26-43`: reescribir el comentario de módulo para que describa **los dos backends** y no sólo `systemd-journal-gatewayd` —hoy habla sólo de `__REALTIME_TIMESTAMP`/`__CURSOR` y de un «gatewayd remoto», así que miente por omisión en cuanto el mismo módulo sirva también la API de Docker, y es el fichero donde un dev mira primero—: el **corte por intento de conexión** (la garantía de no-replay), la **comparación exacta por instante de la carga** en el backend de Docker, el **dedupe por `__CURSOR`** en el de journald, y la **limitación de reloj** con sus **dos** direcciones (emisor atrasado → tráfico legítimo descartado en silencio; emisor adelantado → replay del backlog). Verificación cumplida: `grep -n 'API de Docker\|comparación exacta\|__CURSOR\|atrasado\|adelantado' backend/src/journald_reader.rs` encuentra los cinco términos dentro del comentario de módulo (líneas 17–88, antes de la primera línea `use`, línea 102); además documenta la tabla de backends y transportes, la precedencia, la petición de cada uno —con el camino directo de Docker a `parse_traefik_access_log`— y el fail-open con su aviso por backend.
- [x] 6.7 Alineación de las referencias obsoletas a gatewayd **fuera de §2**
      (hecha ya en `5eb7247`, con `9b7c0d7`): `README.md` y `SELF-HOSTING.md`
      §1.1, §6.1, §6.3, §12, §13 y §14 dejan de presentar gatewayd y el plugin
      `traefik-shuul-reporter` como el camino recomendado, y apuntan al
      `JournaldReader` vía la API de Docker. Verificación cumplida: esas
      secciones describen el transporte por la API de Docker, citando `9b7c0d7`
      y `5eb7247`.

## 7. Gates

- [x] 7.1 `cargo test` con el baseline **284** intacto más los tests nuevos, sin tests ignorados nuevos. Verificación cumplida: exit 0 y recuento anotado — **318** verdes (91 + 220 + 4 + 3); el único `ignored` es el doc-test heredado de `log_collector`, no uno nuevo.
- [x] 7.2 Los **tres** comandos que corre el CI (`.github/workflows/ci.yml`, job
      backend, `working-directory: backend`), cada uno con su propósito y su
      verificación propia (exit 0):
      (a) `cargo fmt --all -- --check` (línea 37);
      (b) `cargo clippy -- -D warnings` (línea 40) — **sin** `--all-targets`, o
      sea sólo el código de producción: **éste es el gate estricto**;
      (c) `cargo clippy --all-targets --all-features` (línea 43) — **sin**
      `-D warnings`: incluye los tests, donde los avisos pedantic se reportan
      pero **no** tumban el paso.
      > **Nota (no «arreglar» esto volviendo a juntarlos):** el crate habilita
      > `all`, `pedantic` y `nursery` en `backend/Cargo.toml:39-47`, así que
      > `cargo clippy --all-targets --all-features -- -D warnings` —el comando
      > que decía antes esta tarea— es **inalcanzable**: junta los targets de
      > test, con decenas de avisos pedantic **preexistentes** (medidos: 53 en
      > el lib test, 82 en el bin test y 1 en cada test de integración), con el
      > `-D warnings` del gate estricto. No lo corre nadie y ya salía rojo en
      > `develop`.
      Verificación cumplida: (a) `cargo fmt --all -- --check` exit 0; (b) `cargo clippy -- -D warnings` exit 0; (c) `cargo clippy --all-targets --all-features` exit 0.
- [x] 7.3 `openspec validate docker-logs-transport --strict`. Verificación cumplida: exit 0 (`Change 'docker-logs-transport' is valid`).

## 8. Despliegue en `co1`

> Estas tareas las ejecuta el **usuario**, no el agente. El `compose.yaml` de
> `co1` vive fuera del repositorio.

- [x] 8.1 Preparar **los dos bloques** del `compose.yaml` (modo A con
      `SHUUL_DOCKER_SOCKET` + montaje del socket + `group_add`, modo B con
      `SHUUL_DOCKER_URL` y el `docker-socket-proxy` con `CONTAINERS=1`), sin
      tocar el código para cambiar de uno a otro y **sin** `tty: true`.
      Verificación: cambiar de bloque es editar variables y recrear el
      contenedor. — **Hecho, con una salvedad que hay que declarar:** la
      propiedad que persigue la tarea —cambiar de entrada es editar variables y
      recrear el contenedor, **sin recompilar**— queda verificada por la cascada
      de precedencia de cinco niveles (`SHUUL_DOCKER_SOCKET` →
      `SHUUL_DOCKER_URL` → `SHUUL_JOURNALD_SOCKET` → `SHUUL_JOURNALD_URL` → TCP
      por defecto) y por sus tests. Pero en `co1` **no** se preparó el bloque B:
      el `compose.yaml` del despliegue sólo tiene el modo A
      (`SHUUL_DOCKER_SOCKET: /var/run/docker.sock`,
      `SHUUL_DOCKER_CONTAINER: traefik`, `group_add: ["999"]` y el montaje
      `:ro` del socket), y el bloque B (proxy) no está escrito, ni siquiera
      comentado. Se declara explícitamente: no es un requisito de la fase —el
      modo B vive en la documentación de `SELF-HOSTING.md`— sino una comodidad
      para cambiar de uno a otro.
- [x] 8.2 Desplegar (runbook acordado): imagen `:0.15.x`, `RUST_LOG: backend=info`,
      `RUST_BACKTRACE: full`, `docker compose up -d --force-recreate shuul`.
      Verificación: los logs muestran la línea de arranque con el endpoint de
      Docker. — **Hecho.** La línea de arranque que lo confirma está en los logs:
      `journald reader: streaming access logs from docker:unix:/var/run/docker.sock (traefik)`.
      Dos desviaciones del runbook acordado, que hay que anotar: la imagen
      desplegada **no** está fijada por tag en el `compose.yaml` (usa
      `ghcr.io/atareao/shuul` sin tag, es decir `latest`; el digest en ejecución
      corresponde a la release **0.16.0**, que es la que contiene este cambio
      —el runbook decía `:0.15.x` y era erróneo—), y el nivel de log es
      `RUST_LOG: backend=debug,maxminddb=warn` (el runbook decía `backend=info`).
      `RUST_BACKTRACE: full` sí está puesto. El recreado se hizo con
      `docker compose up -d --force-recreate shuul`.
- [x] 8.3 Confirmar en `co1` que `follow`, `since` y `timestamps` se comportan
      igual con el driver de logging en `journald` (que es el que hay hoy) que
      con `json-file`, antes de dar la fase por buena. Verificación: se anota el
      resultado en el plan §0.2. — **Hecho para el driver `journald`, que es el
      desplegado**, y con la mitad `json-file` **declarada como no verificada**
      por decisión: verificarla exigiría mover los logs de Traefik bajo otro
      colector (alloy/vector/openobserve) y no compensa. Lo verificado con
      `journald`: `follow` entrega en vivo; `timestamps=1` llega como prefijo
      RFC3339Nano que el lector extrae y usa para comparar con el corte del
      intento; `since` actúa como pista gruesa (granularidad de segundo) y la
      corrección la da la comparación exacta; el filtrado por contenedor lo hace
      el servidor. El resultado se anota además en `PLAN.md` §0.2.
- [x] 8.4 Verificar en `co1`: entradas leídas con normalidad, **cero** `BANNED`
      en el primer minuto tras el arranque (no se reprocesa histórico) y el WAF
      intacto. Verificación: recuento de eventos del pipeline Jail y una
      petición de prueba que siga pasando por forwardAuth. — **Hecho, y pasa dos
      veces en frío** (contenedores creados el 2026-10-21 a las 17:23:43 y
      17:58:43): el evento más antiguo del pipeline Jail es 0,4 s **posterior**
      al corte, **cero** eventos con `ts` anterior al corte, y en el primer
      minuto **0 `PENDING` / 0 `BANNED`**. El WAF queda intacto: bloqueó incluso
      una sonda propia con la regla `4 Geo blocking` (`country: DE`). Nota para
      quien lea el conteo en caliente: los `[BANNED]` que se ven después **no**
      son un contador de baneos, son una línea por **cada petición** de una IP
      ya baneada, y los baneos activos se restauran de la base de datos al
      arrancar — es estado, no replay.
- [x] 8.5 Retirar de `co1` el gatewayd, el volumen `/run/systemd/journal`, el
      `group_add` de `systemd-journal` y el puerto `19531`. Verificación: el
      reader sigue leyendo con el modo A o B elegido. — **Hecho.**
      `systemd-journal-gatewayd.socket` está `inactive` y su unidad ya no existe;
      `/run/systemd/journal` no está montado; no hay `group_add` de
      `systemd-journal` ni `SHUUL_JOURNALD_*`; el puerto 19531 no se expone.
      Queda un `/run/systemd/journal/gatewayd.sock` **huérfano** en tmpfs
      (inocuo: `RemoveOnStop=false`), invisible para el contenedor. El lector
      sigue leyendo con el modo A.

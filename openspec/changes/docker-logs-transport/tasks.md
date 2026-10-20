# Tasks: docker-logs-transport

> **Base:** `develop` @ `ec09456` (árbol limpio). Baseline de tests: **284**.
> **Alcance:** sólo el pipeline Jail. El WAF sigue siendo forwardAuth síncrono y
> no se toca.

## 1. Preparación

- [ ] 1.1 `just check-spec` en verde con este change activo. Verificación: la receta sale con exit 0 (antes de crear el change salía con exit 1 y el mensaje `No active change proposal found`).
- [ ] 1.2 Confirmar que las variables y la precedencia de §0.3 del plan son las que implementan los escenarios de `specs/journald-reader/spec.md`. Verificación: `openspec validate docker-logs-transport --strict` sin errores y `deltaCount` 3 (dos MODIFIED y un ADDED).
- [ ] 1.3 Decisión registrada (plan §0.5.2): este change **mantiene** el soporte de `systemd-journal-gatewayd` como backend de menor prioridad; no se retira. Verificación: `design.md` §D8 lo dice y los escenarios de journald siguen en el delta.
- [ ] 1.4 `design.md` escrito con las decisiones que no son evidentes desde las specs (contenedor, variantes aditivas, `stderr`, reparto `since`/`timestamps`, TTY, asimetrías aceptadas). Verificación: `openspec status --change docker-logs-transport` marca `4/4 artifacts complete`.

## 2. Decodificación de los frames (RED)

- [ ] 2.1 RED: tests unitarios del decodificador incremental con frames construidos a mano: cabecera de 8 bytes (stream, 3 de relleno, longitud big-endian), un frame **partido entre dos chunks**, dos frames en un chunk, un frame con carga vacía y una cabecera truncada al cerrarse el stream. Verificación: `cargo test journald_reader` falla porque el decodificador todavía no existe.
- [ ] 2.2 RED: tests del identificador de stream: `1` (stdout) llega al pipeline, `2` (stderr) se descarta. Verificación: el test falla por API inexistente.
- [ ] 2.3 RED: test de **detección de TTY**: un primer byte que no sea `0`, `1` ni `2` (stream crudo, un JSON empieza por `{`) descarta la conexión con un warning que nombra la causa probable, en vez de seguir decodificando. Verificación: el test falla; el warning incluye la palabra `TTY` o equivalente explícito.
- [ ] 2.4 RED: test de extracción del prefijo de instante: una carga `<instante RFC3339Nano> {…}` se parte en instante + línea, y el `ReportPayload` llega al pipeline con los mismos campos que sin el prefijo. Verificación: el test falla.

## 3. Transporte Docker (RED)

- [ ] 3.1 RED: test de precedencia con las **cuatro** variables de transporte y los cinco niveles, pasando los cinco argumentos a `from_values` en el orden de la precedencia: sólo `SHUUL_DOCKER_SOCKET`; sólo `SHUUL_DOCKER_URL`; socket y URL de Docker a la vez; las dos de Docker y las dos de journald; y el caso en blanco. Verificación: el test falla —y **no compila** contra la firma actual de tres parámetros, que es justamente la señal de que el contrato hay que cambiarlo.
- [ ] 3.2 RED: test del target y de las cabeceras de la petición de Docker: `GET /v1.44/containers/{container}/logs?follow=1&stdout=1&stderr=1&tail=0&timestamps=1&since={epoch}`, `Accept: application/json`, `Host: docker` por socket y la autoridad del endpoint por TCP, y **ausencia** de `Upgrade`. Verificación: el test falla por API inexistente.
- [ ] 3.3 RED: test de `endpoint_display()` para las dos entradas de Docker (`docker:unix:… (traefik)` y `docker:http://…:2375 (traefik)`). Verificación: el test falla.
- [ ] 3.4 RED: servidor `hyper` de prueba sobre un socket Unix temporal **y** sobre TCP que responda con `200 OK` y `Content-Type: application/vnd.docker.raw-stream`, emitiendo en **ambos** los mismos frames multiplexados construidos a mano —incluido a propósito un frame partido entre dos chunks, un chunk con dos frames y una carga con el prefijo de instante— y exigiendo que las líneas de stdout lleguen al pipeline Jail. Verificación: el test falla; los tests existentes del transporte de gatewayd siguen en verde.
- [ ] 3.5 RED: test de no-replay del backend de Docker: (a) el histórico anterior al corte no llega al pipeline; (b) tras reconectar, el `since` de la petición lleva el corte del **intento nuevo** —comprobado **como forma de la petición**, igual que en 3.2, y **no** como garantía de corrección: si un día se olvidara ponerlo, el lector seguiría siendo correcto porque es la comparación estricta la que descarta, sólo viajaría de más— y **ninguna** carga del segundo de ese corte se cuenta dos veces (esa no-duplicación la da la comparación estricta, no el `since`); (c) dos cargas de **contenido idéntico** con instantes distintos —los dos posteriores al corte— se cuentan las dos; (d) una carga con el instante **igual** al corte del intento se descarta; (e) una carga sin prefijo o con prefijo no parseable se descarta sin abortar el stream; (f) un intento fallido desplaza el corte, de modo que lo ocurrido durante la caída no llega al pipeline. Verificación: el test falla.
- [ ] 3.6 RED: test de fail-open con el backend de Docker: socket ausente y proxy inalcanzable → warning `docker api not available`, reintento tras `retry_delay` y `run()` que devuelve `Ok(())` al recibir la señal de parada, sin dejar tareas colgadas. Verificación: el test falla.
- [ ] 3.7 RED: test de shutdown a mitad de stream con el backend de Docker. Verificación: el test termina en menos de un segundo y devuelve `Ok(())`.

## 4. Implementación (GREEN)

- [ ] 4.1 GREEN: `from_values` pasa a los **cinco** parámetros (Docker socket, Docker URL, journald socket, journald URL, contenedor) en el orden de la precedencia, y se migran las **~15 llamadas de `#[cfg(test)]`** que hoy pasan tres (`journald_reader.rs:3299-3572`) —trabajo mecánico, pero hacerlo en el mismo commit que el cambio de firma—. Verificación: los tests de 3.1 pasan y no queda ninguna llamada con tres argumentos.
- [ ] 4.2 GREEN: añadir `DockerTcp` y `DockerUnixSocket` a `JournaldTransport` y la resolución de la precedencia de cinco niveles en `from_env()`, con el `warn!` de la variable ignorada. Verificación: los tests de 3.1 pasan.
- [ ] 4.3 GREEN: implementar el target, las cabeceras y el `endpoint_display()` del backend de Docker sobre el cliente `hyper` que ya existe (mismo `Io` borrado, mismo `CONNECT_TIMEOUT`, misma petición en forma origen). Verificación: los tests de 3.2, 3.3 y 3.4 pasan.
- [ ] 4.4 GREEN: implementar el decodificador incremental de frames por delante de `drain_ndjson_lines`, respetando el corte por frontera de chunk, el descarte de `stderr` y la detección de TTY. Verificación: los tests de 2.1, 2.2, 2.3 y 3.4 pasan.
- [ ] 4.5 GREEN: extraer el prefijo de instante de cada carga y entregar el resto a `parse_traefik_access_log`, sin pasar por `extract_report_payload`/`is_traefik_entry` en el camino de Docker (exigirían `CONTAINER_NAME` y descartarían el 100 % de las líneas). Verificación: los tests de 2.4 y 3.4 pasan y el pipeline recibe los payloads con sus campos.
- [ ] 4.6 GREEN: corte por intento —**estrictamente posterior** al corte del intento en curso— y `since` como pista gruesa: el corte del intento en **cada** intento, también tras reconectar y tras un intento fallido; **nunca** el instante de la última carga procesada. **No** comparar contenido de línea. Verificación: los tests de 3.5 y los escenarios de *No history replay* pasan.
- [ ] 4.7 GREEN: fail-open y shutdown del backend de Docker reutilizando el bucle de `run()` y `wait_for_shutdown`, sin duplicar el bucle. Verificación: los tests de 3.6 y 3.7 pasan, junto con los de gatewayd.

## 5. Refactor

- [ ] 5.1 REFACTOR: sin `unwrap()`, `expect()` ni `panic!()` en el camino de producción; errores tipados en `JournalError` con la causa en `#[source]`, como el resto del módulo. Verificación: `grep -n 'unwrap()\|expect(\|panic!' backend/src/journald_reader.rs` no encuentra nada fuera de `#[cfg(test)]`.
- [ ] 5.2 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.
- [ ] 5.3 REFACTOR: comprobar que el gate único de construcción de la petición y el del bucle de streaming siguen siendo **uno solo** para los dos backends. Verificación: el diff no añade una segunda `send_request` ni un segundo `loop` de reconexión.

## 6. Documentación

- [ ] 6.1 `AGENTS.md` §V: actualizar la tabla de transporte del `JournaldReader` (los dos backends y sus dos entradas), añadir `SHUUL_DOCKER_SOCKET`, `SHUUL_DOCKER_URL` y `SHUUL_DOCKER_CONTAINER` a la tabla de variables de entorno, y dejar escrito cómo pasar del modo A al B. Verificación: `grep -n 'SHUUL_DOCKER' AGENTS.md` encuentra las tres variables.
- [ ] 6.2 `SELF-HOSTING.md` §2: documentar las dos entradas, la precedencia, la tabla de seguridad A frente a B, la reversión de la decisión anterior sobre el socket de Docker, y el rollback a gatewayd (restaurar `SHUUL_JOURNALD_*`). Verificación: la sección dice de forma explícita que con el driver en `journald` el journal sigue estando debajo.
- [ ] 6.3 Documentar en `SELF-HOSTING.md` que el contenedor **no** puede llevar `tty: true`: con TTY el stream va crudo, sin cabeceras, y el lector descarta la conexión con un warning que lo nombra. Verificación: la sección lo dice junto a la configuración del contenedor, no en un anexo.
- [ ] 6.4 Dejar escrito en `SELF-HOSTING.md` que el `group_add` del modo A exige el gid **numérico** del grupo `docker` del host y que cambiarlo requiere `--force-recreate`. Verificación: los comandos son copiables y no hardcodean `999`.
- [ ] 6.5 `SELF-HOSTING.md` §2: dejar escrito que el contrato de no-replay **asume el mismo reloj** que el motor —con el daemon en la misma máquina (modos A y B) emisor y lector comparten reloj, como en journald— y que apuntar `SHUUL_DOCKER_URL` a un daemon de **otra máquina** exige relojes sincronizados y queda **fuera del contrato** de este change. Verificación: el texto menciona el desfase de reloj y el requisito de sincronía.
- [ ] 6.6 `backend/src/journald_reader.rs:26-43`: reescribir el comentario de módulo para que describa **los dos backends** y no sólo `systemd-journal-gatewayd` —hoy habla sólo de `__REALTIME_TIMESTAMP`/`__CURSOR` y de un «gatewayd remoto», así que miente por omisión en cuanto el mismo módulo sirva también la API de Docker, y es el fichero donde un dev mira primero—: el **corte por intento de conexión** (la garantía de no-replay), la **comparación exacta por instante de la carga** en el backend de Docker, el **dedupe por `__CURSOR`** en el de journald, y la **limitación de reloj** con sus **dos** direcciones (emisor atrasado → tráfico legítimo descartado en silencio; emisor adelantado → replay del backlog). Verificación: `grep -n 'API de Docker\|comparación exacta\|__CURSOR\|atrasado\|adelantado' backend/src/journald_reader.rs` encuentra los cinco términos dentro del comentario de módulo (antes de la primera línea `use`).

## 7. Gates

- [ ] 7.1 `cargo test` con el baseline **284** intacto más los tests nuevos, sin tests ignorados nuevos. Verificación: exit 0 y recuento anotado.
- [ ] 7.2 `cargo clippy --all-targets --all-features -- -D warnings` y `cargo fmt -- --check`. Verificación: ambos con exit 0.
- [ ] 7.3 `openspec validate docker-logs-transport --strict`. Verificación: exit 0.

## 8. Despliegue en `co1`

> Estas tareas las ejecuta el **usuario**, no el agente. El `compose.yaml` de
> `co1` vive fuera del repositorio.

- [ ] 8.1 Preparar **los dos bloques** del `compose.yaml` (modo A con
      `SHUUL_DOCKER_SOCKET` + montaje del socket + `group_add`, modo B con
      `SHUUL_DOCKER_URL` y el `docker-socket-proxy` con `CONTAINERS=1`), sin
      tocar el código para cambiar de uno a otro y **sin** `tty: true`.
      Verificación: cambiar de bloque es editar variables y recrear el
      contenedor.
- [ ] 8.2 Desplegar (runbook acordado): imagen `:0.15.x`, `RUST_LOG: backend=info`,
      `RUST_BACKTRACE: full`, `docker compose up -d --force-recreate shuul`.
      Verificación: los logs muestran la línea de arranque con el endpoint de
      Docker.
- [ ] 8.3 Confirmar en `co1` que `follow`, `since` y `timestamps` se comportan
      igual con el driver de logging en `journald` (que es el que hay hoy) que
      con `json-file`, antes de dar la fase por buena. Verificación: se anota el
      resultado en el plan §0.2.
- [ ] 8.4 Verificar en `co1`: entradas leídas con normalidad, **cero** `BANNED`
      en el primer minuto tras el arranque (no se reprocesa histórico) y el WAF
      intacto. Verificación: recuento de eventos del pipeline Jail y una
      petición de prueba que siga pasando por forwardAuth.
- [ ] 8.5 Retirar de `co1` el gatewayd, el volumen `/run/systemd/journal`, el
      `group_add` de `systemd-journal` y el puerto `19531`. Verificación: el
      reader sigue leyendo con el modo A o B elegido.

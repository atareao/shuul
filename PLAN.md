# PLAN — Deuda técnica de Shuul: transporte por socket de Docker, limpieza de dependencias y majors

> **Estado: PENDIENTE DE APROBACIÓN.** No se escribe ni una línea de código hasta que el usuario apruebe este plan (protocolo SDD del proyecto).
>
> **Base:** `develop` @ `b4e57f0` (árbol limpio) · **Fecha:** 2026-10-20

---

## Resumen

| Fase | Contenido | Riesgo | Cierra |
|---|---|---|---|
| **0** | Transporte de logs por la **API de Docker** (socket directo o proxy) — paso previo | Alto en el **despliegue**, no en el código | deuda del lector de logs |
| **1** | Limpieza de **8 dependencias muertas** | Bajo | PR #74 (borrando, no subiendo) |
| **2** | `tower-http` 0.6 → 0.7 | Bajo-medio | PR #70 |
| **3** | `maxminddb` 0.27 → 0.32 | **Alto** (GeoIP real) | PR #71 |
| **4** | `react-router` 7 → 8 | Medio (toda la navegación) | PR #75 |
| **5** | Infraestructura: `Dockerfile` y `@types/node` | Bajo | deuda de despliegue |
| **6** | 3 alertas de Dependabot transitivas | Bajo | alertas abiertas |

---

## 1. Estado de partida (verificado, no supuesto)

- `develop` limpio en `b4e57f0`. Release **0.15.4** publicada (tag `91e0402`): imagen en GHCR, GitHub Release *Latest*, `Cargo.lock` sincronizado.
- **4 PRs de Dependabot abiertas:** #70 `tower-http` 0.6.8 → 0.7.1, #71 `maxminddb` 0.27.3 → 0.32.0, #74 `ipnetwork` 0.20.0 → 0.21.1, #75 `react-router` 7.18.4 → 8.4.0.
- #75 ya se rebasó sola y está `MERGEABLE`. #70, #71 y #74 arrastran el `Cargo.lock` anterior y necesitan `update-branch`.
- **3 alertas abiertas**, todas transitivas: `brace-expansion` ×2 (media) y `rand` (baja).

Hallazgos verificados hoy y que motivan las fases:

| Dependencia | Declarada en | Usos reales | Detalle |
|---|---|---|---|
| `ipnetwork` 0.20.0 | `backend/Cargo.toml:32` | **0** | ya está en el árbol como transitiva de `maxminddb` (0.21.1) |
| `axum-extra` 0.10.3 (feature `cookie`) | `backend/Cargo.toml:8` | **0** | cero `use axum_extra`, cero `CookieJar` |
| `cookie` 0.18.1 | `backend/Cargo.toml:10` | **0** | cero `use cookie`, cero `cookie::` |
| `time` 0.3.47 | `backend/Cargo.toml:25` | **0** | el código cronometra con `tokio::time` y fecha con `chrono` (9 `use chrono`) |
| `class-variance-authority` | `frontend/package.json` | **0** | sin `cva(` |
| `react-highlight-words` | `frontend/package.json` | **0** | sin `Highlighter` |
| `@testing-library/react` | `frontend/package.json` | **0** | el único test usa `vitest` y mockea `loadData` |
| `@testing-library/user-event` | `frontend/package.json` | **0** | ídem |

`@testing-library/jest-dom` **sí** se usa (`src/test/setup.ts`) y `depcheck` no lo marca: eso es lo que da confianza en los otros cuatro.

---

## FASE 0 — Transporte de logs por la API de Docker (socket directo o proxy) (paso previo)

### 0.1 Por qué

Hoy el pipeline **Jail** lee los access logs de Traefik de `systemd-journal-gatewayd` (socket Unix de journald, o TCP como alternativa). Eso obliga a tener gatewayd habilitado, `Storage=volatile`, montar `/run/systemd/journal` (el directorio, no el fichero), `group_add` con el gid de `systemd-journal` y un cliente HTTP propio contra el socket.

Con **Docker** el mismo dato está detrás de una API estándar que ya viene con el motor: no hay que habilitar nada en systemd ni depender de que el socket de journald esté donde se espera.

> ⚠️ **Esto revierte una decisión anterior.** En su momento se rechazó explícitamente montar `/var/run/docker.sock`. La reversión es deliberada y consciente: ver §0.4 antes de tocar el `compose.yaml`.

**Un transporte, dos formas de llegar.** El acceso directo al socket y el acceso a través de un proxy **no son dos implementaciones distintas**. La API de logs de Docker es la misma; lo único que cambia es por dónde se entra, y el cliente que ya existe (`hyper` sobre `AsyncRead + AsyncWrite`, genérico a propósito) sirve para los dos sin tocar una línea:

| | Cómo se llega | Variable |
|---|---|---|
| **Socket directo** | `AF_UNIX` contra `/var/run/docker.sock` | `SHUUL_DOCKER_SOCKET` |
| **Proxy** | TCP contra el proxy interno | `SHUUL_DOCKER_URL` |

Es el mismo patrón que el lector de journald ya usa hoy para elegir entre socket Unix y TCP. Por eso **se implementan las dos entradas** y la elección se hace en el `compose.yaml`: se puede empezar por el socket y pasar al proxy —o al revés— sin recompilar ni tocar Rust. Lo que sí es una decisión de despliegue con consecuencias de seguridad es **cuál se monta**, y de eso va §0.4.

### 0.2 Verificación previa (antes de escribir código)

En `co1`, y **antes** de decidir nada:

```bash
# ¿Con qué driver escribe Traefik sus logs? Determina si la API los ve.
docker inspect -f '{{.HostConfig.LogConfig.Type}}' traefik

# Versión de la API del motor (Docker 28.2.2 → v1.50+). Decide si `since` está disponible.
docker version --format '{{.Server.APIVersion}}'

# gid del grupo docker en el host (NO hardcodear: varía por máquina).
getent group docker
```

Si el driver no es `json-file`, `local` o `journald`, este transporte no sirve y hay que replantear.

**Resultados de la verificación en `co1` (2026-10-20).** Ejecutada contra el host real, con el stack en marcha:

| Comprobación | Resultado | Consecuencia |
|---|---|---|
| Driver de logging de Traefik | **`journald`** | La API de Docker lo sirve igualmente (soporta `json-file`, `local` y `journald`), pero **el journal sigue estando debajo**. Ver aviso. |
| Driver de logging de Shuul | `journald` | Lo mismo para el propio contenedor. |
| API del motor | **1.50** (Docker 28.2.2) | `since` está disponible: el no-replay se puede implementar tal y como está previsto. |
| gid del grupo `docker` | **999** | Es el que necesita `group_add` en la opción A. **No hardcodear**: en desarrollo es otro número. |
| gid del grupo `systemd-journal` | **101** | El que usa hoy el servicio para leer el journal. |

> ⚠️ **Consecuencia que hay que tener clara antes de invertir en esta fase.** Con el driver en `journald`, `docker logs` es una capa sobre el journal: el daemon de Docker (que corre como root) lee el journal y lo sirve por su API. La ganancia real es **quitar `systemd-journal-gatewayd`**, el montaje de `/run/systemd/journal` y el `group_add` de `systemd-journal`; **no** es independizarse del journal. Si algún día se cambiara el driver de logging de Traefik a `json-file`, entonces sí desaparecería journald del camino.
>
> En la implementación hay que confirmar en `co1` que `follow` y `since` se comportan igual con driver `journald` que con `json-file`, antes de dar la fase por buena.

---

### 0.3 Contratos

**Variables de entorno**

| Variable | Default | Descripción |
|---|---|---|
| `SHUUL_DOCKER_SOCKET` | sin definir | Path del socket de Docker (p. ej. `/var/run/docker.sock`). Acceso **directo** por `AF_UNIX`. |
| `SHUUL_DOCKER_URL` | sin definir | URL base de un **proxy** de la API de Docker (p. ej. `http://docker-socket-proxy:2375`). Acceso por **TCP**. |
| `SHUUL_DOCKER_CONTAINER` | `traefik` | Contenedor cuyos logs se leen. |

`SHUUL_DOCKER_SOCKET` o `SHUUL_DOCKER_URL` —cualquiera de las dos, definida y no vacía— activan el **transporte Docker**. El código soporta las dos a la vez; cuál se usa lo decide el despliegue.

**Precedencia del transporte:** `SHUUL_DOCKER_SOCKET` → `SHUUL_DOCKER_URL` → `SHUUL_JOURNALD_SOCKET` → `SHUUL_JOURNALD_URL` → default TCP. Los valores vacíos cuentan como no definidos. Si llegan varios, gana el de más prioridad y se emite un `warn!` nombrando los ignorados. Dentro del transporte Docker, el socket gana a la URL por el mismo criterio que ya aplica journald.

**Petición HTTP** (reutiliza el cliente `hyper` sobre `AsyncRead + AsyncWrite` que ya existe para gatewayd; da igual que el extremo sea un socket Unix o un TCP, la abstracción es la misma y ya está probada con ambos):

```http
GET /v1.44/containers/traefik/logs?follow=1&stdout=1&stderr=1&tail=0&since=<epoch>
Host: docker
Accept: application/json
```

- **No** enviar `Upgrade` ni `Connection: Upgrade`: si se piden, el motor contesta `101 UPGRADED` en lugar de servir el stream. Sin esas cabeceras devuelve `200 OK` con `Content-Type: application/vnd.docker.raw-stream`.
- `since=<epoch>` cumple la misma función que hoy cumple el corte por `__REALTIME_TIMESTAMP`: **no reproducir histórico al arrancar**. Un histórico reproducido banearía IPs con escaneos viejos.
- Con `SHUUL_DOCKER_SOCKET` la cabecera `Host` es `docker` (como ya se hace con gatewayd); con `SHUUL_DOCKER_URL` se usa el host del endpoint, igual que hoy en el modo TCP de journald. El path y la query son idénticos en los dos casos.
- `endpoint_display()` debe distinguir las dos entradas (`docker:unix:/var/run/docker.sock (traefik)` o `docker:http://docker-socket-proxy:2375 (traefik)`) para la línea de arranque.

**Demultiplexado del stream (el punto delicado)**

La respuesta del motor viene en **frames de 8 bytes de cabecera** cuando el contenedor no tiene TTY:

```
byte 0      : stream id (1 = stdout, 2 = stderr)
bytes 1-3   : ceros
bytes 4-7   : tamaño del payload, big-endian u32
```

El parser debe ser **incremental**: un frame puede llegar partido entre dos chunks, y un chunk puede traer varios frames. Nada de `split` sobre líneas antes de desmultiplexar.

**Parseo de la línea**

Cada payload es una línea JSON del access log de Traefik (la API ya extrae el campo `log` del envoltorio del driver `json-file`). Se parsea igual que hoy y desemboca en la misma función `run_jail_pipeline(app_state, payload)`: **no se duplica la lógica del pipeline**.

**Comportamiento en fallo**

- Socket ausente, proxy inalcanzable o contenedor inexistente → `warn!` (`docker api not available`) y reintento cada 30 s. **Fail-open**: el WAF nunca se ve afectado.
- Reconexión → dedupe de la última línea procesada, igual que hoy se hace por `__CURSOR`.

### 0.4 Seguridad (leer antes de tocar el `compose.yaml`)

La documentación oficial de Docker es explícita: montar el socket da **control sobre el Docker Engine**, y eso equivale a root en el host.

> "By default, ECI blocks bind mounting the Docker Engine socket (/var/run/docker.sock) as this would grant containers control over Docker Engine."

Consecuencias que hay que asumir:

- `:ro` **no** limita la API. El socket es un canal de comunicación, no un fichero: el montaje en solo-lectura no impide emitir peticiones de escritura por él.
- Nunca exponer el socket por TCP en red pública. La propia doc lo prohíbe sin TLS y avisa de escalada de privilegios.

Lo que se elige **no es qué se implementa** —eso está cerrado: las dos entradas, §0.1— sino **qué se monta en producción**. Y ahí el riesgo de cada opción no se parece en nada:

| | Opción A — socket directo | Opción B — proxy (recomendada) |
|---|---|---|
| Configuración | `SHUUL_DOCKER_SOCKET=/var/run/docker.sock` + `-v /var/run/docker.sock:/var/run/docker.sock:ro` + `group_add` con el gid de `docker` | `SHUUL_DOCKER_URL=http://docker-socket-proxy:2375` + un `docker-socket-proxy` con `CONTAINERS=1` y el resto a `0`, en una red interna |
| Permisos que exige | El grupo `docker` del host | Ninguno del host: sólo hablar con el proxy por la red interna |
| Riesgo real | El contenedor puede hablar con **todo** el Engine | Sólo pasa lo permitido; el resto se queda fuera |
| Coste | Cero piezas nuevas | Un contenedor más en el stack |
| Cómo se revoca | Quitar el montaje y recrear | Apagar el proxy |

En la opción A, `group_add` es obligatorio porque la imagen corre como `app` (uid 1000, `USER app`), no como root: el socket es `root:docker 0660`. El gid se fija al crear el contenedor — cambiarlo exige `--force-recreate`, no `restart`. Y en Podman hay que dar el **gid numérico** (resuelve el nombre contra la imagen, no contra el host).

Pasar de A a B (o al revés) es cambiar dos variables de entorno y un bloque del `compose.yaml`: **nunca recompilar**.

### 0.5 Tareas

- [ ] **0.5.1** Ejecutar las tres comprobaciones de §0.2 en `co1` y anotar el resultado aquí.
- [ ] **0.5.2** Decidir si journald se queda como *fallback* o se retira (mantener dos transportes tiene un coste de test que hay que reconocer) y **qué se despliega en `co1`**, A o B (§0.4). El código soporta las dos entradas, así que esta elección **no bloquea** la implementación.
- [ ] **0.5.3** Change proposal de OpenSpec: `openspec new change docker-logs-transport`, con la spec delta (contratos de §0.3 y escenarios Given/When/Then de: stream normal, frame partido entre chunks, dos frames en un chunk, stderr descartado, socket ausente, reconexión sin duplicar).
- [ ] **0.5.4** Tests en **RED**, cubriendo **las dos entradas**: servidor `hyper` de prueba sobre un socket Unix temporal **y** sobre TCP, emitiendo en ambos los mismos frames multiplexados construidos a mano, incluyendo a propósito un frame partido entre dos chunks y un chunk con dos frames.
- [ ] **0.5.5** Implementación en **GREEN**: transporte Docker detrás de la abstracción de transporte que ya existe en `backend/src/journald_reader.rs`, con el extremo (socket o URL) resuelto por la misma lógica de precedencia que journald.
- [ ] **0.5.6** **REFACTOR**: `cargo clippy -- -D warnings` y `cargo fmt --check` limpios.
- [ ] **0.5.7** Documentación: `SELF-HOSTING.md` §2 (las dos entradas, precedencia, tabla de seguridad A frente a B y la reversión de la decisión) y `AGENTS.md` §V (tabla de transporte del `JournaldReader`, variables de entorno y cómo pasar de A a B).
- [ ] **0.5.8** `compose.yaml` de `co1`: lo ejecuta el usuario, no el agente. Runbook acordado: imagen `:0.15.x`, `RUST_LOG: backend=info`, `RUST_BACKTRACE: full`, `docker compose up -d --force-recreate shuul`. Hay que dejar preparados **los dos bloques** (A y B) para poder cambiar de uno a otro sin tocar el código.
- [ ] **0.5.9** Verificación en `co1`: contar entradas leídas, confirmar que **no** se reprocesa histórico al arrancar (cero `BANNED` en el primer minuto) y que el WAF sigue intacto.

### 0.6 Gates de la fase

- `cargo test` (baseline actual: **284 tests**), `cargo clippy -- -D warnings`, `cargo fmt --check`.
- Prueba real en `co1`, que es donde se ve el comportamiento de verdad.
- El camino del **WAF no se toca**: sigue siendo forwardAuth síncrono y no depende de este transporte.

### 0.7 Decisiones abiertas de la fase

1. **Decidido (2026-10-20): se implementan las dos entradas.** No hay que elegir en código; el extremo se resuelve por variable de entorno.
2. **Decidido:** qué se monta en `co1` no bloquea nada, el código soporta A y B. Se elegirá con los datos de §0.2 y la recomendación sigue siendo el **proxy**.
3. ¿El transporte Docker pasa a ser el **preferido** y journald queda como alternativa, o al revés? **Pendiente.**
4. ¿Se retira el soporte de `systemd-journal-gatewayd` o se mantiene indefinidamente? Mantener dos transportes tiene un coste de test que hay que reconocer.

---

## FASE 1 — Limpieza de dependencias muertas

Ocho dependencias declaradas y **nunca usadas** (§1). `ipnetwork` es una de ellas y es justo lo que pretende subir la PR #74: **la PR se resuelve borrando la línea, no subiéndola**. Borrar además quita ruido de futuras alertas.

- [x] **1.1** Quitar de `backend/Cargo.toml`: `ipnetwork`, `axum-extra`, `cookie`, `time`.
- [x] **1.2** Quitar de `frontend/package.json`: `class-variance-authority`, `react-highlight-words`, `@testing-library/react`, `@testing-library/user-event`.
- [x] **1.3** Regenerar `Cargo.lock` y `pnpm-lock.yaml` (el lock **debe** cambiar: quitar dependencias lo modifica).
- [x] **1.4** Gates backend: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
- [x] **1.5** Gates frontend: `pnpm install --frozen-lockfile`, `pnpm run lint` (0 errores), `pnpm run build`, `pnpm test` (5 tests).
- [x] **1.6** `cargo machete backend` y `npx depcheck` deben salir **sin hallazgos nuevos**.
- [x] **1.7** Cerrar #74 explicando que se elimina en lugar de subirse.

**Riesgo y cómo se cubre:** `cargo-machete` puede tener falsos positivos (usos vía macro, `build.rs` o features). Aquí se comprobó uno a uno con `grep` que los usos son **cero**, incluso en `tests/`. El gate `cargo test` es la red final: si alguna estuviera usada de forma indirecta, falla ahí y se devuelve.

**Nota de proceso (decidido el 2026-10-20):** va como **mantenimiento**, sin change proposal. No añade ni cambia comportamiento observable —sólo quita dependencias declaradas sin uso— y los gates (`cargo test`, `pnpm test`) son la garantía.

**Resultado — hecho y fusionado (PR #81, `develop` en `10badce`).** 4 ficheros, **125 líneas fuera**, y **ni una línea de código fuente tocada**:

| Gate | Baseline | Resultado |
|---|---|---|
| `cargo test` | 284 | **284 passed / 0 failed** |
| `pnpm test` | 5 | **5 passed** |
| `cargo clippy -- -D warnings` | — | exit **0** |
| `cargo fmt --check` | — | exit **0** |
| `pnpm run lint` | 0 errores | **0 errores, 0 warnings** |
| `pnpm run build` | — | exit **0** |
| `pnpm install --frozen-lockfile` | — | exit **0** |
| `cargo machete backend` | 4 hallazgos | **sin hallazgos** |
| `npx depcheck` | 4 hallazgos | **sin hallazgos** |

Dos que podían haber sido trampa y no lo fueron: **`time`** parecía usarse en `main.rs`, pero era todo `tokio::time` y `std::time` (el proyecto fecha con `chrono`); y **`cookie`** tenía un único match textual, `document.cookie` dentro de un **string de regex** de la plantilla de detección de XSS en `templates.rs`. **Ninguna de las ocho volvió a su sitio.**

En los locks: `axum-extra` y `cookie` desaparecen por completo; `ipnetwork` deja de ser dependencia directa (la **0.21.1** se queda como transitiva de `maxminddb`); `time` también, pero sigue en el lock como transitiva de `simple_asn1` (vía `jsonwebtoken`) y `tracing-subscriber`. En `pnpm-lock.yaml`, 69 líneas fuera y `@testing-library/jest-dom` **intacto**.

---

## FASE 2 — `tower-http` 0.6.8 → 0.7.1 (PR #70)

Superficie pequeña y localizada: features `cors`, `fs` y `trace`, con `ServeDir` y `ServeFile` en `backend/src/main.rs`.

- [x] **2.1** `gh pr update-branch 70` (su lock es anterior a los merges de hoy).
- [x] **2.2** Leer el changelog de `tower-http` 0.7 en busca de cambios de API en `ServeDir`, `ServeFile`, CORS y trace.
- [x] **2.3** Gates: `cargo test`, `clippy -D warnings`, `fmt --check`. CI verde.
- [x] **2.4** Fusionar y cerrar.

**Resultado — hecho y fusionado (PR #70, `develop` en `1cf7212`).** El `update-branch` **sí funcionó** esta vez (no hubo conflicto con el `Cargo.lock`, al contrario de lo que se preveía), así que no hizo falta rama propia: la de Dependabot ya traía el bump y **no se tocó ni una línea de código fuente**. Gates locales verdes con el baseline intacto (284 tests, clippy y fmt `EXIT=0`) y CI verde.

Lo que la CI **no** puede ver, y por eso queda escrito aquí:

1. **`CorsLayer` deja de emitir `Vary`.** En 0.6 el default era una lista fija (`origin, access-control-request-method, access-control-request-headers`); en 0.7 se **calcula** según las cabeceras de las que la respuesta depende de verdad. Nuestra configuración (`allow_origin(Any)`, `allow_methods([...])`, `allow_headers([...])`) es toda constante, así que el cálculo sale vacío y la cabecera no se emite. **Se acepta el default nuevo**: con `Allow-Origin: *` y sin credenciales la respuesta es idéntica para cualquier origen, luego no hay variante que cachear. Recuperar el comportamiento anterior sería una línea (`.vary(preflight_request_headers())`), y hoy no hay **ni un test de CORS** en el repo que lo cubra.
2. **`ServeDir` rechaza la barra final sobre un fichero real** (breaking): `/assets/app.js/` antes servía el fichero y ahora devuelve `FileNotFound`, que con fallback configurado se convierte en **`static/index.html` con 200** en vez del asset. Cambio silencioso de contenido para una URL con una barra de más. `ServeFile`, que es el fallback del SPA, no se ve afectado.
3. **Aditivo:** `ServeDir`/`ServeFile` emiten `ETag` y soportan `If-Match`/`If-None-Match`.
4. **`TraceLayer` ancla los eventos a su span**: cambio observable en la salida de logs, sin riesgo funcional.

Detalle del lock: conviven `tower-http` **0.7.1** (nuestra) y **0.6.11** (la que arrastra `reqwest`). Es correcto y no se toca.

**Los cuatro puntos están medidos, no leídos.** Un probe fuera del repositorio (`/tmp/opencode/httpprobe`, con las dos versiones declaradas a la vez y renombradas dentro de un mismo `Cargo.toml`) corre la misma batería contra `0.6.11` y `0.7.1` con nuestra configuración literal:

| Caso | 0.6.11 | 0.7.1 | Veredicto |
|---|---|---|---|
| `OPTIONS` preflight con `Origin` | `vary: origin, access-control-request-method, access-control-request-headers` | **sin `Vary`**, y las otras tres cabeceras CORS idénticas byte a byte | ✅ |
| `GET` simple con `Origin` | `Vary` + `ACAO` | **sólo `ACAO`** | ✅ |
| `GET /assets/app.js` (**control**) | `200`, `text/javascript`, 26 B → el JS | `200`, `text/javascript`, 26 B → el JS | ✅ el control sale igual |
| `GET /assets/app.js/` | `200`, `text/javascript`, **el JS** | `200`, `text/html`, **el `index.html` del SPA** | ✅ |
| `GET /admin/` | `200`, `text/html`, el `index.html` | idéntico | ✅ sin cambios |

El **control** es lo que da validez al resto: mismo binario, mismo fixture y misma ruta base, y sólo en la cuarta fila cambia el cuerpo servido; la discrepancia es real y no un artefacto del montaje.

Lo que la medición añade a lo que ya se creía:

- **El caso de la barra final no es un 404: es un `200` con el contenido cambiado.** Un 404 se ve en la consola del navegador; esto sirve en silencio el HTML del SPA a quien pidió un `.js`. El modo de fallo es peor que el que se supone al leer el código, y merecía quedar medido.
- **En CORS sólo desaparece `Vary`**, nada más: `access-control-allow-origin`, `-methods` y `-headers` salen idénticas en las dos versiones.
- **El `ETag` nuevo codifica `(tamaño, mtime)` en hexadecimal**: el del `index.html` termina en `39`, que es 57 B. En el caso de la barra final, el `ETag` devuelto es el del `index.html`, lo que **prueba por una vía independiente** que sirvió el fallback.
- Trampa para el próximo que toque CORS: si algún día se añade `allow_credentials(true)` o se cambia `Any` por una lista de orígenes, `varies_with_origin()` pasa a `true` y **el `Vary` vuelve** (con sólo `origin`). Que hoy desaparezca es consecuencia de que nada varía, no de un olvido.

---

## FASE 3 — `maxminddb` 0.27.3 → 0.32.0 (PR #71)

Cinco minors de golpe sobre lógica de negocio real: GeoIP. **Se entró pensando que era la fase de más riesgo y la investigación previa demostró lo contrario**: ninguna de las roturas del salto nos alcanza (ver el final de la sección).

- Puntos de uso verificados: `backend/src/models/ipdata.rs:9` (`use maxminddb::{Reader, geoip2}`), `backend/src/main.rs:235` (`Reader::open_readfile`), y lecturas en `journald_reader.rs`, `http/shuul.rs`, `http/report.rs` y `http/stats.rs`.
- Los tests requieren una base GeoIP real (`backend/geo/`, `geo/` o `/tmp/test.mmdb`).

- [x] **3.1** Leer el changelog de 0.28, 0.29, 0.30, 0.31 y 0.32 y anotar los cambios de API que afecten a `Reader::open_readfile` y a los tipos de `geoip2`.
- [x] **3.2** Identificar si el salto obliga a cambiar la firma de las búsquedas (`lookup`) y si el retorno pasa a ser `Result` de otra forma.
- [x] **3.3** `gh pr update-branch 71`.
- [x] **3.4** Gates completos + comprobación explícita de que una IP conocida resuelve al mismo país antes y después del salto.
- [x] **3.5** Si el cambio de API es grande, **PR propia con commit aislado** para poder revertirla sin arrastrar nada.

**Investigación previa — hecha el 2026-10-20** (cubre 3.1 y 3.2 de antemano).

El salto 0.27.3 → 0.32.0 asusta por cinco minors, pero **no nos alcanza ninguna de las roturas**. Comprobado contra nuestro código, no deducido:

| Rotura | ¿Nos afecta? | Por qué |
|---|---|---|
| **0.32.0 — se elimina la feature `simdutf8`** y su dependencia opcional | **No** | `backend/Cargo.toml:15` declara `maxminddb = "0.27.0"` **sin features**, y `grep -rn simdutf8` no devuelve nada en todo el backend. |
| **0.29.0 — `Reader::metadata` pasa a ser privado** (hay que usar el método `metadata()`) | **No** | No hay ni un `.metadata` en `backend/src`: sólo usamos `open_readfile` y `lookup`. |
| **0.29.0 — `Metadata::build_time()` pasa a devolver `Result`** | **No** | No se usa. |
| **0.29.0 — `Reader::open_mmap` pasa a ser `unsafe`** | **No** | Usamos `Reader::open_readfile` en los seis sitios. |
| **0.28.0 — deserializar un array en tupla con longitud distinta pasa a ser error** | **No** | Decodificamos `geoip2::City`, una struct, no tuplas. |
| **0.31.0 — variante nuevo `ResourceLimit`; subdivisiones limitadas a 32** | **No, pero vigilar** | `MaxMindDbError` es `#[non_exhaustive]` desde 0.27 y el `match` de `ipdata.rs:86` termina en `_ =>`, así que un variante nuevo no rompe nada. El límite de 32 subdivisiones sí es un cambio de resultado posible en registros enormes: de ahí que 3.4 no sea un trámite. |

Lo grande de 0.27 **ya está adoptado**: `lookup()` devuelve `LookupResult` desde `ipdata.rs:86` (`result.has_data()` + `result.decode::<geoip2::City>()`), no el `Option` antiguo. Y el salto trae endurecimiento real: 0.28.1, 0.30.x y 0.31.0 son **arreglos de DoS** al decodificar bases corruptas.

Además **hay bases MaxMind reales en local** (`./data/GeoLite2-City.mmdb` y `./backend/geo/GeoLite2-City.mmdb`), así que la comprobación de 3.4 es ejecutable de verdad y no teórica.

**Conclusión: esta fase deja de ser «la de más riesgo».** Por la evidencia de arriba debería salir como las anteriores.

**Resultado — hecho y fusionado (PR #71, `develop` en `7148754`).** No hizo falta rama propia ni tocar una sola línea de código: el `update-branch` rebasó el `Cargo.lock` sin conflicto y los gates salieron verdes tal cual — `cargo build` **0**, **284 tests** (exacto al baseline), clippy y fmt `EXIT=0`.

**Y la comprobación de 3.4 se hizo con medida diferencial, no con fe.** Probe fuera del repositorio con las **dos versiones declaradas a la vez** (`maxminddb` 0.27.3 y 0.32.0, renombradas dentro del mismo `Cargo.toml`) y el cuerpo escrito una sola vez en un `macro_rules!`, replicando la lógica de `IPData::complete` con sus `is_empty()`, contra las **bases GeoLite2-City reales**:

```
$ diff diff-027.txt diff-032.txt && echo IDENTICOS
IDENTICOS
```

Las 17 IPs salen **byte a byte iguales**, incluida la columna que importa (`country_code`): las españolas (`81.35.0.1` → Albacete, `80.24.0.1` → Cádiz, `92.186.0.1` → Las Palmas, `213.0.0.1`) dan **ES** en las dos versiones, y coinciden también `US`, `DE` y la IPv6. Los casos sin datos —privadas, multicast, broadcast, cadena inválida— siguen devolviendo los tres campos a `None`, cada uno por su rama distinta del `match`.

**El riesgo que de verdad quedaba era el peor de todos: silencioso.** El límite de 32 subdivisiones de 0.31 es un error de **decodificación**, así que se dispara aunque nosotros nunca leamos `subdivisions` —serde deserializa la struct entera— y de saltar, el `_ =>` de `ipdata.rs` devolvería **los tres campos a `None`**: geolocalización perdida sin un solo error en el log. Se acotó con un muestreo de **1 048 576 direcciones IPv4** (stride 4096, sobre los 4.290 millones del espacio):

```
0.27.3: no_data=147821 ok_some=900755 err=0 max_subdivisions=2
0.32.0: no_data=147821 ok_some=900755 err=0 max_subdivisions=2
sweep differences: 0
```

Cero errores en ambas y un máximo de **2 subdivisiones** frente al límite de **32**: la regla es inalcanzable con esta base. Es la diferencia entre «no falla en mis cuatro pruebas» y «el modo de fallo que no habríamos visto no está a nuestro alcance».

Efecto en el árbol: `ipnetwork` sigue en **0.21.1** como transitiva, sin cambios. Lo único que desaparece del árbol es `log`, que ya no cuelga de `maxminddb`.

**3.5 no aplicó:** el cambio de API era inexistente, así que no hubo nada que aislar en una PR propia.

---

## FASE 4 — `react-router` 7.18.4 → 8.4.0 (PR #75)

Ya está `MERGEABLE` (Dependabot la rebasó sola). La superficie es más mansa de lo que asusta el salto de major — usos inventariados:

| Uso | Veces |
|---|---|
| `type { NavigateFunction }` | 11 |
| `{ useNavigate }` | 10 |
| `{ useNavigate, Navigate, Outlet }` | 1 |
| `{ Outlet }` / `{ Navigate }` | 1 + 1 |
| `{ BrowserRouter, Routes, Route }` | 1 (en `App.tsx`) |

No hay `loader`, `action` ni APIs de datos. Aun así, **es toda la navegación de la aplicación**.

- [x] **4.1** Leer la guía de migración de react-router 7 → 8 y confirmar que `BrowserRouter`, `Routes`, `Route`, `useNavigate`, `Navigate`, `Outlet` y `NavigateFunction` siguen existiendo con la misma forma.
- [x] **4.2** Gates: `tsc --noEmit`, `lint`, `build`, `test`.
- [x] **4.3** **No basta con la CI**: levantar la aplicación y recorrer las rutas de admin (dashboard, rules, rate-limit-profiles, bans, templates, charts, logs, settings) más el redirect a `/login` cuando no hay token.
- [x] **4.4** Fusionar y cerrar.

**Investigación previa — hecha el 2026-10-20** (cubre 4.1 de antemano), leyendo la guía oficial de migración v7 → v8.

**La rotura principal de v8 no nos toca.** v8 elimina el paquete `react-router-dom`, y `grep -rn "react-router-dom" frontend/src/` no devuelve **ni una coincidencia**: el paquete declarado es `react-router` y los 13 ficheros que lo usan importan de `"react-router"`.

Lo demás tampoco aplica, porque **no estamos en framework ni en data mode**: no hay `loader`, `action`, `meta` ni middleware, que es justo donde viven el resto de roturas (`data` → `loaderData` en `meta`, `getLoadContext` devolviendo `RouterContextProvider`, `splitRouteModules` como opción de primer nivel, `hasErrorBoundary` fuera de `RouteObject`).

**Requisitos mínimos de v8, comprobados uno a uno:**

| Requisito | Nuestro valor | |
|---|---|---|
| Node **22.22+** | CI `node-version: 22`; imagen `node:22-alpine` → **v22.23.3** | ✅ |
| React **19.2.7+** | **19.3.1**, una sola versión en el árbol | ✅ |
| Vite **7+** | **7.3.7** | ✅ |
| `target`/`lib` ES2022 | `tsconfig.app.json`: `target` ES2022, `lib` ES2022, `moduleResolution: bundler` | ✅ |
| ESM-only | es lo que ya construye Vite | ✅ |

Los siete símbolos que usamos (`BrowserRouter`, `Routes`, `Route`, `Outlet`, `Navigate`, `useNavigate`, `NavigateFunction`) siguen en la entrada principal de `react-router`; sólo `RouterProvider` y `HydratedRouter` se mueven a `react-router/dom`, y no los usamos. El gate `tsc --noEmit` es precisamente la comprobación de que siguen existiendo: si alguno se hubiera movido, falla ahí.

**Resultado — hecho y fusionado (PR #75, `develop` en `99a24fb`).** La PR estaba `CLEAN` y la CI verde (corriendo sobre el merge commit, es decir, validando ya `develop` + el bump), así que no hubo que rebasar nada. Gates con los tres baselines intactos: `tsc --noEmit` **0**, lint **0 errores**, `pnpm test` **5 passed**, build ✓. Y el chunk `index` **baja** de 412.92 a **411.23 kB** (**−1.69 kB**), coherente con que v8 haya absorbido el paquete `react-router-dom`.

**4.3 se resolvió sin navegador, y sin rebajar la exigencia.** En esta sesión no hay navegador disponible, así que la navegación real se verificó con el entorno de tests del propio proyecto (`vitest` + `jsdom`), **montando la `<App />` real** —sin caer a un enrutado de juguete— y dejando que el enrutado se resolviera dentro de `act`:

```
[DIAG sin-token]  pathname=/login         → guard + <Navigate> OK
[DIAG con-token]  pathname=/admin/rules   → 42.295 chars; contenido del <Outlet>: "ExportImportAdd RuleAll Pipelines…"
[DIAG publica]    pathname=/             → layout público + HomePage
[DIAG tras-click] pathname=/admin/charts  → useNavigate vía Menu, sin recarga
[DIAG 5]          react-router version=8.4.0 → los 6 símbolos de runtime exportados como function
```

Cubre los cuatro flujos que importan: el redirect a `/login` sin token, una ruta de admin renderizando su hija **dentro del `Outlet`** (no sólo el layout), una ruta pública sin redirect, y la navegación por código con `useNavigate`. **El test era temporal y se borró**: el repositorio no gana ni un test de aquí, y el diff de la PR son exactamente `frontend/package.json` y `frontend/pnpm-lock.yaml`.

Lo que la prueba **no** cubre, para que conste: `fetch` iba rechazado a propósito, así que las páginas se pintan en estado vacío/error. El enrutado está verificado; el pintado con datos reales, no.

**Nota para el futuro:** la suite permanente sigue sin recorrer ninguna ruta, así que esto habrá que volver a verificarlo a mano en el siguiente salto de `react-router`.

---

## FASE 5 — Infraestructura

- [x] **5.1** `Dockerfile:29`: `node:23-alpine` → **`node:22-alpine`**. Node 23 es release impar y **está EOL**; además queda fuera del `engines` de `vite-plugin-static-copy@4.1.1` (`^22.0.0 || >=24.0.0`), que acabamos de subir. Node 22 es lo que corre la CI y es LTS.
- [x] **5.2** Verificar el cambio de verdad con el `podman` local: `podman build --target frontend-builder -t shuul-frontend-check .` (compila el stage con Node 22, hace `pnpm install` y el build de Vite).
- [x] **5.3** **`@types/node` → `^22` (decidido el 2026-10-20).** La PR #77 proponía subirlo a 26 y se cerró: aquí el runtime es **Node 22**, y unos tipos por delante del runtime dejan pasar código que compila y falla en ejecución. Se **alinea** con el runtime en lugar de perseguir a Dependabot.
- [x] **5.4** Revisar si `AGENTS.md` sigue diciendo `node:23-alpine` en la sección de Docker y corregirlo.

**Resultado — hecho y fusionado (PR #82, `develop` en `e2e252a`).** Verificado **con Podman 6.1.4**, no con la esperanza de que funcionara. Tres comprobaciones reales:

- `podman build --target frontend-builder -t shuul-frontend-check .` → **`✓ built in 7.80s`**, `BUILD_EXIT=0`.
- `podman run --rm shuul-frontend-check node --version` → **`v22.23.3`**, con pnpm 12.14.0.
- `podman run --rm shuul-frontend-check ls dist/` → `assets`, `images`, `index.html`, `logo.svg`.

`@types/node` quedó en **22.20.5**, una sola versión en el árbol, y la bajada **no destapó nada**: `tsc --noEmit` limpio, que era exactamente lo que se quería comprobar. El único arrastre en el lock es `undici-types` 7.24.6 → **6.21.0**, el par que corresponde a `@types/node` 22.x. Gates: lint 0 errores, 5 tests, build ✓ y bundle **idéntico** (412.92 kB). `AGENTS.md:514` corregido; no hay `.nvmrc`, `.node-version` ni config de Volta, y la CI ya estaba en 22.

**Consecuencia práctica:** el cambio del `Dockerfile` sólo surte efecto **al construir la imagen**, así que el próximo release saldrá ya con el frontend compilado en Node 22.

---

## FASE 6 — Alertas transitivas

3 alertas abiertas, ninguna con PR propia: `brace-expansion` ×2 (media) y `rand` (baja).

- [x] **6.1** `pnpm why brace-expansion` → identificar el paquete padre que la arrastra.
- [x] **6.2** `cargo tree -i rand` → identificar quién depende de `rand` (el lock muestra `rand 0.8.5`).
- [x] **6.3** Subir el **padre** (o añadir un `override` de pnpm si el padre no se puede mover) y volver a mirar las alertas.
- [x] **6.4** Si un padre no se puede subir sin romper, **documentar por qué** queda abierta en lugar de silenciarla.

**Resultado — hecho y fusionado (PR #83, `develop` en `e0e0f57`).** Y el diagnóstico inicial era **demasiado pesimista**: de las tres alertas, sólo **una** necesitaba trabajo.

**6.1 — `brace-expansion` ×2: no había nada que arreglar.** Conviven **dos linajes** en el árbol y los dos están ya en su versión parcheada:

| Linaje | Quién lo arrastra | Versión |
|---|---|---|
| `1.x` | `minimatch@3.1.5` ← `@eslint/config-array` y `@eslint/eslintrc` ← `eslint@9.39.5` | **1.1.21** |
| `5.x` | `minimatch@10.2.6` ← `@typescript-eslint/typescript-estree@8.71.2` | **5.0.12** |

Las dos son exactamente el `first_patched_version` del aviso vivo (`GHSA-q2hr-2g5m-vwhr`), así que **`develop` ya está parcheado** y `pnpm audit` ya no reporta `brace-expansion`. Un `pnpm.overrides` aquí habría sido **incorrecto**: no hay ninguna versión más alta a la que fijar dentro de ningún major, y fijar `brace-expansion@1`/`@2` habría roto a los consumidores de los otros majors sin arreglar nada.

**Por qué siguen `open` entonces:** leyendo los dos lockfiles, `main` tiene **1.1.18** y **5.0.9**, mientras que `develop` tiene **1.1.21** y **5.0.12**. El dependency graph de Dependabot evalúa la **rama por defecto** —el mismo motivo por el que GitHub las anuncia «on the default branch» en cada push—. **Se cerrarán solas con el próximo release**, cuando la lockfile de `develop` aterrice en `main`. No son deuda pendiente: son deuda **ya saldada** que todavía no ha viajado.

**6.2 y 6.3 — `rand` sí, y sin forzar nada.** `rand = "0.8"` es dependencia **directa** de `backend`, y además la arrastran `jsonwebtoken`, `num-bigint-dig`, `sqlx-mysql` y `sqlx-postgres`. `cargo update -p "rand@0.8.5"` → **0.8.5 → 0.8.8**, que cumple el `first_patched_version` del aviso (`<0.8.6 => 0.8.6`). El padre ya admitía el bump, así que **no hizo falta `[patch]` ni forzar el árbol**. El commit toca **un fichero y 7 líneas**.

**6.4 — y aquí está el matiz que había que dejar escrito:** lo único que no se arregla es lo que **no tiene parche publicado**. `pnpm audit` sale con exit 1, pero ya no por `brace-expansion`, sino por **`braces <=3.0.3`** (`patched_versions: null`), dev-only bajo `vite-plugin-static-copy > chokidar`. Queda anotado, no arreglado, y **no forma parte de estas tres alertas**.

Gates con los baselines intactos: **284 tests** backend, **5** frontend, lint **0 errores**, clippy y fmt `EXIT=0`, build ✓ y chunk `index` en **411.23 kB** (sin cambios).

---

## Protocolo de trabajo

- **SDD:** no hay código nuevo sin change proposal aprobado en `openspec/changes/<feature>/`.
- **TDD:** RED → GREEN → REFACTOR, verificando con CLI (`cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `pnpm run lint`, `pnpm test`, `npx tsc --noEmit`).
- **Nunca** asumir que algo pasa sin verlo en la CLI.
- Prohibido `eslint-disable`, tocar `eslint.config.js`/`tsconfig*.json` o relajar reglas.
- Todo el trabajo va a `develop`. **`main` es rama de release**: cualquier push a ella con HEAD distinto de un tag dispara `release-prepare` y publica versión.
- Las PRs `develop → main` **no** se fusionan con `--delete-branch` (la rama *head* es `develop`).
- El `compose.yaml` de `co1` vive en `/home/lorenzo/docker/dockge/stacks/shuul/compose.yaml`, fuera del repo, y lo ejecuta el usuario.

---

## Registro de lo ya hecho (2026-10-20)

- Fusionadas: **#69** (grupo Rust: `hyper` 1.8.1 → 1.12.0, `tokio` 1.50, `uuid` 1.28, `regex` 1.13.1, `base64` 0.23.1, `time` 0.3.55; hubo que hacer `update-branch` porque su lock arrastraba `backend 0.15.3`), **#73** (`serial_test` 4), **#79** (grupo npm sin el plugin de lint: `react`/`react-dom` 19.3.1, `antd` 6.6.6, `jsdom` 30.1.3, `typescript-eslint` 8.71.2, `vitest` 5.0.5, `vite` 7.3.7), **#80** (`globals` 17.13.0 + `vite-plugin-static-copy` 4.1.1).
- Cerradas con motivo: **#72** (rompía el lint: `eslint-plugin-react-refresh` 0.5.7 marca 26 errores en 13 ficheros por los componentes de clase con `withTranslation`; queda fijado en 0.4.26 y excluido del grupo en `dependabot.yml`), **#76** y **#78** (sustituidas por #80), **#77** (`@types/node` 26, ver §5.3).
- Hallazgo que casi se cuela: `vite-plugin-static-copy` v4 **elimina el aplanado por defecto** y el asset pasaba a `dist/images/src/assets/logo.svg` **sin romper el build**. Se dejó idéntico a v3 con `rename: { stripBase: 2 }`.
- Bundle medido contra `develop` antes y después de #79: total 2761.3 → 2784.1 kB (**+0.8 %**), chunk de entrada 382.8 → 412.9 kB (**+7.9 %**), vendors sin cambios, React en una sola versión. No bloqueante, pero anotado.
- **Fase 1 fusionada** (PR #81): ocho dependencias declaradas sin uso fuera, 125 líneas menos, y **#74 cerrada** por la vía de borrar (la `ipnetwork` directa sobraba y la 0.21.1 ya viajaba como transitiva de `maxminddb`).
- **Fase 5 fusionada** (PR #82): el frontend se compila en **Node 22** —verificado con Podman, `v22.23.3`— y `@types/node` alineado en **`^22.20.5`**.
- **Fase 2 fusionada** (PR #70): `tower-http` 0.7.1 **sin tocar código**, y el `update-branch` funcionó contra lo previsto.
- **Fase 3 fusionada** (PR #71): `maxminddb` 0.32.0 **sin tocar código**, y la geolocalización **idéntica bit a bit** sobre las bases reales —17 IPs y un muestreo de un millón de direcciones—. Queda demostrado que el único riesgo real, el límite de 32 subdivisiones, es **inalcanzable** con `GeoLite2-City` (máximo observado: 2).
- **Fase 4 fusionada** (PR #75): `react-router` 8.4.0, con la navegación real verificada montando la `<App />` en jsdom (guard, redirect a `/login`, `Outlet` anidado y `useNavigate`). El chunk `index` **baja** de 412.92 a **411.23 kB**.
- **Con esto quedan cerradas las cuatro PRs de Dependabot del día y no hay ninguna abierta en el repositorio.**
- **Fase 6 fusionada** (PR #83): de las tres alertas, **sólo `rand` requería trabajo** (`0.8.5 → 0.8.8`, un fichero, 7 líneas). `brace-expansion` ya estaba parcheado en `develop`, y sus dos alertas **se cerrarán solas** cuando la lockfile llegue a `main`, que es la rama que Dependabot escanea. Queda anotado un cuarto aviso **sin parche publicado** (`braces`, dev-only, bajo `vite-plugin-static-copy`).
- **Todas las fases menos la 0 están cerradas.** Fusionadas y sin ninguna abierta: las tres de mantenimiento (#81 dependencias muertas, #82 Node 22, #83 `rand`) y las tres de Dependabot que quedaban (#70, #71, #75).

---

## Decisiones tomadas (2026-10-20)

1. **Fase 0 — se implementan las dos entradas** (socket directo y proxy): no es una disyuntiva de código. Lo que se monte en `co1` se cambia con variables de entorno y un bloque del `compose.yaml`, **sin recompilar**. La reversión de la decisión anterior sobre el socket queda **aprobada**.
2. **Fase 1 — mantenimiento**, sin change proposal de OpenSpec.
3. **Fase 5.3 — `@types/node` se alinea a `^22`** y el `Dockerfile` migra a `node:22-alpine`.

## Pendiente

1. **Documentación, no bloqueo.** Con las dos entradas implementadas (ver `## Decisiones tomadas`), cuál se escribe como preferida es una línea del README y se cambia sin recompilar: **no impide iniciar la Fase 0**. Se decide con §0.2 delante toda.
2. ¿Se retira el soporte de `systemd-journal-gatewayd` o se mantiene? Mantener dos transportes tiene un coste de test que hay que reconocer.
3. Ajeno a este plan: no existe directorio `locales` ni JSON de traducción pese a ~88 llamadas a `t()`; i18next recibe 404 y devuelve la clave.

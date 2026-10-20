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

# Versión de la API del motor (Docker 28.2.2 → v1.49+). Decide si `since` está disponible.
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

- [ ] **1.1** Quitar de `backend/Cargo.toml`: `ipnetwork`, `axum-extra`, `cookie`, `time`.
- [ ] **1.2** Quitar de `frontend/package.json`: `class-variance-authority`, `react-highlight-words`, `@testing-library/react`, `@testing-library/user-event`.
- [ ] **1.3** Regenerar `Cargo.lock` y `pnpm-lock.yaml` (el lock **debe** cambiar: quitar dependencias lo modifica).
- [ ] **1.4** Gates backend: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
- [ ] **1.5** Gates frontend: `pnpm install --frozen-lockfile`, `pnpm run lint` (0 errores), `pnpm run build`, `pnpm test` (5 tests).
- [ ] **1.6** `cargo machete backend` y `npx depcheck` deben salir **sin hallazgos nuevos**.
- [ ] **1.7** Cerrar #74 explicando que se elimina en lugar de subirse.

**Riesgo y cómo se cubre:** `cargo-machete` puede tener falsos positivos (usos vía macro, `build.rs` o features). Aquí se comprobó uno a uno con `grep` que los usos son **cero**, incluso en `tests/`. El gate `cargo test` es la red final: si alguna estuviera usada de forma indirecta, falla ahí y se devuelve.

**Nota de proceso (decidido el 2026-10-20):** va como **mantenimiento**, sin change proposal. No añade ni cambia comportamiento observable —sólo quita dependencias declaradas sin uso— y los gates (`cargo test`, `pnpm test`) son la garantía.

---

## FASE 2 — `tower-http` 0.6.8 → 0.7.1 (PR #70)

Superficie pequeña y localizada: features `cors`, `fs` y `trace`, con `ServeDir` y `ServeFile` en `backend/src/main.rs`.

- [ ] **2.1** `gh pr update-branch 70` (su lock es anterior a los merges de hoy).
- [ ] **2.2** Leer el changelog de `tower-http` 0.7 en busca de cambios de API en `ServeDir`, `ServeFile`, CORS y trace.
- [ ] **2.3** Gates: `cargo test`, `clippy -D warnings`, `fmt --check`. CI verde.
- [ ] **2.4** Fusionar y cerrar.

---

## FASE 3 — `maxminddb` 0.27.3 → 0.32.0 (PR #71)

**La fase de más riesgo de dependencias.** Cinco minors de golpe sobre lógica de negocio real: GeoIP.

- Puntos de uso verificados: `backend/src/models/ipdata.rs:9` (`use maxminddb::{Reader, geoip2}`), `backend/src/main.rs:235` (`Reader::open_readfile`), y lecturas en `journald_reader.rs`, `http/shuul.rs`, `http/report.rs` y `http/stats.rs`.
- Los tests requieren una base GeoIP real (`backend/geo/`, `geo/` o `/tmp/test.mmdb`).

- [ ] **3.1** Leer el changelog de 0.28, 0.29, 0.30, 0.31 y 0.32 y anotar los cambios de API que afecten a `Reader::open_readfile` y a los tipos de `geoip2`.
- [ ] **3.2** Identificar si el salto obliga a cambiar la firma de las búsquedas (`lookup`) y si el retorno pasa a ser `Result` de otra forma.
- [ ] **3.3** `gh pr update-branch 71`.
- [ ] **3.4** Gates completos + comprobación explícita de que una IP conocida resuelve al mismo país antes y después del salto.
- [ ] **3.5** Si el cambio de API es grande, **PR propia con commit aislado** para poder revertirla sin arrastrar nada.

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

- [ ] **4.1** Leer la guía de migración de react-router 7 → 8 y confirmar que `BrowserRouter`, `Routes`, `Route`, `useNavigate`, `Navigate`, `Outlet` y `NavigateFunction` siguen existiendo con la misma forma.
- [ ] **4.2** Gates: `tsc --noEmit`, `lint`, `build`, `test`.
- [ ] **4.3** **No basta con la CI**: levantar la aplicación y recorrer las rutas de admin (dashboard, rules, rate-limit-profiles, bans, templates, charts, logs, settings) más el redirect a `/login` cuando no hay token.
- [ ] **4.4** Fusionar y cerrar.

---

## FASE 5 — Infraestructura

- [ ] **5.1** `Dockerfile:29`: `node:23-alpine` → **`node:22-alpine`**. Node 23 es release impar y **está EOL**; además queda fuera del `engines` de `vite-plugin-static-copy@4.1.1` (`^22.0.0 || >=24.0.0`), que acabamos de subir. Node 22 es lo que corre la CI y es LTS.
- [ ] **5.2** Verificar el cambio de verdad con el `podman` local: `podman build --target frontend-builder -t shuul-frontend-check .` (compila el stage con Node 22, hace `pnpm install` y el build de Vite).
- [ ] **5.3** **`@types/node` → `^22` (decidido el 2026-10-20).** La PR #77 proponía subirlo a 26 y se cerró: aquí el runtime es **Node 22**, y unos tipos por delante del runtime dejan pasar código que compila y falla en ejecución. Se **alinea** con el runtime en lugar de perseguir a Dependabot.
- [ ] **5.4** Revisar si `AGENTS.md` sigue diciendo `node:23-alpine` en la sección de Docker y corregirlo.

---

## FASE 6 — Alertas transitivas

3 alertas abiertas, ninguna con PR propia: `brace-expansion` ×2 (media) y `rand` (baja).

- [ ] **6.1** `pnpm why brace-expansion` → identificar el paquete padre que la arrastra.
- [ ] **6.2** `cargo tree -i rand` → identificar quién depende de `rand` (el lock muestra `rand 0.8.5`).
- [ ] **6.3** Subir el **padre** (o añadir un `override` de pnpm si el padre no se puede mover) y volver a mirar las alertas.
- [ ] **6.4** Si un padre no se puede subir sin romper, **documentar por qué** queda abierta en lugar de silenciarla.

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

---

## Decisiones tomadas (2026-10-20)

1. **Fase 0 — se implementan las dos entradas** (socket directo y proxy): no es una disyuntiva de código. Lo que se monte en `co1` se cambia con variables de entorno y un bloque del `compose.yaml`, **sin recompilar**. La reversión de la decisión anterior sobre el socket queda **aprobada**.
2. **Fase 1 — mantenimiento**, sin change proposal de OpenSpec.
3. **Fase 5.3 — `@types/node` se alinea a `^22`** y el `Dockerfile` migra a `node:22-alpine`.

## Pendiente

1. ¿El transporte Docker pasa a ser el **preferido** y journald queda como alternativa, o al revés? Se puede decidir con §0.2 delante.
2. ¿Se retira el soporte de `systemd-journal-gatewayd` o se mantiene? Mantener dos transportes tiene un coste de test que hay que reconocer.
3. Ajeno a este plan: no existe directorio `locales` ni JSON de traducción pese a ~88 llamadas a `t()`; i18next recibe 404 y devuelve la clave.

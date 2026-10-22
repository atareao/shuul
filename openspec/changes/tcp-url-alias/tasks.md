# Tasks: tcp-url-alias

Ciclo TDD: primero el baseline, después los tests que fallan, y sólo entonces el código.
El proyecto usa OpenSpec (SDD) y TDD estricto.

## 1. Preparación

- [ ] 1.1 Anotar el baseline: `cargo test` en verde con su recuento exacto, `cargo clippy -- -D warnings` sin avisos y `cargo fmt --check` limpio. Verificación: el recuento queda escrito en esta tarea.
- [ ] 1.2 **Reproducir el rechazo antes de tocar código**: comprobar que `tcp://host:2375` produce hoy el diagnóstico `unsupported scheme tcp` y dejar anotado el test que lo fija. Verificación: existe un test que **falla** por esa razón, con el mensaje literal observado.
- [ ] 1.3 Confirmar que el requisito nuevo no choca con los existentes de `openspec/specs/journald-reader/spec.md`: `grep -n '^### Requirement:' openspec/specs/journald-reader/spec.md` no contiene `TCP transport URL scheme`. Verificación: el nombre está libre.

## 2. Tests (RED)

- [ ] 2.1 RED: `SHUUL_DOCKER_URL=tcp://host:2375` produce `DockerTcp` con autoridad `host:2375`, igual que con `http://`. Verificación: `cargo test` falla en aserción.
- [ ] 2.2 RED: `SHUUL_JOURNALD_URL=tcp://host:19531` produce `Tcp` con esa autoridad. Verificación: el test falla en aserción.
- [ ] 2.3 RED: `endpoint_display()` devuelve el esquema **efectivo** (`docker:http://host:2375 (traefik)`) y nunca `tcp`. Verificación: el test falla en aserción.
- [ ] 2.4 RED: `https://` sigue rechazado con un diagnóstico que nombra `https` y que no se confunde con un esquema desconocido. Verificación: el test falla en aserción.
- [ ] 2.5 RED: un esquema desconocido (`ftp://`) se rechaza nombrando el esquema. Verificación: el test falla en aserción.
- [ ] 2.6 RED: una URL sin esquema se rechaza nombrando la variable que la contiene. Verificación: el test falla en aserción.
- [ ] 2.7 RED: un path base se ignora y se registra un warning nombrándolo, con `tcp://` y con `http://`. Verificación: el test falla en aserción.
- [ ] 2.8 RED: el rechazo es recuperable —`run()` reintenta tras `retry_delay` y no devuelve error fatal— para cada uno de los diagnósticos anteriores. Verificación: el test falla en aserción.

## 3. GREEN

- [ ] 3.1 `journald_reader.rs`: el parseo de la URL base acepta `tcp` como sinónimo de `http` en los dos transportes TCP.
- [ ] 3.2 `journald_reader.rs`: `endpoint_display()` normaliza al esquema efectivo en los dos transportes TCP.
- [ ] 3.3 `journald_reader.rs`: conservar el rechazo explícito de `https`, el del esquema desconocido y el de la URL sin esquema, nombrando la causa en cada caso.

## 4. REFACTOR

- [ ] 4.1 `cargo fmt --check` limpio.
- [ ] 4.2 `cargo clippy -- -D warnings` sin avisos.
- [ ] 4.3 `cargo test` en verde; el recuento no baja respecto al baseline y no queda ningún test ignorado.
- [ ] 4.4 Documentación: `SELF-HOSTING.md` (tabla de variables y apartado de modos del backend de Docker) y `AGENTS.md` (apartado de variables de entorno) recogen que `tcp://` es sinónimo de `http://`.

## 5. Verificación manual

- [ ] 5.1 Con un proxy real: arrancar con `SHUUL_DOCKER_URL=tcp://<proxy>:2375` y comprobar que conecta y que la línea de arranque muestra el endpoint como `http://<proxy>:2375`.
- [ ] 5.2 Comprobar que el aviso de fail-open nombra el backend y el endpoint correctos cuando el proxy no está disponible.

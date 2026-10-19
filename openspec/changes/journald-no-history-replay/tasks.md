# Tasks: journald-no-history-replay

## 1. Corte por instante de conexión

- [x] 1.1 RED: test que sirve dos entradas, una con `__REALTIME_TIMESTAMP` anterior al corte y otra posterior, y exige que solo la segunda llegue al pipeline. Verificación: el test falla porque hoy llegan las dos.
- [x] 1.2 GREEN: fijar el corte al inicio de cada intento de conexión y descartar las entradas anteriores comparando `__REALTIME_TIMESTAMP` (µs) con el corte. Verificación: el test de 1.1 pasa.
- [x] 1.3 RED/GREEN: test de entrada sin `__REALTIME_TIMESTAMP` o con valor no numérico → se descarta. Verificación: el test pasa.
- [x] 1.4 RED/GREEN: test de que al reintentar tras un fallo el corte avanza (una entrada generada durante la caída no se procesa). Verificación: el test pasa.

## 2. Dedupe por cursor

- [x] 2.1 RED: test en el que el stream entrega, tras reconectar, la misma entrada que ya se procesó (mismo `__CURSOR`); debe procesarse una sola vez. Verificación: el test falla con el doble conteo actual.
- [x] 2.2 GREEN: descartar la entrada cuyo `__CURSOR` coincide con el último procesado. Verificación: el test de 2.1 pasa.
- [x] 2.3 REFACTOR: `cargo clippy -- -D warnings` y `cargo fmt --check`.

## 3. Documentación

- [x] 3.1 `SELF-HOSTING.md` §2: nota de comportamiento — tras un reinicio el lector empieza en blanco y no cuenta lo ocurrido durante la caída; mencionar que `realtime` y el `skip` de `entries=` no son fiables en este gatewayd. Verificación: la sección lo dice de forma explícita.
- [x] 3.2 `AGENTS.md`: una línea en la descripción del `JournaldReader` con el filtro por `__REALTIME_TIMESTAMP` y el dedupe por `__CURSOR`. Verificación: `grep -n "__REALTIME_TIMESTAMP" AGENTS.md SELF-HOSTING.md`.

## 4. Verificación

- [x] 4.1 `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.
- [ ] 4.2 Comprobar en el host que un reinicio del contenedor no produce un pico de `[BANNED]` ni de `[REGISTERED]` en los primeros segundos. Verificación: comparar el número de eventos del pipeline en el primer minuto tras el reinicio con el tráfico real del journal en ese minuto.

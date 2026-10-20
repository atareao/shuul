# Design

## Context

Ver `proposal.md` — Why. Estado del arte medido contra el gatewayd real
(systemd 249 en el host de producción), porque las suposiciones de la spec
original no se cumplían:

| Petición | Resultado real |
|---|---|
| `?follow` sin `Range` | streamea desde el principio del journal |
| `Range: entries=<cursor>:1` | devuelve **el propio cursor** (el `skip` se ignora) |
| `Range: entries=<cursor>:1:3` | devuelve la entrada siguiente al cursor |
| `Range: entries=<cursor>:0:3` | devuelve desde el cursor |
| `Range: realtime=<t>` (± `until`, s o µs) | **ignorado**; devuelve el journal completo |

El reader ya parsea `__CURSOR` por entrada. `__REALTIME_TIMESTAMP` viene en el
mismo objeto y hoy se ignora.

## Goals / Non-Goals

**Goals:**

- Que un reinicio no pueda desencadenar baneos por tráfico histórico.
- Que una reconexión sea idempotente respecto al último elemento procesado.
- Que el comportamiento no dependa de detalles no garantizados de gatewayd.

**Non-Goals:**

- Reconstruir el estado del rate limiter tras un reinicio (persistir ventanas
  deslizantes en SQLite). Es un cambio aparte y no hace falta para este fallo.
- Reducir el coste de arranque (seguimos leyendo el backlog por la red).
- Cambiar el transporte ni la configuración.

## Decisions

### D1. Filtro en cliente por `__REALTIME_TIMESTAMP`

Es la única opción determinista: `realtime` está roto en este gatewayd y el
`skip` de `entries=` no es fiable sin `num_entries`. El dato ya viaja en cada
entrada, así que no hay coste de parseo extra.

Alternativas descartadas:

- **`Range: realtime=<t>`**: verificado que se ignora con `follow`.
- **`Range: entries=<cursor>:1:N`** (usar `num_entries` para forzar el `skip`):
  limitaría el stream a N entradas y obligaría a reconectar en bucle.
- **Persistir el cursor en SQLite**: no resuelve el problema; al arrancar con un
  cursor viejo se seguiría reproduciendo todo lo intermedio, que es justo lo que
  provoca los baneos. Además el journal es volátil y el cursor puede no existir.

### D2. El corte se fija por intento de conexión, no por proceso

Cada intento de conexión fija su propio corte. Si gatewayd está caído 30 s y
vuelve, esas 30 s no se cuentan. Es deliberado: contar 30 s de fallos de golpe
reproduce el mismo problema a menor escala.

### D3. Dedupe por `__CURSOR`

Barato (comparación de `String` contra el último procesado) y cubre el caso real
del reinicio del stream en la misma entrada, que es lo que hace gatewayd hoy.

### D4. Silencio en los descartes

A nivel `INFO` no se registra nada: en arranque se descartan miles de entradas y
llenar el log no aporta. `DEBUG` sí, para diagnóstico.

## Risks / Trade-offs

- **Se pierden las entradas ocurridas durante una caída del reader** → es
  intencionado y coincide con el plugin anterior. Documentado en
  `SELF-HOSTING.md`.
- **El arranque sigue leyendo el backlog** → unos pocos MB y unos miles de
  parses. Aceptable; lo caro (rate limiter, GeoIP, auditoría) ya no se ejecuta.
- **`__REALTIME_TIMESTAMP` ausente o no numérico** → se trata como entrada sin
  corte conocido y se descarta por precaución, para no arriesgar un baneo.
  Queda cubierto por test.
- **Reloj del host hacia atrás** (ajuste NTP) → podría descartar entradas
  legítimas durante un instante. Impacto despreciable frente al riesgo de baneo.
- **Desfase de reloj (clock skew) en modo TCP** → el corte se toma del reloj **local** y se
  compara con los `__REALTIME_TIMESTAMP` que emite el gatewayd **remoto**. Si el
  gatewayd va atrasado, se descarta tráfico legítimo (el rate limiter deja de
  verlo); si va adelantado, el histórico parece reciente y se reabre el replay
  que el corte evita. Mitigación: NTP sincronizado en ambos hosts y preferir el
  socket Unix —comparten reloj— salvo que el gatewayd remoto sea necesario.

## Migration Plan

1. Desplegar la imagen.
2. Recrear el contenedor.
3. Comprobar en los logs que no aparecen picos de `[BANNED]` al arrancar.
4. Rollback: volver a la imagen anterior. No hay cambios de datos.

## Open Questions

None.

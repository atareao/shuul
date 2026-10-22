# Design

## Context

Ver `proposal.md` — Why. El estado actual relevante, con su ubicación:

- `TraefikAccessLog` (`journald_reader.rs:1525-1550`) deserializa el access log
  JSON de Traefik y `parse_traefik_access_log` (`journald_reader.rs:1584`)
  produce el `ReportPayload`. De los campos que mapea, **cinco** provienen de
  claves de cabecera con prefijo `request_`: `request_User-Agent`,
  `request_Referer`, `request_Content-Type`, `request_Accept-Language` y
  `request_X-Request-Id`.
- `CacheRule::matches` (`rule.rs:175-216`) evalúa los 14 filtros con AND y cada
  uno pasa por el closure `check_match` (`rule.rs:176-186`), cuyo contrato es
  `(Some(regex), Some(value))` → `regex.is_match(value)` y, en cualquier otro
  caso, `true`.
- `NewRequest::from_request` (`new_request.rs:119-159`) resuelve esos cinco
  campos a `None` cuando la cabecera falta o está vacía.
- Medido en producción: transporte activo **API de Docker por socket**
  (`SHUUL_DOCKER_SOCKET=/var/run/docker.sock`), contenedor `traefik`, `tty:false`;
  `traefik.yml` con `accessLog: format: json` sin `fields.headers.names`; 159 de
  159 eventos del Jail con `ua` y `referer` a `null`; **cero** claves que empiecen
  por `request_` en el access log crudo.
- `country_code` **no** procede de una cabecera, sino de la consulta GeoIP, con
  `.filter(|s| !s.is_empty())` → `None` cuando la IP no está en la base o la
  respuesta viene vacía. En el WAF lo resuelve `NewRequest::from_request`
  (`new_request.rs:98-107`); en el Jail lo resuelve `run_jail_pipeline`
  (`report.rs:95-109`) a partir del mismo lookup. Es un campo que **también**
  puede faltar, por una causa distinta a la de los cinco de cabecera.
- Medido en base de datos: 15 reglas (8 WAF, 7 Jail), **0** usan los cinco campos
  de cabecera, pero **2** usan `country_code`: la **4 `Geo blocking`** (WAF,
  `allow=0`, `mode=enforce`, `active=1`, `weight=1000`, y su único filtro es
  `country_code ^(RU|CN|KP|IR|BY|AF|SY|VE|CU|SD|NL|DE|SG|HK|UA|LT)$`) y la **10
  `Scraping por pais`** (JAIL, perfil de rate limit 5, filtro
  `country_code ^(RU|CN|NL|DE|SG|HK)$`). Con `country_code` ausente la 4 deniega
  (403) y la 10 acumula observaciones.
- Medido en la ventana del contenedor actual: de las **2.973 denegaciones** de la
  regla 4, **exactamente 1** traía `country:""` —la IP **`130.12.180.175`**, un
  403 **vivo**—; y **229 peticiones** con `country:""` terminaron `admitted`
  porque otra regla casó antes que la 4. El resto de filtros (`fqdn`, `method`,
  `query`, `protocol`, `city_name`, `country_name`) los usa **0** reglas.
- Specs: `journald-reader/spec.md` con el requisito *Traefik access log mapping*
  (línea 214). **No existe** ninguna capacidad que especifique el matcheo de
  reglas.

## Goals / Non-Goals

**Goals:**

- Que un campo ausente no pueda convertirse en un match silencioso: el fallo
  seguro es no casar sin evidencia.
- Que la ausencia sea **diagnosticable** en el punto donde se decide: el matcher
  nombra el campo y la regla con exactitud (§D3).
- Escribir el contrato de matcheo en una capacidad propia, `rule-matching`, que
  hoy no existe.
- No mover ni un ápice del transporte ni del pipeline.

**Non-Goals:**

- Activar `accessLog.fields.headers.names` en Traefik (vive fuera del repo).
- Normalizar el valor ausente a cadena vacía. Ver §D1.
- Tocar la neutralidad de un campo no restringido. Ver §D2.
- Añadir campos al `ReportPayload`/`NewRequest` o filtros a las reglas.

## Decisions

### D1. Campo ausente ⇒ la regla NO casa

`check_match` pasa a tres brazos explícitos:

```rust
fn check_match(filter: Option<&Regex>, value: Option<&str>) -> bool {
    match (filter, value) {
        (Some(re), Some(v)) => re.is_match(v), // decide el regex
        (Some(_), None) => false,              // antes: true
        (None, _) => true,                     // neutral
    }
}
```

**Rationale.** Casar sin evidencia es el fallo peligroso: en el WAF, una regla de
denegación con filtro de cabecera denegaría **todo** el tráfico sin esa cabecera;
en el Jail, acumularía observaciones contra perfiles de rate limit hasta banear
tráfico legítimo. No casar sin evidencia es el fallo seguro: la regla
sencillamente no se aplica a una petición que no puede satisfacerla, que es lo
que su autor quiso decir. Es la misma lógica que rige el fail-open del lector
hacia el WAF: ante la duda, no castigar.

**La instancia viva del fail-open: el GeoIP.** Este no es un fallo meramente
latente. `country_code` también puede faltar —lo resuelve la consulta GeoIP con
`.filter(|s| !s.is_empty())`—, y hay dos reglas vivas que dependen de él: la 4
`Geo blocking` (WAF) y la 10 `Scraping por pais` (JAIL). Medido en la ventana del
contenedor actual, la semántica actual ya denegó **1** petición con `country:""`
por la regla 4 —la IP **`130.12.180.175`**— y **229** peticiones con `country:""`
sólo se libraron porque otra regla casó antes. El cambio **mejora** ese
comportamiento en lugar de empeorarlo: tras él, esas peticiones dejan de
sobre-matchear (la 4 no deniega y la 10 no acumula observaciones), que es
exactamente lo contrario de lo que hoy ocurre.

**Alternativa descartada: normalizar el valor ausente a cadena vacía.** Permitiría
expresar la ausencia con `^$` en el regex, pero (a) cambia la semántica de los 14
campos a la vez —hoy `None` es ausencia, no cadena vacía—, (b) hace la
«ausencia» **implícita** en un patrón que puede casar por accidente, y (c) no
distingue una cabecera presente y vacía de una ausente, que `from_request` ya
colapsa a `None`. La capacidad «casar por ausencia» no la ha pedido nadie; se
declara fuera de alcance en el `proposal.md` en lugar de introducirla por la
puerta de atrás.

**Consecuencia asumida.** Una regla que hoy casara por ausencia dejará de casar
—es el cambio de comportamiento que persigue el change—. Por eso el diagnóstico
de §D3 es parte de la misma decisión: el cambio es observable y no debe ser mudo.

### D2. La neutralidad de `(None, _)` no se toca

Una regla que **no** restringe un campo sigue siendo neutral en ese campo, traiga
la petición valor o no. Sólo cambia el caso «la regla restringe y el valor
falta».

**Rationale.** La neutralidad es lo que sostiene el 100 % de las reglas
existentes —una regla que sólo filtra `path` no puede dejar de casar porque la
petición no traiga `referer`—. Tocarla cambiaría el significado de todas las
reglas a la vez, y lo haría con un efecto absurdo: un campo **sin filtro** sería
más restrictivo que uno con filtro, porque exigiría la presencia del valor para
no romper el match. Se descartó sin camino intermedio.

### D3. Diagnóstico en el matcher

Cuando `check_match` devuelve `false` **por el brazo `(Some(_), None)`** —y sólo
por él—, se emite un `debug!` que nombra el campo y la regla. Ni cuando el valor
está presente y el regex no casa —ese es un no-match legítimo, información
distinta—, ni cuando la regla no restringe el campo.

**Rationale.** Es el **único punto de decisión**: `matches()` no necesita llevar
estado de qué campos fallaron, ni duplicar el recorrido. El nivel `debug!` es
deliberado: un no-match por ausencia es esperable en régimen normal —cualquier
petición sin `Referer` que roce una regla de `referer`—, así que subirlo a `INFO`
inundaría. Quien esté cazando la trampa lo activa con
`RUST_LOG=backend=debug`.

**Alternativa descartada.** Emitirlo desde `matches()` agregando la lista de
campos ausentes: obliga a `matches()` a acumular estado que hoy no tiene y a
recorrer los 14 filtros dos veces para el mismo veredicto. El punto de decisión
ya sabe qué campo y qué valor faltan; ahí es donde toca decirlo.

**Una señal en el lector, descartada.** Se evaluó añadir en el lector un aviso
—como máximo una vez por intento de conexión— cuando la primera línea mapeada de
un intento no trajera ninguna de las cinco claves `request_*`. Se **retira** del
cambio por tres motivos: (a) es una **heurística** con falso positivo declarado
—una petición legítima sin ninguna de las cinco cabeceras haría avisar aunque la
configuración fuera correcta—; (b) tocaría `backend/src/journald_reader.rs`, un
fichero recién auditado y cerrado por el change `docker-logs-transport`; y (c) es
**redundante** con el diagnóstico del matcher, que es **exacto** porque nombra el
campo y la regla en el único punto de decisión. Ante la duda, el diagnóstico
exacto vence a la heurística: el cambio se queda con un solo diagnóstico, el del
matcher.

### D4. Documentación operativa y una verificación pendiente

Los campos que el origen no puede aportar se documentan: el operador los recupera
declarando las cabeceras en `accessLog.fields.headers.names` de su `traefik.yml`.
Ese fichero vive **fuera** del repositorio y este cambio no lo toca.

Queda anotada como **tarea de verificación** —no como supuesto— confirmar que la
clave que Traefik emite al declarar un nombre coincide **literalmente** con la que
espera el parser (`request_<Nombre>`), porque de ese detalle depende que el mapeo
funcione el día que se active. Si no coincidiera, el mapeo seguiría dando `None`
y el operador no tendría señal: sería la misma trampa, ahora con una cabecera
configurada de más.

**Alternativa descartada.** Aceptar variantes de capitalización en el parser
—alias de `serde` por cada variante de cada clave— **antes** de verificar:
multiplica las claves reconocidas por cuatro sin ninguna evidencia de que haga
falta. Primero se mide; si la medición dice que Traefik no respeta el literal, se
abre su propio cambio.

### D5. Fuera de alcance: el transporte

Este cambio **no** toca el transporte del lector ni la forma de la petición de la
API de Docker; es **ortogonal** a `docker-logs-transport`.

**Rationale.** El campo ausente no depende del transporte: ambos backends
transportan el mismo access log JSON y la disponibilidad de las claves depende de
la configuración de Traefik. Mezclar un cambio de matcheo con uno de transporte
haría imposible atribuir una regresión —el rate limiting bajaría o subiría por
dos causas a la vez— y obligaría a un único archivado para dos funcionalidades
sin relación. `docker-logs-transport` sigue siendo el dueño de cómo se lee; este
change es dueño de qué se hace con lo leído.

## Risks / Trade-offs

- **Cambio de comportamiento del matcher**: una regla de denegación con filtro de
  cabecera que hoy deniega todo pasará a no casar. Es intencionado, pero es un
  cambio observable; el `debug!` de §D3 es lo que evita que sea mudo. **No** es un
  cambio sin efecto vivo: **0** de las 15 reglas actuales usan los cinco campos de
  cabecera (medido), pero **2** usan `country_code`, que también puede faltar —la
  4 `Geo blocking` (WAF) y la 10 `Scraping por pais` (JAIL)—.
- **Beneficio, no regresión, para el país no resoluble.** El comportamiento
  cambia para las peticiones cuyo país no se puede resolver (GeoIP): hoy
  sobre-matchean —la regla 4 deniega y la 10 acumula observaciones— y después
  serán **neutrales** (no casan). Es una **corrección** de un fallo vivo, no una
  regresión.
- **La «casar por ausencia» deja de ser expresable.** No era una capacidad
  soportada ni pedida; se declara no-goal en lugar de introducir el centinela de
  §D1.
- **La clave que emite Traefik no está verificada** (§D4): si Traefik no respeta
  el literal `request_<Nombre>`, activar `fields.headers.names` no arreglaría el
  mapeo. Es tarea de verificación, no un supuesto.
- **Los cinco campos siguen siendo inservibles hasta que el operador toque su
  `traefik.yml`.** Este change hace el fallo **seguro** y **visible**; no
  recupera los campos por sí solo, y no puede: el fichero vive fuera del repo.

## Migration Plan

No hay migración de datos ni de esquema: `CacheRule` compila sus regex al cargar
las reglas y el cambio es de lógica, no de almacenamiento.

1. Desplegar la imagen nueva. Las reglas que no dependen de un campo que pueda
   faltar se comportan igual; las dos que sí dependen de `country_code` —la 4 y
   la 10— dejan de casar con las peticiones cuyo país no se puede resolver, que es
   justamente el arreglo.
2. Verificar con `RUST_LOG=backend=debug` que el matcher registra los descartes
   por campo ausente cuando se prueba una regla con filtro de cabecera.
3. Para recuperar de verdad los cinco campos, el operador añade
   `accessLog.fields.headers.names` a su `traefik.yml` y **confirma** que las
   claves resultantes son las que el parser espera (§D4).
4. Rollback: revertir la imagen. No hay estado que deshacer.

## Open Questions

- **Capitalización de la clave emitida por Traefik** (`request_<Nombre>` frente a
  otras formas). No se resuelve aquí: se verifica en `tasks.md` y, si la
  medición lo exige, se abre un cambio propio para el parser. Escribirlo aquí es
  lo que evita que alguien lo dé por hecho.
  - **Resuelto (2026-10-22).** Medido en un contenedor de usar y tirar con la
    **misma versión que producción** (`traefik:v3.7.5`): config
    `accessLog.format: json` + `fields.headers.defaultMode: drop` + los cinco
    nombres declarados como `keep`, y una petición `GET` con esas cinco cabeceras
    a un path sin router (404 de Traefik). Claves emitidas, literales:
    `request_User-Agent`, `request_Referer`, `request_Content-Type`,
    `request_Accept-Language`, `request_X-Request-Id`. **Coinciden exactamente**
    con las que espera el parser (struct `TraefikAccessLog` de
    `backend/src/journald_reader.rs`), así que la recuperación documentada en
    `SELF-HOSTING.md` §2.15 **funciona**: **no** hace falta ningún alias de
    capitalización y **no** se abre ningún cambio para el parser. Dos
    observaciones del mismo experimento: las cabeceras de **respuesta** usan un
    prefijo distinto y en minúsculas (`downstream_Content-Type`), que no es lo
    que lee el parser; y una respuesta generada por el propio Traefik llega con
    `OriginStatus: 0`, coherente con que el Jail la ignore (ya estaba
    documentado).
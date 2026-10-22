# Proposal: missing-header-fields

## Why

El lector mapea cinco campos del `ReportPayload` desde claves de cabecera que
Traefik emite con prefijo `request_` —`request_User-Agent` → `user_agent`,
`request_Referer` → `referer`, `request_Content-Type` → `content_type`,
`request_Accept-Language` → `accept_language` y `request_X-Request-Id` →
`x_request_id`— y **la fuente configurada no las emite**: el `traefik.yml` de
producción tiene `accessLog: format: json` y nada más, sin bloque
`fields.headers.names`, y el `defaultMode` de Traefik para cabeceras es `drop`.
Medido: en los eventos del Jail (`[REGISTERED]`, `[PENDING]`) `ua` y `referer`
salen `null` en **159 de 159** eventos, y el access log crudo no contiene
**ninguna** clave que empiece por `request_`. Esos cinco campos llegan, pues,
**siempre** `None` al pipeline.

Con campos vacíos, el daño depende del matcher. Y el matcher es **fail-open ante
valor ausente**: `check_match` da por cumplida la condición en cualquier caso que
no sea `(Some(regex), Some(valor))`. Es decir, una regla con filtro de
`user_agent` casa el **100 %** de las peticiones que no traigan esa cabecera. En
el WAF, una regla con `allow=false` y filtro de cabecera denegaría toda petición
sin ella; en el Jail, acumularía observaciones contra perfiles de rate limit y
podría banear tráfico legítimo. Y no es un problema de un solo pipeline: el WAF
construye el `NewRequest` desde las cabeceras vivas del forwardAuth y el Jail
desde el log, así que el fallo es alcanzable en los dos —y en el Jail
**sistemático**, porque la fuente no emite las claves.

El impacto **no** se agota en esos cinco campos, y **no** es sólo una trampa
latente. De las 15 reglas en base de datos (8 WAF, 7 Jail), **0** usan los cinco
campos de cabecera —para ellos la trampa descrita sigue latente—, pero **2**
usan `country_code`, que también puede faltar por otra causa: se resuelve con el
GeoIP y el `.filter(|s| !s.is_empty())` de la consulta lo deja a `None` cuando la
IP no está en la base o la respuesta viene vacía. En el WAF lo resuelve
`NewRequest::from_request` (`backend/src/models/new_request.rs`, líneas ≈98-107);
en el Jail lo resuelve `run_jail_pipeline` (`backend/src/http/report.rs`, líneas
≈95-109) a partir del mismo lookup.

Las dos reglas afectadas son:

- la **4 `Geo blocking`**: WAF, `allow=0`, `mode=enforce`, `active=1`,
  `weight=1000`, y su **único** filtro es
  `country_code ^(RU|CN|KP|IR|BY|AF|SY|VE|CU|SD|NL|DE|SG|HK|UA|LT)$`. Con un
  `country_code` ausente, la semántica actual hace que la regla **case** →
  `allow=0` → **403**;
- la **10 `Scraping por pais`**: JAIL, perfil de rate limit 5, filtro
  `country_code ^(RU|CN|NL|DE|SG|HK)$`. Con un `country_code` ausente, la regla
  casa y acumula observaciones → riesgo de banear tráfico legítimo.

Y **no es hipotético**: medido en la ventana del contenedor actual, de las
**2.973 denegaciones** de la regla 4, **exactamente 1** traía `country:""` —la IP
**`130.12.180.175`**. Es decir: hoy, en producción, una IP que el GeoIP no
clasifica es denegada por una regla que dice «bloquea a estos países». Es un
**403 vivo**, no una trampa latente. Y otras **229 peticiones** con `country:""`
terminaron `admitted`, sólo porque otra regla casó antes que la 4. El resto de
filtros —`fqdn`, `method`, `query`, `protocol`, `city_name` y `country_name`— los
usa **0** reglas, así que el radio de impacto más allá de los cinco campos de
cabecera es exactamente esas dos reglas.

De modo que el fallo no se limita a activarse el día que alguien escriba una
regla sobre `user_agent`, `referer` o `x_request_id`: hay una regla que hoy
deniega y otra que hoy acumula observaciones, ambas sobre un campo que puede
faltar. D1 **arregla un fallo vivo**, no sólo una trampa latente.

## What Changes

- **A. Disponibilidad de campos.** La spec deja de insinuar que los cinco campos
  están siempre y pasa a decir de dónde vienen y quién puede aportarlos: los
  cinco dependen de la configuración del origen (`accessLog.fields.headers.names`
  de Traefik), **no** del transporte, y ninguno de los dos backends del lector
  puede inventarlos. El mapeo SHALL devolver `None` cuando la clave de origen
  falta y **NO SHALL** descartar la línea por ello: un access log sin cabeceras
  sigue siendo un access log válido con su IP, su status, su path y su método.

- **B. Semántica del matcher.** Una regla que restringe un campo y una petición
  que no trae ese valor **NO casan**. Concretamente `check_match` deja de ser
  fail-open en el caso `(Some(_), None)`: sólo `(Some(regex), Some(valor))`
  decide por regex, `(Some(_), None)` es `false` y `(None, _)` sigue siendo
  neutral. Es un requisito **nuevo** (`rule-matching`), porque hasta hoy ninguna
  capacidad especifica el matcheo de reglas.

- **C. Diagnóstico.** Cuando un match se descarta porque la regla restringe un
  campo y la petición no lo trae, el matcher emite un `debug!` que nombra el
  campo y la regla, de modo que la trampa deje de ser silenciosa sin inundar
  `INFO`. El diagnóstico vive en el matcher porque es **exacto**: es el único
  punto de decisión, y nombra el campo y la regla a la vez. **No** se añade
  ninguna señal en el lector (ver la rationale de la parte de diagnóstico en
  `design.md`).

- **Documentación operativa.** Los campos que el origen no puede aportar quedan
  documentados. El `traefik.yml` vive **fuera** del repositorio y este cambio
  **no** lo toca.

No hay **BREAKING changes** de API, ni de esquema de datos, ni de variables de
entorno.

## Capabilities

### New Capabilities

- `rule-matching`: el contrato de matcheo de una regla contra una petición
  —`CacheRule::matches` sobre sus 14 filtros con AND y `check_match` campo a
  campo—, incluida la semántica del campo restringido cuyo valor falta y el
  diagnóstico del match descartado por esa causa.

### Modified Capabilities

- `journald-reader` — **un único** requisito entra en el delta:
  - *Traefik access log mapping* pasa a declarar qué campos puede aportar el
    origen, que los cinco derivados de cabecera SHALL mapearse a `None` cuando la
    clave no está y que la línea NO SHALL descartarse por ello.

## Impact

- `backend/src/models/rule.rs`: `check_match` —el caso `(Some(_), None)` pasa de
  `true` a `false`— y el `debug!` del match descartado por campo ausente. Es el
  **único** punto de decisión: `matches()` sigue siendo el AND de los 14 filtros
  y no cambia su forma. La semántica cambia sobre **los catorce** campos, no sólo
  sobre los cinco de cabecera: cualquier campo restringido cuyo valor falte deja
  de casar. En base de datos, las **dos** reglas vivas afectadas son la 4
  `Geo blocking` (WAF) y la 10 `Scraping por pais` (JAIL), ambas sobre
  `country_code` —resuelto por GeoIP—, además de las que lleguen a escribirse
  sobre los cinco campos de cabecera.
- `backend/src/models/new_request.rs`: sin cambio de comportamiento; queda
  documentado que los cinco campos se resuelven a `None` cuando la cabecera falta
  o está vacía, que es lo que da sentido a la semántica nueva.
- **Beneficio, no regresión.** El comportamiento cambia de forma deliberada para
  las peticiones cuyo país no se puede resolver (GeoIP): hoy sobre-matchean
  —la regla 4 deniega y la 10 acumula observaciones— y después serán **neutrales**
  (no casan). Es una **corrección** de un fallo vivo, no una regresión: se deja de
  castigar tráfico legítimo que no puede satisfacer una regla geográfica.
- `openspec/specs/journald-reader/spec.md` y `openspec/specs/rule-matching/spec.md`
  (nueva): el contrato que este cambio escribe.
- Tests: el requisito nuevo se cubre con el caso del valor presente (casa y no
  casa), el del valor ausente (no casa, con diagnóstico) y el neutral. Baseline a
  respetar: los tests existentes de `parse_traefik_access_log` —incluido *Missing
  optional fields become None*— y los de `matches()` deben seguir en verde.
- **NO se toca**: el pipeline WAF, el pipeline Jail, `run_jail_pipeline`, el
  lector (`backend/src/journald_reader.rs` —su mapeo **no** cambia: ya devolvía
  `None` cuando la clave de origen falta, y ese comportamiento se conserva—), el
  transporte del lector ni la forma de su petición, el esquema SQLite, el
  frontend y el `traefik.yml`. Este cambio es **ortogonal** a
  `docker-logs-transport`.

## Non-goals

- **No se activa** `accessLog.fields.headers.names` en Traefik: el `traefik.yml`
  de producción vive fuera del repositorio y este cambio no lo modifica; sólo
  deja documentada la clave que lo recupera.
- **No se toca el transporte** ni la forma de la petición de la API de Docker. El
  campo ausente depende de la configuración de Traefik, no del transporte.
- **No se normaliza** el valor ausente a cadena vacía (ver `design.md` §D1).
- **No se cambia** la neutralidad de una regla que no restringe un campo (§D2).
- **No se añade** ningún campo nuevo al `ReportPayload` ni al `NewRequest`, ni
  ningún filtro nuevo a las reglas.
- **No se puede expresar** «casar por ausencia de valor»: no es una capacidad
  soportada.
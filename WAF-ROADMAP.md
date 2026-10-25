# WAF-ROADMAP — Llevar el motor del WAF al terreno de Coraza

> **Estado: PROPUESTA. No se escribe ni una línea de código hasta que el usuario apruebe una fase y se abra su change proposal (protocolo SDD del proyecto).**
>
> **Base:** `develop` @ `0e3a25e` (v0.22.0, árbol limpio) · **Fecha:** 2026-10-24
>
> Este documento **no sustituye** a `PLAN.md` (deuda técnica, fases 0–7). Es un plan hermano y vive aparte a propósito.

---

## 0. Resumen ejecutivo

El motor de Shuul hoy **no es un motor de WAF**: es un matcheador de 14 campos contra 14 regex, AND entre ellos, primera regla que casa gana. Eso está bien para lo que hace (filtrar por IP, país, path, user-agent), pero está a años luz de Coraza, y la diferencia **no está en el número de reglas**: está en el modelo.

Coraza evalúa `valor → [transformaciones] → operador` sobre *colecciones* de datos, acumula una *puntuación* y decide al final. Shuul evalúa `campo → regex` y decide en el acto. Todo lo demás (CRS, niveles de paranoia, scoring) se apoya en esa diferencia.

El agujero más grave y más barato de tapar: **Shuul no normaliza la entrada antes de matchear**. Las 24 plantillas WAF son reglas correctas sobre datos que nunca se decodifican, así que un `%20` las esquiva.

---

## 1. Estado de partida (verificado en el código, no supuesto)

### 1.1 El motor

| Pieza | Fichero | Qué hace |
|---|---|---|
| Extracción | `backend/src/models/new_request.rs` | `NewRequest::from_request` lee **9** cosas: proto, host, uri (→ path + query), xff, ua, method, referer, content-type, accept-language, x-request-id. Más GeoIP (ciudad/país/código) y Tor. |
| Compilación | `backend/src/models/rule.rs` | `CacheRule` precompila **un** `Regex` por cada uno de los 14 filtros. |
| Matcheo | `backend/src/models/rule.rs:177` | `matches()` = AND de los 14 `check_match`. |
| Decisión | `backend/src/http/shuul.rs:82` | Bucle `for` con `break`: primera regla que casa gana (`weight ASC`). |

Semántica de `check_match` (`rule.rs:250`): `(None, _)` → casa; `(_, None)` → casa; `(Some(re), Some(v))` → decide el regex. Documentada en `AGENTS.md`.

### 1.2 El esquema

`rules` tiene 14 columnas de filtro, más `weight`, `mode`, `pipeline`, `allow`, `is_tor`, `rate_limit_profile_id`, `active`. **No tiene** `severity`, `tags`, `paranoia_level`, `transformations`, `operator` ni `chain`.

### 1.3 Lo que Coraza tiene y Shuul no

| Capacidad | Coraza | Shuul |
|---|---|---|
| Transformaciones (`t:`) | 38 | **0** |
| Operadores | ~25 (`@rx`, `@pm`, `@detectSQLi`, `@ipMatch`, `@validateByteRange`…) | **1** (`regex`) |
| Colecciones | ~60 (`ARGS`, `REQUEST_HEADERS`, `REQUEST_COOKIES`, `FILES`, `TX`…) | 14 campos planos |
| Fases | 1–5 | 1 (y el Jail, post-factum) |
| Encadenado (`chain`) | Sí, con `TX` y `capture` | AND degenerado de 14 campos |
| Puntuación por anomalía | Sí, por severidad y por PL | No |
| Niveles de paranoia | PL1–PL4 | No |
| Compatibilidad SecLang / CRS | 100 % CRS v4 | No |

### 1.4 Un defecto medido y cerrado: la dirección del cliente la consumen todos

`NewRequest::from_request()` resuelve la dirección del cliente **una sola vez**: toma el último elemento de `X-Forwarded-For` —el mismo del que el `JournaldReader` deriva la dirección del access log—, lo recorta y lo valida como `IpAddr`. Ese valor es el que usan **los tres** consumidores: el campo `ip_address`, la consulta `GeoIP` y la consulta del filtro de Tor. Cuando no hay dirección válida, ninguna de las dos consultas se intenta.

No siempre fue así. Medido en producción el 2026-10-24, con la imagen v0.23.0, una petición con `X-Forwarded-For: 192.0.2.1, 203.0.113.7` dejaba

```
ERROR backend::models::ipdata: Look data for ip: "192.0.2.1, 203.0.113.7": invalid IP address syntax
```

aunque el campo `ip` del registro de auditoría ya salía correcto (`203.0.113.7`). La resolución se había arreglado, pero el `GeoIP` y el Tor seguían recibiendo la cabecera **cruda**, así que con una cadena de saltos los dos caían en fail-open: el `GeoIP` no resolvía —y las reglas por país y por ciudad dejaban de casar— y `is_exit_node` hacía `parse::<IpAddr>().ok()?`, fallaba y dejaba `is_tor` en `None` —y las reglas de Tor dejaban de casar—.

Hoy no se disparaba porque el despliegue mantiene `trustForwardHeader` en `false` y Traefik sustituye la cabecera: llega un solo valor. **Encenderlo es justo lo que introduce cadenas**, así que el orden importa y está escrito: primero la resolución a un solo valor —y que **todos** los consumidores la usen—, después la confianza en la cabecera. El change `client-address-consistency` cierra ese hueco.

---

## 2. Los cuatro agujeros, con evidencia

### 2.1 No hay normalización: las reglas son correctas y aun así se esquivan

`NewRequest::from_request` toma el query de `Uri::query()`. El crate `http` **no decodifica** al parsear. Por tanto el regex ve el porcentaje tal cual.

Ejemplo cierto, verificable en el código sin salir del repositorio:

- Plantilla «SQL injection probes»: `(?i)(…|union\s+select|…)`
- Ataque: `GET /?id=1%20union%20select%201,2,3`
- Valor que llega al regex: `id=1%20union%20select%201,2,3`
- `\s+` **no** casa con `%20` → **la regla no casa → la petición pasa.**

Lo mismo con `'` → `%27`, `<script` → `%3cscript`, `../` → `%2e%2e%2f`, `..%2f`, `%252e%252e%252f`, `....//`, `..\/`, `..%5c`. Ninguna de esas formas está cubierta por las plantillas actuales, y todas son triviales de escribir para un escáner.

**Conclusión: el WAF tiene reglas de CRS pero sin la capa de transformaciones de CRS, y esa capa es la que hace que las reglas sirvan.**

### 2.2 Se tiran colecciones que ya llegan por el cable

`forwardAuth` de Traefik pasa **todas** las cabeceras de la petición al servidor de autenticación (`authRequestHeaders` vacío = todas). `NewRequest::from_request` recibe el `HeaderMap` completo y conserva **cinco**: `user-agent`, `referer`, `content-type`, `accept-language`, `x-request-id`.

Lo que se descarta y es oro:

- **`Cookie`** — vector de ataque de primer orden. Shuul no puede ver una cookie. Llega y se tira.
- **Cualquier cabecera con nombre arbitrario** — `Authorization`, `X-Api-Key`, `X-Forwarded-For` secundario, `Origin`, `Range`… inalcanzables.
- **Los argumentos del query, uno a uno** — el query se guarda como una cadena opaca, así que ninguna regla puede decir «el parámetro que se llama `id` contiene SQLi». Solo se puede matchear el blob entero.
- **El nombre base y la extensión del path** — obliga a escribir regex de path más frágiles.

**Estado (F2b-2a, ✅ entregada):** las cookies, las cabeceras con nombre, los argumentos del query uno a uno y el nombre base y la extensión del path **ya** se extraen del request y se pueden restringir como colecciones (ver §F2b-2a). Queda una limitación declarada: las colecciones con clave —cookies y cabeceras— sólo están disponibles en el WAF, porque el payload del Jail no trae el mapa de cabeceras.

### 2.3 Los operadores son todos `regex`

- **Rangos de IP**: se escriben como regex. `192\.168\.\d+\.\d+` casa con `192.168.999.999`. Coraza usa `@ipMatch` con CIDR real. El crate `ipnetwork` **ya está en el árbol** como dependencia transitiva de `maxminddb` (y `PLAN.md` FASE 1 proponía retirarlo por muerto: adoptar `@ipMatch` le da un uso real).
- **Listas de palabras**: plantillas como «User-Agent bots conocidos» son una alternación gigante `(?i)(curl|wget|python-requests|…)`. Coraza usa `@pm` (Aho-Corasick), más rápido y más legible.
- **Faltan por completo**: `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within`, `@validateByteRange`, `@validateUrlEncoding`, `@validateUtf8Encoding`, y las comparaciones numéricas `@gt/@ge/@lt/@le` (que servirían para `Content-Length`, número de argumentos, número de cabeceras).

### 2.4 La decisión es binaria y la severidad se tira a la basura

Hoy: primera regla que casa gana → allow o deny. No hay acumulación.

Y hay una brecha verificada: `RuleTemplate` (`backend/src/templates.rs:6`) **declara** `severity`, y la página de plantillas del frontend la usa para filtrar y pintar etiquetas (`frontend/src/pages/admin/templates_page.tsx`). Pero:

- la tabla `rules` **no tiene columna `severity`** (ninguna migración la crea);
- el handler de importación **no la lee** (`grep severity` en `backend/src/http/` no devuelve nada).

**La severidad de una plantilla se pierde en el momento de importarla.** Es información que el proyecto ya tiene y que no persiste. Sin severidad no hay puntuación por anomalía, y sin puntuación no hay niveles de paranoia ni nada que se parezca al modelo de CRS.

---

## 3. El techo arquitectónico

`forwardAuth` es un middleware de **petición**: Traefik manda método, URI y cabeceras, y decide por el código de estado que devuelve el servidor de autenticación. Consecuencias duras:

| Fase | Contenido | ¿Alcanzable con ForwardAuth? |
|---|---|---|
| 1 | Cabeceras de petición | **Sí**, hoy |
| 2 | Cuerpo de la petición | **Sí, desde Traefik v3.3** — ver abajo |
| 3 | Cabeceras de respuesta | **No** |
| 4 | Cuerpo de la respuesta | **No** |
| 5 | Registro | Sí (LogCollector) |

La respuesta del backend **nunca pasa por Shuul**. Las reglas de fuga de datos de CRS (ficheros 95x: errores de PHP/Java/SQL, tarjetas, identificadores) son **estructuralmente inalcanzables** mientras Shuul sea un ForwardAuth. Solo se alcanzarían si Shuul pasase a ser un proxy inverso, que es otro producto.

### La puerta que sí existe: `forwardBody`

Traefik ≥ 3.3 tiene `forwardAuth.forwardBody` (por defecto `false`). En producción se corre **`traefik:v3.7.16`**, así que está disponible.

- A favor: desbloquea `ARGS_POST`, `REQUEST_BODY`, `FILES_NAMES`, `FILES_SIZES` — es decir, la fase 2 y buena parte del CRS de entrada.
- En contra, y hay que decirlo claro: *«As body is read inside Traefik before forwarding, this breaks streaming»*. Traefik bufferiza el cuerpo entero en memoria. Eso rompe SSE, upgrades a WebSocket y subidas en streaming **para todos los routers que lo activen**.
- Obligatorio acompañarlo de `maxBodySize`: sin él Traefik emite un aviso de agotamiento de memoria y el cuerpo queda sin límite.

**Diseño correcto: opt-in por router**, no global. Quien quiera inspección de cuerpo lo añade a su middleware; quien no, no paga nada. Shuul debe funcionar igual con cuerpo y sin él.

**Y el orden importa**: la inspección de cuerpo **sin transformaciones no sirve de nada**. Un `%27` en un campo de formulario esquiva igual que en un query. Por eso el cuerpo va después de la normalización, no antes.

---

## 4. «Usa Coraza» no es una opción real en este stack

Conviene descartarlo con datos, porque es la respuesta obvia:

| Vía | Veredicto | Evidencia |
|---|---|---|
| Plugin WASM de Coraza para Traefik | **No** | ~**29 req/s** frente a ~683 del nativo (23× más lento). El propio repo se declara *experimental* y *«needs a maintainer»* |
| Coraza nativo en Traefik | **No** | Es **Traefik Hub**, la edición comercial |
| Coraza como sidecar Go + Shuul como plano de control | **Posible, caro** | Metería un runtime Go en la pila y dejaría el motor Rust redundante. Solo tendría sentido si el objetivo fuese importar CRS al 100 % |
| Motor propio en Rust, en el mismo proceso que su BD de reglas | **Sí** | Sin salto de red, sin runtime extra, con la UI y el banco de pruebas que ya existen |

Conclusión: **el nicho de Shuul justifica motor propio**. Lo que hay que copiar de Coraza es el **modelo**, no el binario.

---

## 5. Plan por fases

Cada fase es **un change de OpenSpec independiente y publicable por sí solo**. Ninguna toca el pipeline Jail. Ninguna cambia el comportamiento por defecto de las reglas existentes.

### F1 — Transformaciones (`t:`) · tamaño M · la de mayor valor por esfuerzo · ✅ entregada

> **✅ Entregada** — change `waf-transformations`, archivado en `openspec/changes/archive/` cuando se cierre.
>
> **Lo entregado:**
>
> - La columna `transformations` en `rules`.
> - Las 18 transformaciones y la pipeline, con `parse` / `apply` / `is_empty`.
> - La aplicación al valor de la petición en `check_match`, con la evidencia sacada del **mismo** recorrido que decide el matcheo.
> - La validación y la canonización de la especificación en las **tres** vías de escritura (crear, editar e importar).
> - La exclusión de la caché de reglas activas, con un `error!` que nombra la fila irresoluble y el token.
> - La clave `"matched"` en el evento de auditoría, con el valor transformado que casó.
> - Los chips de los 18 nombres canónicos en la pestaña «Request» del diálogo de regla.
> - Las cuatro plantillas de detección con su pipeline y su severidad.
>
> **Desviaciones del boceto, cada una con su porqué:**
>
> - **Los 18 decodificadores se escribieron a mano, sin dependencias nuevas.** El crate `urlencoding`, que ya estaba en el proyecto, **no** sirve: convierte `+` en espacio, y la misma transformación se aplica al `path`, donde `+` es un carácter literal. El de Base64 tampoco se añadió: la semántica que hace falta —estricta, y ante cualquier duda dejar el valor intacto— es más simple de escribir que de configurar.
> - **Solo subieron tres severidades de las cuatro:** `SQL injection probes` y `Command injection probes` a `critico`, `XSS probes` a `alto`; `Path traversal probes` se queda en `alto`. La cuarta ya estaba en su sitio, y subirla habría sido mover un número para cumplir la tarea.
> - **No se tocó ningún regex de las cuatro plantillas.** La pipeline decodifica **una sola vez** —no hay punto fijo—, así que las alternativas literales `%0a`, `%0d` y `%00` del regex de `Command injection probes` siguen siendo alcanzables: un payload doblemente codificado (`%250a`, `%2500`) emerge de `urlDecodeUni` como el literal y casa la alternativa. Quitarlas habría **debilitado** la detección.
> - **La UI ofrece los 18 nombres como chips, no como campo de texto con una lista al lado.** El orden importa y un desplegable de selección múltiple lo perdería; con chips el camino común no puede producir un token no canónico, y el parser estricto solo lo ve quien escribe contra la API a mano.
> - **La especificación se canoniza al escribir.** `urlDecodeUni,none,lowercase` queda almacenado como `lowercase`.
> - **La evidencia se emite solo con la pipeline no vacía Y al menos un campo de los 14 restringido.** Con la pipeline vacía el valor transformado es el crudo, que el evento ya lleva; y con una pipeline que no restrinja ningún campo —solo `is_tor`, o solo `allow`— no hay nada que anotar.
> - **`htmlEntityDecode` exige el `;` final: una limitación conocida y deliberada.** HTML5 reconoce la forma sin punto y coma (`&#39`) y esta implementación no, así que `&#39` se deja intacto y `&#39;` se decodifica. Si algún día se quiere conformidad plena, es un cambio de comportamiento que hay que decidir, no un descuido.
> - **La evidencia también es `None` cuando la regla no casa.** La cadena de comparaciones cortocircuita con `&&`, así que sin ese corte la evidencia describiría campos de una decisión que no se tomó.
>
> **Lo medido:** el `path` y la `query` de `X-Forwarded-Uri` llegan **codificados**; el backend queda en **503 pruebas verdes** y el frontend en **92**.

**Por qué primero:** es la que convierte las 24 plantillas actuales en reglas que de verdad bloquean. Todo lo demás se apoya en ella.

**Contrato:**

```sql
ALTER TABLE rules ADD COLUMN transformations TEXT NOT NULL DEFAULT '';
```

```rust
/// Una función de transformación de la pipeline `t:`.
pub enum Transformation {
    UrlDecode, UrlDecodeUni, Lowercase, Uppercase,
    RemoveNulls, ReplaceNulls, CompressWhitespace, RemoveWhitespace,
    NormalizePath, NormalizePathWin, HtmlEntityDecode, JsDecode,
    CssDecode, EscapeSeqDecode, Base64Decode, CmdLine, Trim, Length,
}

pub struct TransformationPipeline(Vec<Transformation>);

impl TransformationPipeline {
    /// Parsea `"urlDecodeUni,lowercase"`. Cadena vacía → pipeline vacía.
    pub fn parse(spec: &str) -> Result<Self, TransformError>;
    /// Devuelve `Cow::Borrowed` si la pipeline está vacía: coste cero.
    pub fn apply<'a>(&self, value: &'a str) -> Cow<'a, str>;
}
```

**Reglas de comportamiento:**

- Pipeline vacía → **el valor no se toca**. Las reglas existentes se comportan byte a byte igual que hoy. Esta es la garantía que protege las 384 pruebas del backend y las 87 del frontend.
- `t:none` limpia la pipeline (equivalente a vacía). Recomendado empezar siempre por él, como hace Coraza.
- Transformación desconocida → **la regla no carga y se reporta**; nunca se ignora en silencio. Precedente: el `debug!` que nombra campo y regla que se añadió en `missing-header-fields`.
- El registro de auditoría debe anotar **el valor transformado que casó**, no solo el crudo. Sin eso, afinar el WAF es a ciegas.
- La pipeline se aplica a los 14 filtros de la regla. Limitación conocida y aceptada: una regla que filtre `path` y `query` a la vez recibe las mismas transformaciones en ambos. Se resuelve en F2, donde cada condición tendrá las suyas.

**Subconjunto recomendado** (18 de las 38 de Coraza, las que tienen sentido sin cuerpo): las de la lista de arriba. Se dejan fuera deliberadamente las de paridad, `md5`/`sha1`, y `base64Encode`/`urlEncode`/`hexEncode`, que no sirven para detección.

**Tareas:**

1. Verificar en un test si `X-Forwarded-Uri` llega codificado o decodificado (afecta a la plantilla de path traversal; el caso del query ya está probado por construcción).
2. `TransformationPipeline` con sus pruebas unitarias, una por transformación, con casos de evasión reales.
3. Migración + `CacheRule` compilando la pipeline una sola vez.
4. Aplicarla en `matches()`.
5. UI: campo de texto en la pestaña «Request» del `RuleDialog`, con ayuda de las transformaciones disponibles.
6. Actualizar las plantillas que lo necesiten (`Path traversal`, `SQL injection probes`, `XSS probes`, `Command injection probes`) y **subir su `severity`** de paso.
7. Documentar en `USER-GUIDE.md` y `SELF-HOSTING.md`.

**Gates:**

- Las 42 plantillas con pipeline vacía matchean exactamente igual que antes (prueba de caracterización).
- Regla con `t:urlDecodeUni` y patrón `\.\./` casa con `%2e%2e%2f`.
- Regla con `t:urlDecodeUni` y patrón `union\s+select` casa con `1%20union%20select%201`.
- `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios; `tsc -b --force` y `npm run lint` limpios; cero `eslint-disable`, cero `any`, cero `@ts-ignore`.
- **Resultado real de F1:** backend **503** verdes y frontend **92**; `cargo clippy --lib --bins` en **0** avisos, `cargo clippy --all-targets` en **83** (la base preexistente, sin añadidos); `cargo fmt --check`, `tsc -b --force` y `npm run lint` limpios.

### F2 — Condiciones 1:N: colecciones, operadores y lógica · tamaño L · la fase estructural

Es una sola fase porque las tres cosas viven en el mismo sitio: una condición.

```sql
CREATE TABLE rule_conditions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id         INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    position        INTEGER NOT NULL,
    collection      TEXT    NOT NULL,
    key             TEXT,
    transformations TEXT    NOT NULL DEFAULT '',
    operator        TEXT    NOT NULL DEFAULT '@rx',
    pattern         TEXT    NOT NULL,
    negate          INTEGER NOT NULL DEFAULT 0,
    UNIQUE(rule_id, position)
);
```

**Colecciones alcanzables sin cuerpo:**

`REQUEST_URI_RAW`, `REQUEST_URI`, `REQUEST_BASENAME`, `REQUEST_FILENAME`, `QUERY_STRING`, `ARGS`, `ARGS_GET`, `ARGS_GET_NAMES`, `REQUEST_HEADERS`, `REQUEST_HEADERS_NAMES`, `REQUEST_COOKIES`, `REQUEST_COOKIES_NAMES`, `REQUEST_METHOD`, `REQUEST_PROTOCOL`, `REMOTE_ADDR`, `REMOTE_HOST`, `GEO_COUNTRY_CODE`, `GEO_CITY`, `GEO_COUNTRY_NAME`, `IS_TOR`.

**`is_tor`: el caso especial que entra en esta fase.**

`IS_TOR` está en la lista, pero no es una colección como las demás. Hoy el matcheo lo trata como un **predicado booleano al margen del conjunto de condiciones** (`check_tor` en `CacheRule::evaluate()`, encadenado al final de la conjunción). Es la última cosa que el motor no sabe expresar como condición, y por eso **F2a la deja fuera**: con `Operator` reducido a `@rx` no hay forma de comparar un booleano que no sea inventarle una representación de cadena, que es andamiaje muerto. La reentrada es escalonada, y cada peldaño cae en su sub-entrega:

| Sub-entrega | Qué añade | Sin ella, `is_tor` no puede… |
|---|---|---|
| **F2b** | `IS_TOR` en el vocabulario de colecciones y su proyección desde la columna `is_tor` — **✅ entregada en F2b-3** | representarse y compararse |
| **F2c** | los operadores nuevos (`@ipMatch` con CIDR, `@pm` con Aho-Corasick, los de cadena y los de validación) | — (el operador booleano `@truthy` llegó ya en F2b-3) |
| **F2d** | `negate` por condición y `condition_operator` | expresar «sólo Tor» — hoy el interruptor es de una sola dirección |

**La regla de proyección es distinta a la de los 14 campos, y es la parte que se rompe si se generaliza sin pensar.** `is_tor = true` proyecta a **una** condición `is_tor`/`@truthy`, **al final** del conjunto. `is_tor = false` e `is_tor = NULL` proyectan a **ninguna** condición: son **neutrales**, igual que hoy —la *closure* sólo decide con `Some(true)` y deja pasar cualquier otro valor—. Proyectar el `false` a una condición **invertiría el sentido de la regla** y bloquearía todo el tráfico que **no** venga de Tor. El error no se vería en una prueba escrita con `is_tor = true`: hay que cubrir explícitamente el caso `false`.

**Invariantes que se conservan, idénticos a los de los 14 campos.**

- **La colección ausente no casa.** `TorService::is_exit_node()` devuelve `None` mientras la lista no ha cargado —los primeros 30 s tras el arranque— y también cuando la dirección del cliente no parsea como IP. El brazo es el mismo que el de un campo restringido que la petición no trae: la condición no se cumple y se emite su `debug!` nombrando la regla y la colección. Es un fail-open deliberado, hermano del que ya tiene el transporte del Jail.
- **La colección no se transforma.** `IS_TOR` no es una cadena: su pipeline es, por contrato, **vacía**, y una pipeline sobre ella no debe poder declararse.
- **La evidencia es una decisión, no un accidente.** `check_tor` **no** anotaba nada en el evento de auditoría, y convertirlo en condición abría la puerta a un `{"is_tor": "true"}` que no existía. **Decidido en F2b-3: no se anota.** La forma del evento no cambia —es un cambio observable cero—, y la decisión es explícita, no el arrastre de la generalización.

**Entregada en F2b-3.** `is_tor` es ya una colección más del conjunto (`Collection::IsTor`, sin clave, fuera de `Collection::ALL`) y el predicado `check_tor` y el campo `CacheRule.is_tor` se han **retirado**: el chequeo de Tor es una condición `is_tor`/`@truthy` como cualquier otra, evaluada en AND con el resto. En modo simple la columna `is_tor` se proyecta a esa condición y el comportamiento es idéntico al de antes; en modo avanzado la columna deja de participar fuera del conjunto, conforme al requisito de `rule-matching`.

**Operadores:** `@rx`, `@truthy` (booleano, sólo para `is_tor`), `@pm`, `@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within`, `@ipMatch`, `@ipMatchF`, `@gt`, `@ge`, `@lt`, `@le`, `@validateByteRange`, `@validateUrlEncoding`, `@validateUtf8Encoding`, `@unconditionalMatch`.

**Compatibilidad — lo crítico:** los 14 campos actuales **no se retiran**. La regla se puede escribir en modo `simple` (las 14 columnas, lo que la UI muestra hoy) o `advanced` (la tabla de condiciones). Al cargar, **ambas formas se proyectan a la misma representación interna** `Vec<Condition>`, y el pipeline del WAF solo conoce esa. Resultado: **un único camino de matcheo**, `first match wins` intacto, y las plantillas y la UI actuales siguen funcionando sin tocarlas.

**Sub-entregas, cada una publicable:**

- **F2a** — IR `Condition` y proyección de compatibilidad desde los 14 campos, **en memoria**. **Sin tabla y sin migración**: la fuente de verdad siguen siendo las 14 columnas y la tabla llega en F2b. Sin cambio funcional visible. **✅ Entregada.** `Collection`, `Operator::Rx` y `Condition` con `project()` viven en `backend/src/models/condition.rs`; `CacheRule` pasa de 14 `Option<Regex>` más una pipeline propia a un `Vec<Condition>`; `evaluate()` recorre el conjunto en AND con cortocircuito y `check_tor` sigue **fuera** del conjunto. Resultado real de los gates: backend **535** verdes / **0** rojos / 1 ignorado; `cargo clippy --lib --bins` en **0** y `--all-targets` en **83** (la base, sin añadidos); `cargo fmt --check` limpio. Cero migración, cero cambio de oferta de API y cero cambio en la forma del evento.
- **F2b** — se subdivide en tres entregas publicables:
  - **F2b-1** — la tabla `rule_conditions` con su backfill desde los 14 campos; la tabla pasa a ser la **fuente persistida** del conjunto de condiciones y las 14 columnas de `rules` se conservan como representación simple. Sin cambio observable. **✅ Entregada.**
  - **F2b-2** — las colecciones nuevas (cookies, cabeceras con nombre, argumentos del query uno a uno, basename/extensión) y su extracción del request. **Subdividida en dos entregas publicables:**
    - **F2b-2a** — la representación de las colecciones con clave y la extracción del request, sin editor de UI. **✅ Entregada.** El vocabulario de las cinco colecciones nuevas (`request_cookies`, `request_headers` y `args_get` con clave; `request_basename` y `request_extension` sin clave), su extracción del request, la clave en la condición con su evidencia compuesta, la persistencia de `key` en `rule_conditions`, la validación de la autoría avanzada por API (`conditions`) y la representación de la regla con condiciones (por lote en listado y exportación). **Lo entregado, en detalle:** (1) **extracción** — `NewRequest` gana `cookies`, `headers` (nombre en minúsculas), `query_args` (crudos, uno a uno) y `basename`/`extension`; `derive_path_collections()` los deriva del path y del query y lo comparten el WAF y el Jail; (2) **matcheo** — `request_value(request, key)` resuelve por nombre las tres colecciones con clave y la evidencia usa la clave compuesta `colección[clave]`; (3) **persistencia y autoría avanzada** — la columna `key` se escribe y se lee, `conditions` en `NewRule`/`UpdateRule` decide el modo (ausente = simple, presente = avanzado), `ConditionInput::compile()` valida cada condición con rechazo 400, y la importación persiste `is_tor` y rechaza por entradas; (4) **rollback** — volver a una imagen anterior descarta las condiciones irresolubles con diagnóstico en vez de aplicarlas mal. **Limitación documentada:** las colecciones con clave (cookies y cabeceras) son del **WAF**; el payload del Jail no trae el mapa de cabeceras, así que `request_basename`, `request_extension` y los argumentos del query sí valen en ambos pipelines pero las tres con clave no. **Sin migración:** la columna `key` ya existía desde F2b-1. **Lo medido:** backend **587** verdes / **0** rojos / 1 ignorado; integración `rule_conditions_test` **22/22**; `cargo clippy --lib --bins` en **0** y `--all-targets` en **83** (la base); `cargo fmt --check` limpio.
    - **F2b-2b** — el editor de condiciones del `RuleDialog` (la UI de la autoría avanzada). **✅ Entregada.** El diálogo de regla gana una pestaña **Conditions** con un conmutador simple/avanzado: en **simple** omite `conditions` (el backend proyecta los 14 campos), como hasta ahora; en **avanzado** autoriza una lista ordenada de condiciones —colección, clave (solo en las colecciones que la exigen), transformaciones por chips canónicos, patrón y el operador `@rx` fijo— y la envía siempre, aunque esté vacía. En avanzado las pestañas de los 14 campos (Network, Location, Request) quedan deshabilitadas con una nota de que se almacenan pero no participan del matcheo. Pasar de simple a avanzado **siembra** la lista proyectando los 14 campos rellenos en orden canónico (entonces `is_tor` no se proyectaba; F2b-3 lo añadió al final); volver a simple pide confirmación y descarta la lista. Una condición de una colección con clave sin clave impide guardar y señala la fila. Al editar, el modo se **infiere** comparando las condiciones recibidas con la proyección de los 14 campos: coinciden → simple; difieren → avanzado con la lista sembrada con lo recibido, así que la inferencia es sin pérdida. El editor se extrajo a `ConditionEditor` (+ `condition_editor_model.ts` y `rule_transformations.ts`) para no engordar el diálogo. **Lo medido:** frontend **113** verdes (13 ficheros), `npx tsc -b` y `npx eslint .` en 0; backend intacto (`cargo check` en 0).
  - **F2b-3** — la representación de `IS_TOR` como colección booleana y el operador `@truthy` que la compara. **✅ Entregada.** `Collection::IsTor` entra en el vocabulario con `field_name = "is_tor"`, sin clave y **fuera** de `Collection::ALL` (que siguen siendo las 14 de campo): el catálogo alcanzable por la autoría avanzada queda en **20** colecciones. El operador `@truthy` es la segunda variante de `Operator`: la condición `is_tor`/`@truthy` se cumple **si y solo si** el booleano de la petición es exactamente `true`; `Some(false)` y ausente (`None`) **no** cumplen —fail-open— y emiten el `debug!` que nombra la colección y la regla. Su pipeline es **vacía** por contrato y **no** anota evidencia: la forma del evento de auditoría no cambia. **Proyección y matcheo:** `project()` añade **al final** la condición `is_tor`/`@truthy` cuando la regla tiene `is_tor == Some(true)`; `Some(false)` y `None` no proyectan nada. Se **retiraron** el predicado `check_tor` de `CacheRule::evaluate` y el campo `CacheRule.is_tor`: el chequeo de Tor es ahora **una condición más** del conjunto. **Persistencia:** la columna `operator` de `rule_conditions` ya no está fijada a `@rx`; se escribe y se lee `@rx` o `@truthy`, y una fila incoherente se descarta con diagnóstico. **Validación:** `@truthy` se acepta solo para `is_tor` (patrón vacío, sin clave y sin transformaciones); `@truthy` fuera de `is_tor` y `@rx` sobre `is_tor` son 400. **Migración:** `20261026000000_project_is_tor_into_rule_conditions.up.sql`, backfill idempotente (`WHERE NOT EXISTS`) que da a las reglas con `is_tor = 1` su fila `is_tor`/`@truthy`. **Frontend:** `is_tor` en el catálogo del editor, con operador fijo `@truthy` y sin patrón/clave/transformaciones; `projectConditions` incluye la condición de `is_tor`. **Cambio observable:** ninguno en modo simple; en modo avanzado la columna `is_tor` deja de participar fuera del conjunto. Resultado real de los gates: backend **620** verdes / **0** rojos / 1 ignorado; `cargo clippy --lib --bins -- -D warnings` en **0**; `cargo clippy --all-targets` en **82** (base 83, sin avisos nuevos); `cargo fmt --check` en **0**; frontend **118** verdes; `npx tsc -b` en **0**; `npx eslint .` en **0**.
- **F2c** — operadores nuevos: `@ipMatch` con CIDR, `@pm` con Aho-Corasick, los de cadena y los de validación. **Pendiente** (el operador booleano que le daba sentido a `IS_TOR` llegó ya en F2b-3 como `@truthy`).
- **F2d** — lógica booleana: `condition_operator` (`and` | `or`) en la regla y `negate` por condición. **Pendiente.** El encadenado completo de Coraza con `capture` y variables `TX` **queda fuera** de esta fase.

**Gates:** proyección de compatibilidad probada contra las 42 plantillas; cada operador nuevo con su prueba de contrato; `@ipMatch` rechaza `192.168.999.999` donde el regex actual lo acepta.

### F3 — Severidad, etiquetas y puntuación por anomalía · tamaño M

```sql
ALTER TABLE rules ADD COLUMN severity       TEXT    NOT NULL DEFAULT 'warning';
ALTER TABLE rules ADD COLUMN tags           TEXT    NOT NULL DEFAULT '';
ALTER TABLE rules ADD COLUMN paranoia_level INTEGER NOT NULL DEFAULT 1;
```

- **Cierra la brecha verificada** del apartado 2.4: la importación de plantillas deja de tirar la severidad.
- Mapa de severidad a puntos, con los valores por defecto de CRS: `critical` 5, `error` 4, `warning` 3, `notice` 2. Las severidades españolas de las plantillas (`critico`/`alto`/`medio`/`bajo`) se mapean a esas cuatro.
- Ajustes nuevos: `anomaly_score_threshold` (5), `executing_paranoia_level` (1), `scoring_mode` (`first_match` | `anomaly`).
- **`first_match` sigue siendo el modo por defecto.** `anomaly` es opt-in: evalúa **todas** las reglas WAF que casan, acumula puntos y bloquea al cruzar el umbral. Es la única forma de respetar el contrato actual mientras se abre la puerta al modelo de CRS.
- Solo se evalúan las reglas con `paranoia_level <= executing_paranoia_level`.
- El registro de auditoría anota la puntuación, la severidad y **qué condiciones casaron**.

### F4 — Rendimiento: que aguante un juego de reglas grande · tamaño M

El bucle actual es O(reglas) × 14 regex, sin índice. Con 42 plantillas da igual; con 700 reglas no.

- **Memoización** del valor transformado por `(colección, clave, pipeline)` dentro de la petición. Coraza lo hace igual.
- **Prefiltro Aho-Corasick**: extraer los literales obligatorios de cada regla, construir un autómata con el juego completo y, por petición, calcular una vez qué literales están presentes. Las reglas cuyo literal no aparece se saltan sin ejecutar el regex. Es la idea del `SecRxPreFilter` de Coraza.
- **Indexado barato** por método y prefijo de path, para no recorrer todas las reglas.
- Banco de pruebas con `criterion` antes y después. El WAF está en el camino de la petición: una regresión aquí se nota.

### F5 — Cuerpo de la petición (`forwardBody`), opt-in · tamaño L · la fase que cruza el techo

- **En Traefik** (lo edita el usuario, fuera del repo): `forwardBody: true` + `maxBodySize` en el middleware, **por router**.
- **En Shuul**: el handler `shuul` lee el cuerpo cuando llega; parsea `application/x-www-form-urlencoded` (y más adelante `multipart/form-data` y JSON) hacia `ARGS_POST`; expone `REQUEST_BODY` acotado por un ajuste tipo `SecRequestBodyLimit`.
- Colecciones nuevas: `ARGS_POST`, `ARGS_POST_NAMES`, `REQUEST_BODY`, `FILES_NAMES`, `FILES_SIZES`. **Nunca** se inspecciona el contenido de un fichero subido, solo su nombre y su tamaño, igual que hace CRS por defecto.
- Límites duros: tamaño máximo inspeccionado y cero bufferización sin cota. El cuerpo es el vector de agotamiento de memoria más obvio que se abre aquí.
- **Consecuencia que hay que documentar en `SELF-HOSTING.md`**: activarlo rompe el streaming (SSE, upgrade a WebSocket, subidas en trozos) en los routers donde se active.

### F6 — Importación de un subconjunto de SecLang · tamaño L · opcional, la última

Solo tiene sentido cuando F1–F5 estén hechas. Requiere además `@pmFromFile`, variables `TX`, `SecMarker` y —la más problemática— `@detectSQLi`/`@detectXSS`, que son libinjection: no hay port maduro a Rust y hacerlo bien pasa por FFI a la biblioteca en C.

**Recomendación: no perseguir la compatibilidad con CRS.** Cubrir el mismo terreno con reglas nativas es más barato, encaja con las 24 plantillas que ya existen y evita arrastrar una dependencia en C. Si algún día se importa, que sea un subconjunto declarado —los ficheros REQUEST-9xx de cumplimiento de protocolo, que son solo cabeceras y URI— con un informe de rechazos, para lo que ya hay precedente en `POST /api/v1/rules/import`.

---

## 6. Mitigación inmediata, gratis y hoy

Traefik tiene un middleware `encodedCharacters` que, **por defecto, rechaza** en el path: `%2F`, `%5C`, `%3B`, `%25`, `%3F`, `%23`.

Eso mata hoy mismo, sin tocar Shuul, buena parte de la clase de evasión del apartado 2.1 (`%2e%2e%2f` no, porque `%2e` es un punto y no está en la lista; pero `..%2f`, `..%5c` y el doble encoding con `%25` sí). Es una línea en la configuración de cada router y no cuesta rendimiento apreciable.

Merece la pena aunque F1 llegue después: son capas distintas y se complementan.

---

## 7. Lo que NO haría, y por qué

| Descartado | Motivo |
|---|---|
| Inspección de **respuesta** (fases 3 y 4, reglas 95x de CRS) | Estructuralmente imposible con ForwardAuth: la respuesta del backend no pasa por Shuul. Exigiría convertir Shuul en proxy inverso. |
| Plugin WASM de Coraza para Traefik | 23× más lento, experimental y sin mantenedor. |
| Coraza nativo | Es Traefik Hub, de pago. |
| Reescribir el motor en Go | Tira a la basura el motor, la UI, la BD y las 384 pruebas, y mete un runtime extra. |
| Compatibilidad total con CRS | Necesita cuerpo, `TX`, `SecMarker`, plugins y libinjection. Fuera de alcance; ver F6. |
| Sustituir `first match wins` por scoring | Rompería el contrato actual y con él las pruebas. Convive como modo opt-in (F3). |

---

## 8. Decisiones abiertas (las tiene que resolver el usuario)

| # | Decisión | Recomendación |
|---|---|---|
| D1 | ¿Transformaciones opt-in por regla, o un conjunto por defecto global? | **Por regla, pipeline vacía por defecto.** Es lo único que garantiza que ninguna regla existente cambie de comportamiento. Coraza recomienda lo mismo y desaconseja meterlas en `SecDefaultAction`. |
| D2 | Al actualizar las plantillas de F1, ¿se les añade `t:none` explícito? | Recomendación original: **sí**, por convención y legibilidad. **Resuelto: la canonización lo hace innecesario.** El marcador `none` limpia la pipeline al parsear, así que declararlo en una plantilla se normalizaría fuera y la especificación almacenada sería la misma; añadirlo sería peso muerto. |
| D3 | ¿El modo scoring sustituye a `first match wins` o convive? | **Convive**, `first_match` por defecto y `anomaly` opt-in. |
| D4 | ¿Se abre la puerta del cuerpo (`forwardBody`)? | **Sí, pero en F5 y opt-in por router.** Antes no aporta nada, porque sin transformaciones el cuerpo se esquiva igual. |
| D5 | ¿Se asume FFI a libinjection para `@detectSQLi`/`@detectXSS`? | **No por ahora.** Fuera de alcance; revisar tras F2. |
| D6 | ¿Objetivo final: cubrir el terreno de CRS con reglas nativas, o importar `.conf`? | **Nativo primero.** Ver F6. |

---

## 9. Orden sugerido

```mermaid
flowchart LR
  F1["F1 · Transformaciones<br/>(mayor valor/esfuerzo)"] --> F2a["F2a · Condiciones 1:N<br/>sin cambio funcional"]
  F2a --> F2b["F2b · Tabla + colecciones + IS_TOR<br/>F2b-1 ✅ · F2b-2a ✅ · F2b-2b ✅ · F2b-3 ✅"]
  F2b --> F2c["F2c · Operadores<br/>CIDR, @pm, validación"]
  F2c --> F2d["F2d · Lógica<br/>AND/OR/NOT"]
  F2d --> F3["F3 · Severidad y scoring<br/>+ niveles de paranoia"]
  F3 --> F4["F4 · Rendimiento<br/>prefiltro y memoización"]
  F4 --> F5["F5 · Cuerpo<br/>forwardBody opt-in"]
  F5 --> F6["F6 · SecLang (opcional)"]
  X["encodedCharacters en Traefik<br/>(hoy, sin tocar Shuul)"] -.-> F1
```

**F1 sola ya cambia el WAF de categoría.** F2 es la que lo pone a la altura estructural de Coraza. F3 lo hace ajustable. F4 lo hace sostenible. F5 cruza el único techo que se puede cruzar.

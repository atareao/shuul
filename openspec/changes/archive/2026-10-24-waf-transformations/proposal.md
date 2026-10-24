# Proposal: waf-transformations

## Why

Las reglas del WAF son correctas y aun así se esquivan. Todas comparan el valor **tal como llega**, sin normalizar, así que el mismo payload codificado de otra forma pasa de largo: `%2e%2e%2f` no casa el regex `\.\./`, `1%20union%20select%201` no casa `union\s+select` y `%3Cscript%3E` no casa `<script`.

La plantilla «Path traversal probes» existe hoy, tiene peso 100 y deniega exactamente las peticiones que ya vienen decodificadas. Frente a un escáner que codifique, es un adorno.

Esta es la fase **F1** del `WAF-ROADMAP.md`: la de mayor valor por esfuerzo, porque convierte las plantillas de detección en reglas que de verdad bloquean. Todo lo demás del roadmap se apoya en ella.

## What Changes

1. **La regla gana una especificación de transformaciones.** La tabla `rules` gana una columna `transformations TEXT NOT NULL DEFAULT ''` y la regla pasa a llevar una *pipeline* `t:` —la misma idea que Coraza— que se aplica al **valor de la petición** antes de compararlo con el regex de la regla. La cadena vacía es una pipeline vacía: el valor no se toca.

2. **Dieciocho transformaciones.** `urlDecode`, `urlDecodeUni`, `lowercase`, `uppercase`, `removeNulls`, `replaceNulls`, `compressWhitespace`, `removeWhitespace`, `normalizePath`, `normalizePathWin`, `htmlEntityDecode`, `jsDecode`, `cssDecode`, `escapeSeqDecode`, `base64Decode`, `cmdLine`, `trim` y `length`. Se aplican **de izquierda a derecha** y el marcador `none` limpia la pipeline construida hasta ese punto. Se dejan fuera las de paridad y las de codificación (`md5`, `sha1`, `base64Encode`, `urlEncode`, `hexEncode`), que no sirven para detección.

3. **La escritura valida la especificación.** Las tres vías de escritura del cliente —creación, edición e importación— rechazan una transformación desconocida nombrando el token, igual que ya rechazan un pipeline no admitido. Lo almacenado es siempre una especificación canónica.

4. **Una especificación irresoluble no entra en la caché.** Si una fila ya almacenada trae un token que no se reconoce —legado o editada a mano—, la regla **no se carga** y se emite un `error!` que nombra la regla y el token. Nunca se ignora en silencio: una regla que participase con la pipeline a medias decidiría algo distinto de lo que su autor escribió.

5. **La auditoría anota el valor transformado.** Cuando una regla que casa lleva una pipeline no vacía **y** restringe al menos uno de los 14 campos, el evento de auditoría del WAF añade la clave `"matched"` —un objeto que mapea cada campo restringido a su valor transformado, con la forma `"matched": {"path": "../"}`—. Con la pipeline vacía —o con una pipeline que no restrinja ningún campo de los 14— el evento es idéntico al de hoy.

6. **La interfaz gana el campo.** La pestaña «Request» del diálogo de regla gana un campo para la especificación, con los **18 nombres canónicos como chips clicables** que la insertan en el orden pulsado —el orden importa: `urlDecodeUni,lowercase` no es `lowercase,urlDecodeUni`—. La especificación resultante queda visible y editable a mano. Se ofrecen chips y no un `Select` múltiple porque este perdería el orden.

7. **Cuatro plantillas pasan a usar transformaciones.** `Path traversal probes`, `SQL injection probes`, `XSS probes` y `Command injection probes` declaran su pipeline y ajustan lo que la pipeline deja obsoleto. De paso suben su severidad **tres** de ellas: `SQL injection probes` de `alto` a `critico`, `Command injection probes` de `alto` a `critico` y `XSS probes` de `medio` a `alto`; `Path traversal probes` se queda en `alto`, que ya es su sitio.

## Capabilities

**Modified Capabilities**

- `rule-matching` — se añaden dos requisitos: aplicar la pipeline al valor de la petición antes de compararlo, y obtener la evidencia del matcheo del mismo recorrido que decide el matcheo. El requisito existente «Rule matching with absent request fields» no cambia: cambia **qué valor** se compara.
- `rules` — se añaden dos requisitos: la validación de la especificación en la escritura y la exclusión de la caché de una regla cuya especificación no se puede leer.
- `templates` — se añade un requisito que obliga a que una plantilla pueda declarar su pipeline y a que aplicarla la traslade a la regla creada.
- `waf-pipeline` — se añade un requisito que obliga a anotar el valor transformado en el evento de auditoría cuando la regla que casa lleva una pipeline no vacía.

**New Capabilities**

- Ninguna. Las transformaciones no son una capacidad nueva: son una propiedad de la regla y del matcheo, y viven en las capabilities que ya existen.

## Impact

- **Base de datos**: una migración aditiva —`ALTER TABLE rules ADD COLUMN transformations TEXT NOT NULL DEFAULT ''`—. Las filas existentes quedan con la cadena vacía, que es la pipeline vacía: ninguna regla cambia de comportamiento.
- **Backend**: un módulo nuevo (`models/transformation.rs` con el enum, la pipeline y sus pruebas), `models/rule.rs` (columna, validación, pipeline precompilada, matcheo y evidencia), `models/log_collector.rs` (clave nueva en el evento), `http/shuul.rs` (emisión de la evidencia), `http/rule.rs` (validación en la importación) y `templates.rs` (campo nuevo y cuatro plantillas).
- **Frontend**: `models/rule.ts` y `models/template.ts` (campo nuevo), `components/dialogs/rule_dialog.tsx` (campo en la pestaña «Request»), `pages/admin/templates_page.tsx` (trasladar la pipeline al aplicar una plantilla).
- **API**: aditiva. `POST`/`PATCH /api/v1/rules` y `POST /api/v1/rules/import` aceptan el campo nuevo y lo validan; `GET /api/v1/rules/export` y `GET /api/v1/templates` lo devuelven. Ningún cliente existente se rompe: el campo es opcional en la entrada y su ausencia equivale a la pipeline vacía.
- **Docs**: `USER-GUIDE.md` (el campo y la lista de transformaciones), `SELF-HOSTING.md` (por qué existen y cuándo usarlas) y `WAF-ROADMAP.md` (marcar F1 como hecha).
- **Fuera de alcance**: el cuerpo de la petición (F5), las condiciones 1:N (F2), la puntuación por anomalía (F3) y el endurecimiento de Traefik en `co1`. Las transformaciones de **codificación** (`base64Encode`, `urlEncode`, `hexEncode`) y las de paridad (`md5`, `sha1`).

## Decisions taken when opening the change

- **Una sola pipeline por regla, aplicada a sus 14 filtros.** Es la forma que cabe en el esquema actual sin inventar una tabla de condiciones, y cubre el caso real: las reglas de detección restringen un solo campo. La limitación —una regla que filtre `path` y `query` a la vez recibe las mismas transformaciones en ambos— es conocida y se resuelve en F2, donde cada condición tendrá las suyas.
- **La pipeline se aplica al valor de la petición, nunca al patrón de la regla.** Transformar el patrón obligaría a recompilar el regex en cada petición y haría ilegible lo que el operador escribió.
- **La pipeline vacía devuelve `Cow::Borrowed`.** Es la garantía que protege las 384 pruebas del backend y las 87 del frontend: con la cadena vacía el valor no se copia ni se toca, así que el matcheo es byte a byte el de hoy.
- **La transformación desconocida se rechaza, no se ignora.** Un token que no se reconoce es un error del autor, no un dato a adivinar. Rechazarlo en la escritura con un 400 que lo nombre —y excluir de la caché la fila que ya lo tuviera— es la única forma de que el fallo se vea.
- **El punto de decisión del matcheo sigue siendo único.** La evidencia que necesita la auditoría se obtiene del **mismo** recorrido de filtros, no de una segunda implementación: dos recorridos divergirían y la auditoría mentiría.
- **La evidencia solo se emite con la pipeline no vacía y al menos un campo restringido.** Con la pipeline vacía el valor transformado es el crudo, que el evento ya lleva: añadirlo sería ruido y cambiaría la forma de todos los eventos existentes. Y con una pipeline no vacía que no restrinja ningún campo de los 14 —solo `is_tor`, o solo `allow`— no hay ningún valor transformado que anotar, así que la clave sería un objeto vacío: tampoco se emite.
- **`length` se incluye aunque no sea una transformación de normalización.** Es la única forma de escribir una regla sobre la **longitud** de un campo —un `User-Agent` de 4 KiB, una `query` de 8 KiB— y cuesta lo mismo que las demás.

## Límites que se aceptan

- **Ninguna regla existente cambia de comportamiento.** La columna nace con la cadena vacía y la pipeline vacía no toca el valor. La prueba de caracterización lo fija: las plantillas con pipeline vacía matchean exactamente igual que antes.
- **El pipeline de Jail comparte el matcher y no cambia.** Las transformaciones viven en `CacheRule::matches()`, que ambos pipelines usan; con la cadena vacía —el valor de toda regla de Jail existente— el camino es el de hoy. No se añade ninguna transformación a las plantillas de Jail.
- **Las transformaciones se aplican una sola vez, en el orden escrito.** No hay expansión a punto fijo —`urlDecodeUni` decodifica una vez, no hasta que el valor deje de cambiar—, porque un bucle de decodificación es una vía de agotamiento de CPU con una entrada hostil. Una evasión de doble codificación se cubre escribiendo `urlDecodeUni,urlDecodeUni`.
- **La normalización de rutas no decide qué es un traversal.** `normalizePath` y `normalizePathWin` resuelven `..`, de modo que una regla de traversal **no** debe llevarlas: lo que hace falta ahí es decodificar (`urlDecodeUni`) para que el `..` se vea.
- **El valor transformado no se persiste en la base de datos.** La evidencia vive en el evento de auditoría, que es un búfer en memoria; no hay tabla nueva ni retención nueva.
- **La especificación es sensible a mayúsculas.** Los nombres son los canónicos (`urlDecodeUni`, no `urldecodeuni`). Un token con otra capitalización se rechaza con el mensaje que lista los válidos. La interfaz ofrece los 18 nombres como chips clicables, así que el camino común no puede producir un token no canónico: el parser estricto solo lo ve quien escribe a mano contra la API.

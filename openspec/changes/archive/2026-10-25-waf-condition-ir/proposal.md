# Proposal: waf-condition-ir

## Why

El matcheo de una regla es hoy una cadena `&&` de comprobaciones cableada a mano
en `CacheRule::evaluate()` (`backend/src/models/rule.rs`), con un `Option<Regex>`
propio por cada uno de los 14 campos de `CacheRule`. Esa forma fija el techo del
motor: solo sabe expresar «un regex por campo, los 14 en AND». No hay sitio para
una colección nueva (una cookie, una cabecera con nombre arbitrario, un argumento
del query), ni para un operador distinto de `regex`, ni para lógica OR o negación.
Es exactamente el techo que la fase **F2** del `WAF-ROADMAP.md` se propone cruzar.

Cruzarlo de una vez mezclaría la reestructuración interna con cambios de
comportamiento, y un fallo en cualquiera de las dos partes sería indistinguible
del otro. Esta fase —**F2a**— hace **solo la reestructuración**: introduce la
representación interna que las fases siguientes necesitan y demuestra que la
proyección desde los 14 campos actuales es **total y equivalente**. Nada más.

## What Changes

1. **Una representación interna de condiciones.** Una regla pasa a describirse,
   por dentro, como un **conjunto de condiciones**. Cada condición declara la
   colección de la petición que restringe, la pipeline de transformaciones que se
   aplica a su valor y el patrón con el que se compara. Un conjunto vacío es una
   regla neutral.

2. **Los 14 campos se proyectan a ese conjunto.** Todo campo que la regla
   restrinja se proyecta a exactamente una condición. La proyección es **total**
   —cubre los 14 campos— y **equivalente** —el resultado observable no cambia—.

3. **Un solo camino de matcheo.** `CacheRule` deja de guardar catorce
   `Option<Regex>` y pasa a compilar un conjunto de condiciones. `evaluate()`
   recorre ese conjunto: mismo `matched`, misma `evidence`, misma forma del evento
   de auditoría.

4. **Sin cambios de esquema, de extracción ni de API.** No hay migración. No se
   toca `NewRequest` ni la resolución de la dirección del cliente. La API pública
   no cambia: sigue recibiendo y devolviendo los 14 campos.

**No es un cambio observable.** Es un cambio de representación interna. Ninguna
regla cambia de comportamiento, ningún evento cambia de forma y ningún cliente se
entera.

## Capabilities

**Modified Capabilities**

- `rule-matching` — se añaden dos requisitos: que el matcheo se defina sobre un
  conjunto de condiciones, y que la proyección de los 14 campos a ese conjunto sea
  total y equivalente. Ningún requisito existente cambia de texto: cambia **cómo**
  se implementa el mismo contrato.

**New Capabilities**

- Ninguna. Las condiciones no son una capacidad nueva: son la representación
  interna del matcheo que ya existe.

## Impact

- **Backend**: `backend/src/models/rule.rs` —la IR (`Condition`, `Collection`,
  `Operator`), la proyección y el matcheo por condiciones— y, si la IR vive en un
  módulo propio, un módulo nuevo (`backend/src/models/condition.rs`).
- **Base de datos**: ninguna. No hay migración.
- **API**: ninguna. El contrato público no cambia.
- **Frontend**: ninguna.
- **Docs**: `AGENTS.md` (la sección que describe `CacheRule` y su matcheo) y
  `WAF-ROADMAP.md` (marcar F2a como entregada, al cerrar).
- **Fuera de alcance**: la tabla `rule_conditions` y la persistencia de
  condiciones (F2b), las colecciones nuevas (F2b), los operadores nuevos (F2c), la
  lógica booleana `and`/`or`/`not` (F2d), la puntuación por anomalía (F3) y el
  cuerpo de la petición (F5).

## Decisions taken when opening the change

- **La fuente de verdad sigue siendo las 14 columnas; la IR es una proyección en
  memoria.** La tabla `rule_conditions` se introduce en **F2b**, cuando haya algo
  que persistir que los 14 campos no pueden expresar. Mantener una sola fuente de
  verdad en todo momento evita la desincronización que traería un `dual-write`.
- **El vocabulario de las colecciones es el de los 14 campos actuales.** Adoptar
  los nombres de colección de Coraza (`REQUEST_HEADERS`, `ARGS_GET`…) exige una
  extracción que hoy no existe —`REQUEST_HEADERS:User-Agent` tendría que
  resolverse del `HeaderMap`—, y eso sí sería un cambio de comportamiento. El
  vocabulario nuevo entra en F2b, junto con la extracción que lo respalda.
- **`is_tor` queda fuera del conjunto de condiciones.** No es un valor de cadena:
  no se transforma ni se compara con un patrón, sino que es un predicado
  booleano. Modelarlo como condición obligaría a inventar un operador booleano
  que nadie más usa. Sigue siendo el chequeo booleano que `evaluate()` hace hoy,
  fuera del conjunto. El roadmap lo lista como colección `IS_TOR`, y su §F2
  detalla la reentrada escalonada —colección en F2b, operador booleano en F2c,
  negación en F2d— junto con la regla de proyección que debe respetar: `true`
  proyecta a una condición, pero `false` y `NULL` son neutrales y **no**
  proyectan, so pena de invertir el sentido de la regla.
- **El conjunto es ordenado y se evalúa en AND con cortocircuito.** Es la
  estructura que ya tiene la cadena `&&`: la primera condición que no casa
  detiene la evaluación. Preservar el cortocircuito es lo que garantiza que la
  evidencia siga describiendo la decisión que de verdad se tomó.
- **La clave de la evidencia sigue siendo el nombre del campo.** El evento de
  auditoría no cambia: la evidencia se sigue anotando como `{"path": "../"}` y no
  como `{"REQUEST_URI_PATH": "../"}`. Renombrar la evidencia sería un cambio
  observable que esta fase no hace.

## Límites que se aceptan

- **Un conjunto de condiciones no aporta nada nuevo todavía.** Esta fase no añade
  ni una colección ni un operador. Su valor es estructural: deja el matcheo
  preparado para que F2b/c/d sean cambios aditivos sobre una base ya probada.
- **La proyección es una función, no un almacén.** Cada carga de la caché vuelve
  a proyectar desde los 14 campos. El coste es despreciable —catorce
  comparaciones por regla al cargar— y desaparece en F2b, cuando la tabla sea la
  fuente.
- **`Operator` nace con una sola variante (`Rx`).** Declarar las otras antes de
  implementarlas sería andamiaje muerto.

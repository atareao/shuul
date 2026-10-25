# Design: waf-condition-ir

## Contexto

El matcheo de una regla era una cadena `&&` cableada a mano en
`CacheRule::evaluate()` (`backend/src/models/rule.rs`), con un `Option<Regex>`
propio por cada uno de los 14 campos y una `TransformationPipeline` compartida.
Esa forma fija el techo del motor: solo expresa «un regex por campo, los 14 en
AND». No hay sitio para una colección nueva, ni para un operador distinto de
`regex`, ni para lógica OR o negación.

La fase **F2** del `WAF-ROADMAP.md` cruza ese techo con un modelo de condiciones
1:N. Cruzarlo de una vez mezclaría la reestructuración interna con cambios de
comportamiento, y un fallo en cualquiera de las dos partes sería indistinguible
del otro. **F2a** hace solo la reestructuración interna y demuestra que la
proyección desde los 14 campos actuales es **total y equivalente**.

## Objetivos y no-objetivos

**Objetivos**

- Introducir la representación interna (`Condition`) que las fases siguientes
  necesitan.
- Proyectar los 14 campos actuales a un `Vec<Condition>` total y equivalente.
- Cero cambio observable: mismo `matched`, misma evidencia, mismo evento, misma
  oferta de API.

**No-objetivos (caen en fases posteriores)**

- La tabla `rule_conditions` y su migración → **F2b**.
- Las colecciones nuevas (cookies, cabeceras, argumentos del query) → **F2b**.
- Los operadores nuevos (`@ipMatch`, `@pm`, de cadena, de validación) → **F2c**.
- La lógica booleana (`condition_operator`, `negate`) → **F2d**.

## Decisiones

### 1. La IR vive en memoria; sin tabla y sin migración

La fuente de verdad siguen siendo los 14 campos de `rules`. La tabla llega en
F2b con su backfill, así que F2a no toca `backend/migrations/`.

### 2. El vocabulario es el de los 14 nombres de campo actuales

Se conserva el vocabulario actual (`ip_address` … `x_request_id`), no los
nombres de colección de Coraza. La clave de la evidencia sigue siendo el
**nombre del campo** (`path`, `user_agent`…), no un nombre de colección, para que
la evidencia no cambie de forma.

### 3. El orden del conjunto es el orden histórico de la cadena `&&`

`Collection::ALL` replica el orden de la cadena `&&`
(`ip_address, protocol, fqdn, path, query, city_name, country_name,
country_code, user_agent, method, referer, content_type, accept_language,
x_request_id`), de modo que la evidencia de una regla que no casa es la misma
que antes.

### 4. `Operator` nace con una sola variante

`Operator::Rx(Regex)`. Los operadores nuevos llegan con F2c; declarar variantes
sin uso sería andamiaje muerto.

### 5. `project` es una función pura y total

`project(rule, &pipeline) -> Vec<Condition>` recorre `Collection::ALL` en orden y
proyecta **una** condición por cada campo con patrón **no vacío y compilable**.
Un patrón `None`, en blanco o que no compile **no** proyecta condición: su
ausencia equivale al brazo neutral `(None, _)` de hoy, de modo que la proyección
es **total** y no convierte un regex inválido en error ni en pánico. Es el único
punto donde los campos de la regla se traducen a condiciones.

### 6. `evaluate` recorre el conjunto en AND con cortocircuito

La primera condición que no casa detiene la evaluación y `matched` es `false`.
La evidencia se acumula en el **mismo** recorrido que decide `matched`, así que
describe la decisión que de verdad se tomó. Una regla sin condiciones es
**neutral**: casa.

### 7. `is_tor` queda FUERA del conjunto (reentrada escalonada)

`is_tor` no es una colección: sigue siendo el predicado booleano `check_tor`,
combinado al final y evaluado **solo** si todas las condiciones casaron. Con
`Operator` reducido a `Rx` no hay forma de comparar un booleano sin inventarle
una representación de cadena, que sería andamiaje muerto. La reentrada es
escalonada, y cada peldaño cae en su sub-entrega:

| Sub-entrega | Qué añade |
|---|---|
| **F2b** | `IS_TOR` en el vocabulario de colecciones y su proyección desde la columna |
| **F2c** | el operador booleano que lo compara |
| **F2d** | `negate` por condición para expresar «solo Tor» |

### 8. Las funciones que solo leen toman la pipeline por referencia

`project` y `from_rule_and_pipeline` no poseen la pipeline: la reciben por
referencia (`&TransformationPipeline`) y la clonan en cada condición. Se
construye una sola vez al cargar la regla, así que clonarla una vez por
condición es irrelevante. Tomarla por valor obligaría a poseer el argumento solo
para satisfacer el lint `needless_pass_by_value` (pedantic).

## Riesgos y compromisos

- **Equivalencia.** El único riesgo real es que la proyección no sea exacta. Se
  cubre con: tests de caracterización por cada uno de los 14 campos, el test de
  la evidencia (clave = nombre del campo, valor transformado), el de campo
  ausente y el de fidelidad de la proyección sobre las 42 plantillas del
  catálogo.
- **Coste del clon.** Una regla con N condiciones clona la pipeline N veces al
  cargarse; el matcheo en caliente no clona nada.
- **Exposición de la IR.** `pub mod condition` expone la IR como API de la lib;
  es defendible, porque `CacheRule::conditions` ya es público.

## Migración

Ninguna. Cero cambios en `backend/migrations/`, cero cambios de esquema y cero
cambios en la oferta de API o en la forma del evento.

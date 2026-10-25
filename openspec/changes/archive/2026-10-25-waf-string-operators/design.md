# Design: waf-string-operators

## Contexto

Ver `proposal.md — Why`. F2a dejó el matcheo sobre un conjunto ordenado de condiciones; F2b-1 hizo de `rule_conditions` la fuente persistida; F2b-2a amplió el vocabulario con las colecciones con clave; F2b-3 añadió la colección booleana `is_tor` y su operador `@truthy`. `Operator` tiene hoy **dos** variantes —`Rx(Regex)` y `Truthy`— y el patrón de una condición vive **dentro** de `Rx`.

Dos restricciones del sistema condicionan el diseño:

1. **El modo simple no puede cambiar.** Las 42 plantillas y las reglas guardadas con los 14 campos y la columna `is_tor` deben decidir byte a byte igual que hoy, y la forma del evento de auditoría también.
2. **La columna `operator` es texto libre y ya existe.** Nació en F2b-1 con `DEFAULT '@rx'` y F2b-3 la abrió a `@truthy`. Añadir seis nombres más no cambia el esquema.

## Objetivos y no-objetivos

**Objetivos:**

- Seis operadores de cadena con la semántica de Coraza, aplicados al valor transformado.
- El patrón como dato del operador, con su patrón canónico, para que la persistencia y la representación dejen de ramificar por variante.
- Validación por operador en el único punto de validación de la autoría avanzada.
- Round-trip estable por `rule_conditions` y descarte de la fila irresoluble con diagnóstico.
- Selector de operador en el editor, con la inferencia de modo intacta.

**No-objetivos:**

- `@pm`, `@ipMatch`, `@ipMatchF` → **F2c-2**.
- `@gt`, `@ge`, `@lt`, `@le` y `@validateByteRange`, `@validateUrlEncoding`, `@validateUtf8Encoding` → **F2c-3**.
- `@detectSQLi` y `@detectXSS` → **F5**.
- `negate` por condición y `condition_operator` (`and`/`or`) → **F2d**.
- Los operadores de **conjunto de nombres** (`args_get_names`, `request_headers_names`, `request_cookies_names`): siguen fuera del vocabulario de colecciones, igual que en F2b-2a.

## Decisiones

### D1. El patrón vive en el operador

Cada variante de `Operator` guarda lo que necesita: `Rx(Regex)`, `Truthy`, `Contains(String)`, `Streq(String)`, `BeginsWith(String)`, `EndsWith(String)`, `Within(Vec<String>)` y `UnconditionalMatch`. `Operator` expone su **patrón canónico** con `pattern(&self) -> Cow<'_, str>` —prestado para las variantes que guardan el patrón tal cual, construido para `Within`, vacío para `Truthy` y `UnconditionalMatch`—, de modo que `From<&Condition> for ConditionInput`, `write_conditions` y la evidencia dejan de tener un `match` sobre las variantes: preguntan al operador por su patrón.

**No** se añade un campo `pattern` a `Condition` ni se cambia `project()` en lo estructural. La alternativa —sacar el patrón a `Condition`— obligaría a tocar la IR, la proyección y todos sus tests, y a decidir qué patrón lleva una condición sin patrón. Mantener la IR estable deja el cambio acotado a `Operator` y a sus tres consumidores.

El round-trip SHALL ser **estable** —persistir lo leído y releerlo da lo mismo—, no necesariamente byte-idéntico al original: `@within` se canoniza uniendo los elementos **ya recortados** por coma **sin espacios**, así que `"GET, POST"` y `"GET ,POST"` se persisten ambos como `"GET,POST"`.

**Semántica de `@within`.** El valor es la **aguja** y el patrón es el **pajar**: casa si el valor es igual a alguno de los elementos de la lista. Es el inverso de `@contains`, donde el patrón es la aguja. Los elementos se recortan de espacios antes de comparar, y un elemento que quede vacío tras el recorte NO casa ningún valor: un patrón con una coma suelta no debe comportarse como un comodín. (La extracción colapsa todo valor vacío a ausencia, así que en la práctica la regla es defensiva: deja escrito el contrato del operador sin depender de que la extracción siga colapsando.)

### D2. Nombres canónicos de Coraza

`@contains`, `@streq`, `@beginsWith`, `@endsWith`, `@within` y `@unconditionalMatch`, con la grafía exacta de Coraza (incluida la mayúscula interior de `@beginsWith`/`@endsWith` y la ausencia del prefijo `@` en el nombre de la variante, que es una cuestión de Rust, no del protocolo). `Operator::name()` pasa a cubrir las ocho variantes y es la representación única: la que la escritura persiste y la que la lectura reconoce.

**`@unconditionalMatch` y la colección ausente.** Coraza hace casar `@unconditionalMatch` siempre, con cualquier entrada. En Shuul el brazo de la colección ausente **precede** al operador —es el fail-open deliberado, hermano del de los campos ausentes—, así que `@unconditionalMatch` casa **siempre que la colección esté presente** y NO casa cuando la petición no la trae, con su `debug!`. Se decide así a propósito: una condición que casara sin que exista el dato que dice restringir rompería la coherencia del conjunto —una regla con dos condiciones de las que una mirase una colección ausente no decidiría lo mismo—, y el fail-open es el contrato ya establecido para toda colección ausente.

**Sensibilidad a mayúsculas.** `@contains`, `@streq`, `@beginsWith` y `@endsWith` son **sensibles a mayúsculas**, como en Coraza; la comparación insensible se obtiene con la transformación `t:lowercase`, que ya existe y se aplica al valor (D4). No se añade una variante insensible por operador: sería una segunda forma de decir lo mismo.

### D3. La validación por operador vive en el compilador de condiciones

`ConditionInput::compile` sigue siendo el **único** punto de validación de la autoría avanzada —lo invocan crear, editar e importar— y pasa a resolver el operador con esta tabla, sin excepciones:

| Operador | Colección | Patrón |
|---|---|---|
| ausente o `@rx` | cualquiera de texto | no vacío y **compilable** como regex |
| `@contains`, `@streq`, `@beginsWith`, `@endsWith` | cualquiera de texto | **no vacío** (literal, no se compila) |
| `@within` | cualquiera de texto | **no vacío** (lista separada por comas) |
| `@unconditionalMatch` | cualquiera de texto | **vacío** |
| `@truthy` | solo `is_tor` | **vacío**, sin clave y sin transformaciones |
| cualquiera de los anteriores | `is_tor` | `400` |
| `@rx` y `@truthy` | cualquiera de texto / `is_tor` | como hasta hoy |

Un operador desconocido sigue siendo `400 Bad Request` nombrando el motivo, y la regla no se almacena. Las transformaciones siguen resolviéndose con `TransformationPipeline::parse` para todos los operadores.

### D4. Los operadores de cadena admiten pipeline, con el contrato vigente

La pipeline de la condición se aplica al **valor de la petición** —nunca al patrón, que se usa tal como se compiló— y el operador decide sobre el resultado. Es el contrato que ya rige para `@rx` y el que hace que `@streq` con `t:lowercase` y patrón `get` case con el método `GET` mientras que el patrón `GET` **no** casaría: el patrón no se transforma.

No se añade ninguna restricción nueva: los seis operadores nuevos son, a este respecto, condiciones como las de `@rx`.

### D5. Persistencia y fallo seguro

`write_conditions` persiste `condition.operator.name()` en la columna `operator` y `condition.operator.pattern()` en la columna `pattern`. `condition_from_row` reconstruye el operador desde el nombre y el patrón, y **descarta** la fila con un diagnóstico que la nombra cuando:

- el nombre del operador es **desconocido** —el caso del rollback a una imagen anterior, que no conoce los seis nombres nuevos—;
- el patrón no compila (`@rx`) o está **vacío** donde el operador lo exige, o **no** está vacío donde lo prohíbe (`@unconditionalMatch`);
- la combinación colección/operador es incoherente (`@truthy` fuera de `is_tor`, `is_tor` con un operador distinto de `@truthy`, un operador de cadena sobre `is_tor`).

Descartar en vez de aplicar mal es el **fallo seguro** ya establecido. La comprobación se concentra en el punto único de reconstrucción (`condition_from_row`) para que `read_conditions` y `read_conditions_batch` no puedan divergir.

### D6. Evidencia: el mismo contrato que `@rx`

Los operadores de cadena anotan evidencia **solo** cuando la condición casa **y** su pipeline no está vacía, bajo la clave de la colección —o la clave compuesta `colección[clave]` para las colecciones con clave—. `@unconditionalMatch` sigue el mismo contrato: con pipeline vacía no anota nada. No se inventa forma nueva para el evento de auditoría y no hay ninguna condición que anote por el mero hecho de haber casado.

### D7. Cero cambio observable y sin migración

`project()` sigue compilando **siempre** `Operator::Rx`, así que el modo simple —los 14 campos y la columna `is_tor`— produce exactamente el mismo conjunto que hoy y las 42 plantillas deciden igual. Los operadores nuevos solo son alcanzables por **autoría avanzada**, que es la vía que declara `conditions` explícitamente.

**No hay migración**: la columna `operator` de `rule_conditions` ya existe, es `TEXT NOT NULL DEFAULT '@rx'` y admite cualquier texto; los seis nombres nuevos no requieren cambio de esquema. Las reglas ya almacenadas conservan `@rx`/`@truthy` y se leen igual.

### D8. El editor gana un selector de operador por fila

`Condition.operator` pasa a la unión de los ocho nombres y `DraftCondition` gana un campo `operator`. El selector ofrece un catálogo de **siete** —`@rx` por defecto más los seis nuevos— y cada fila elige el suyo; el patrón se escribe en el mismo campo de siempre, que para `@within` es la lista separada por comas.

`is_tor` conserva el operador **fijo `@truthy`** y sin selector —ofrecerlo insinuaría una elección que la validación rechaza—, y su fila sigue omitiendo patrón, clave y transformaciones. `@unconditionalMatch` **oculta** el campo de patrón y lo envía vacío: dejar el patrón anterior en la fila produciría un `400` al guardar.

La validación de fila del diálogo impide guardar mientras alguna fila tenga el patrón vacío **salvo** en `@unconditionalMatch`, y señala la fila pendiente, igual que ya hace con la clave que falta en una colección con clave.

**La inferencia de modo no cambia.** Sigue comparando las condiciones recibidas con la proyección de los 14 campos y de la columna `is_tor`, que solo produce `@rx` y `@truthy`: una regla avanzada con operadores de cadena **difiere** de esa proyección y por tanto abre en avanzado con su lista sembrada, sin pérdida.

### D9. Rendimiento

Los operadores de cadena son comparaciones de `str` —`contains`, `==`, `starts_with`, `ends_with` y una búsqueda lineal sobre una lista corta— **sin compilación**: el patrón se guarda ya recortado y partido, no se vuelve a analizar por petición. Los comparadores se construyen **una vez**, al compilar la regla (validación de la escritura o carga de la caché), igual que hoy se compila el `Regex` de `@rx` una sola vez.

`@within` hace una búsqueda lineal sobre la lista de elementos; su tamaño es el de un puñado de métodos HTTP o de acciones, no el de un catálogo, así que no procede ni un `HashSet` ni un autómata de cadenas —que es justo lo que `@pm` (F2c-2) traerá para listas grandes—.

## Riesgos y compromisos

- **El caso peligroso: una regla simple que cambiara de comportamiento.** El riesgo no es un operador nuevo, sino que la generalización del patrón toque el camino del modo simple. → `project()` sigue compilando `Operator::Rx` y los tests de equivalencia sobre las 42 plantillas se mantienen verdes; el escenario «el modo simple no cambia» queda escrito en la spec de `rules`.
- **La fila irresoluble de un rollback.** Una imagen anterior no conoce los seis nombres nuevos y descartaría esas condiciones. → Es el fallo seguro ya establecido (D5), con diagnóstico que nombra la fila. La condición no se aplica mal: no se aplica.
- **`@unconditionalMatch` sin colección presente.** La divergencia deliberada con Coraza (D2) podría sorprender a quien venga del SecLang. → Queda escrita en la spec de `rule-matching` con su escenario, y el `debug!` que la acompaña nombra la colección y la regla.
- **`@within` con elementos vacíos.** Una coma suelta no debe comportarse como un comodín. → El elemento vacío no casa ningún valor (D1) y el patrón no vacío se exige en la validación (D3).
- **El patrón residual al cambiar de operador en el editor.** Elegir `@unconditionalMatch` con un patrón escrito produciría un `400` si el patrón viajara. → El campo se oculta y el patrón viaja vacío (D8).
- **La superficie del editor crece.** El selector añade una fila de formulario por condición. → La validación de fila por operador es la única lógica nueva y vive en el modelo del editor, no en el componente presentacional.

## Migración

**Ninguna migración.** La columna `operator` de `rule_conditions` nació en F2b-1 como `TEXT NOT NULL DEFAULT '@rx'` y admite texto libre; F2b-3 ya la abrió a `@truthy` sin migrar. Los seis nombres nuevos caben en la misma columna, y las reglas ya almacenadas conservan `@rx`/`@truthy` y se leen igual. El rollback se hace revirtiendo la imagen, y las filas con operadores nuevos se descartan con diagnóstico (D5).

## Open Questions

Ninguna que afecte a las specs ni al plan. La pregunta de si `@within` debe admitir una lista con elementos repetidos o vacíos se resuelve con la regla conservadora de D1 —el elemento vacío no casa— y no requiere decisión adicional.

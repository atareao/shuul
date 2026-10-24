# Proposal: jail-weight-coherence

## Why

**El peso de una regla de Jail no decide nada y, aun así, decide el orden de la tabla de Rules.**

El pipeline de Jail itera **todas** las reglas que casan, sin `break`: el orden no influye en el
resultado. El del WAF, en cambio, es `first match wins` por `weight ASC`, que es exactamente donde el
número sí decide. Sin embargo, el catálogo reparte a las plantillas de Jail pesos del mismo rango que a
las del WAF y la interfaz los esconde.

Medido sobre `backend/src/templates.rs` (`all_rule_templates()`, 42 plantillas):

| Pipeline | Plantillas | Pesos distintos |
|---|---|---|
| WAF | 24 | 4 (10, 50, 100, 200, 300) |
| JAIL | 18 | **7** (10 ×2, 50 ×3, 100 ×4, 200 ×6, 9997, 9998, 9999) |

Tres de esas 18 plantillas de Jail son además `must_have: true` («Scanner genérico 404», peso 9998;
«Scanner vulnerabilidades web», peso 10; «Scanner directorios+configs+paneles», peso 10), así que su
peso interviene en la primera posición de la lista del catálogo.

Y en `frontend/src/pages/admin/rules_page.tsx:65` la columna Weight de una regla de Jail se pinta como
`-`: el valor no se ve. El resultado es que al ordenar por Weight las filas de Jail se intercalan con
las del WAF según números invisibles, y la tabla parece desordenada.

**El defecto real que hay detrás.** La ordenación de la tabla es de **servidor**
(`frontend/src/components/custom_table.tsx:388-397` envía `sort_by` y `asc`), y el backend la resuelve
con una lista blanca de columnas seguida de `ORDER BY {sort_by} ASC|DESC LIMIT ? OFFSET ?`
(`backend/src/models/rule.rs:658-700`). **No hay desempate**: cuando dos reglas empatan, SQLite puede
devolverlas en cualquier orden y, al paginar con `LIMIT ? OFFSET ?`, una fila empatada puede
**repetirse en una página y desaparecer de la siguiente**. Catorce reglas comparten hoy el peso 100 en
el catálogo y, tras este cambio, las 18 de Jail compartirán el 0: sin desempate, el síntoma crece.

**Normalizar el peso de Jail es seguro.** El WAF salta las reglas de Jail
(`backend/src/http/shuul.rs:87-88`, «Skip jail-only rules in WAF pipeline»), de modo que el cambio no
puede alterar el orden de evaluación del WAF, que es el único sitio donde el peso decide.

**Y los diálogos arrastran el peso con el pipeline de Jail delante.** `templates_page.tsx:874-885`
ofrece el campo Weight al aplicar una plantilla, y el valor que viaja es el de la plantilla (`:226`,
`ruleWeight: template.weight ?? 100`). En `components/dialogs/rule_dialog.tsx` el campo ya está oculto
para Jail (guard en `:367`, del commit `e4e6ac2`), pero el cuerpo del POST sigue llevando el peso por
defecto (`DEFAULT_VALUES.weight`, 100). En un sitio se ofrece un campo que no decide; en el otro viaja
un número que no decide. El RED lo midió: al aplicar una plantilla de Jail el campo aparece con el peso
de la plantilla, y el POST de una regla de Jail lleva `weight: 100`.

## What Changes

1. **El peso de las 18 plantillas de Jail es 0 en el catálogo** (`backend/src/templates.rs`). Es el
   único valor que puede ser correcto para un pipeline que no ordena: cualquier otro es una decisión
   disfrazada de dato.
2. **La API impone el peso 0 a las reglas de Jail.** No se confía en el cliente: una regla con
   `pipeline` de Jail se guarda con peso 0 aunque la petición traiga otro valor. Es la garantía que
   impide que el problema vuelva por un camino nuevo (importación, edición manual, cliente antiguo).
3. **Una migración normaliza a 0 las reglas de Jail ya almacenadas**, para que las bases ya desplegadas
   —la de producción incluida— queden coherentes sin recrear reglas.
4. **Los diálogos dejan de arrastrar un peso para Jail**: `templates_page.tsx` deja de ofrecer el campo
   Weight cuando la plantilla es de Jail, y en los dos diálogos el valor que viaja para una regla de
   Jail es 0 (en `rule_dialog.tsx` el campo ya estaba oculto; lo que faltaba era el valor).
5. **La ordenación de la lista de reglas gana un desempate determinista**, de modo que dos consultas
   con el mismo criterio devuelvan siempre el mismo orden y la paginación no repita ni pierda filas
   empatadas.

## Non-goals

- **No se cambia la evaluación del WAF** ni su `first match wins` por `weight ASC`. El desempate se
  aplica a la **lista** de reglas (la que consume la tabla), no al orden de evaluación
  (`backend/src/models/rule.rs:170`, el `ORDER BY weight ASC` que carga las reglas del WAF): ese camino
  no se toca. Si en algún momento se quiere que el WAF desempate también, es una línea, y va en su
  propio cambio.
- **No se cambia el comportamiento del pipeline de Jail**: sigue contando todas las reglas que casan.
- **No se toca el peso de las plantillas ni de las reglas del WAF**, que es donde el número sí decide.
- **La columna Weight sigue mostrando `-`** para las reglas de Jail: significa «no aplica», que es
  distinto de «cero».
- **El desbordamiento de los Tags de filtros** en el diálogo de plantillas —el otro detalle
  reportado— va en su propio change, aún sin abrir.

## Impact

- `backend/src/templates.rs` — el peso de las 18 plantillas de Jail
- `backend/src/models/rule.rs` — el desempate en el `ORDER BY` de la lista
- `backend/src/http/rule.rs` y/o `backend/src/models/rule.rs` — el peso 0 impuesto a las reglas de Jail
- `backend/migrations/20261023000000_normalize_jail_rule_weight.up.sql` — migración nueva
- `frontend/src/pages/admin/templates_page.tsx` y `frontend/src/components/dialogs/rule_dialog.tsx` — el
  campo Weight deja de ofrecerse para Jail, y el peso que viaja para una regla de Jail pasa a ser 0
- `openspec/specs/templates/spec.md` — el requisito «Weight alignment with pipeline» documenta hoy el
  contrato viejo (rango 10-100 para los catch-all y 100-200 para las protecciones de servicio de Jail):
  este cambio lo modifica. El delta conserva los nombres de esos dos escenarios de Jail
  («JAIL catch-all have lowest weights» y «JAIL service protection have medium weights»), porque
  OpenSpec exige que un bloque `MODIFIED` no pierda escenarios, con el cuerpo reescrito a peso 0
- **Consecuencia visible, aceptada al abrir el change**: el catálogo de plantillas se ordena por peso
  con las `must_have` primero (`templates_page.tsx:354-357`), así que las plantillas de Jail cambian de
  sitio en esa lista: las tres `must_have` pasan al principio de su grupo y el resto al principio del
  grupo que no es `must_have`.
- **Ninguna aserción existente se debilita**: hoy no hay ni un test que aserte los pesos de Jail (solo
  el recuento 24/18, `allow`, el orden de las `must_have` del WAF, las severidades y el 300 de Tor).

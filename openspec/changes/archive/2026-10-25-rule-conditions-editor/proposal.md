# Proposal: rule-conditions-editor

## Why

El backend ya sabe **persistir** y **evaluar** un conjunto ordenado de condiciones (F2a, F2b-1, F2b-2a): la API acepta el campo `conditions` y la regla se representa con sus condiciones. Pero el único autor de ese formato es… la propia API. El `RuleDialog` sigue ofreciendo los 14 campos planos y no tiene forma de componer una condición con clave —la cookie `PHPSESSID`, la cabecera `Authorization`, el parámetro `id` del query—. Es el tercer tramo de F2b-2 (`WAF-ROADMAP.md` §F2b-2b): **el vocabulario existe, pero no hay interfaz que lo escriba**.

Esta sub-entrega —**F2b-2b**— es **solo frontend**: añade el editor de condiciones al `RuleDialog`. No toca el backend ni la base de datos.

## What Changes

1. **Pestaña de condiciones con conmutador de modo.** El diálogo gana una pestaña que autoriza las condiciones con un control entre **simple** (los 14 campos, como hoy) y **avanzado** (la lista de condiciones).

2. **Contrato de modo respetado.** En simple, el diálogo **omite** `conditions` (el backend proyecta los 14 campos); en avanzado, **envía** la lista (aunque sea vacía) y los 14 campos se almacenan pero no matchean.

3. **Siembra al pasar a avanzado.** La lista inicial se **proyecta** de los 14 campos rellenos, con el mismo criterio que la API (`is_tor` no se proyecta), para que ninguna restricción se pierda al cambiar de modo.

4. **Campos simples deshabilitados en avanzado**, con una nota de que se almacenan pero no participan del matcheo.

5. **Vuelta a simple con confirmación:** se descarta la lista y se envía sin `conditions`.

6. **Inferencia del modo al editar.** La respuesta de la API expone el conjunto **efectivo** —en modo simple, la proyección de los 14 campos—, así que el diálogo infiere el modo comparando las condiciones recibidas con la proyección de los 14 campos. La inferencia es **sin pérdida**: cuando ambos coinciden, volver a enviar en simple reproduce el mismo conjunto.

7. **Editor de condiciones.** Cada condición ofrece colección, clave (solo cuando la colección la exige), transformaciones y patrón; el operador se muestra fijo en `@rx`. El diálogo impide guardar si una colección con clave carece de clave.

**No hay cambio en el backend ni en el modo simple de la API.** Una regla escrita con las 14 columnas se comporta exactamente igual.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `rules`: se documenta el editor del diálogo —los dos modos de autoría, la siembra al pasar a avanzado, los campos simples deshabilitados, la vuelta a simple con confirmación, la inferencia del modo al editar y la validación de la clave—.

## Impact

- **Frontend:** el modelo de la regla (tipo de colección, tipo de condición, `conditions` y la proyección de los 14 campos) y el diálogo de regla (la pestaña de condiciones, el conmutador de modo, el editor de filas, la validación y el cuerpo enviado).
- **Backend:** **ninguno**. El contrato de `conditions` (F2b-2a) no cambia.
- **Base de datos:** **ninguna migración**.
- **API:** **ninguna superficie nueva**; el diálogo pasa a usar el campo `conditions` de escritura que ya existe.
- **Documentación:** `USER-GUIDE.md` (la guía deja de decir que el diálogo sólo autoriza la forma simple), `WAF-ROADMAP.md` y `AGENTS.md`.

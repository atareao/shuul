# Proposal: template-filters-overflow

## Why

**Un filtro de 314 caracteres se sale del diálogo de aplicar una plantilla.**

`templates_page.tsx` previsualiza los filtros de la plantilla como `<Tag>` de antd dentro de un `<Flex wrap gap="small">` (`:745-817`; el scope, en `:826-833`). Un `Tag` de antd es `white-space: nowrap` y `display: inline-block`, y en un contenedor flex todo ítem tiene `min-width: auto`: **no puede encogerse por debajo de su contenido**. Si un valor es más ancho que el contenedor, no se envuelve, no se recorta y no se encoge — se sale.

El catálogo tiene 71 valores de filtro. Medidos sobre `backend/src/templates.rs`, **ocho superan los 90 caracteres** que caben en el cuerpo de un `<Modal width={600}>` a 12 px (unos 540 px útiles, a razón de ~6 px por carácter):

| Longitud | Filtro | Plantilla | Pipeline |
|---|---|---|---|
| 314 | `path` | Scanner directorios+configs+paneles | JAIL |
| 228 | `query` | XSS probes | WAF |
| 222 | `query` | SQL injection probes | WAF |
| 214 | `user_agent` | User-Agent bots conocidos | WAF |
| 172 | `query` | Command injection probes | WAF |
| 150 | `path` | Archivos sensibles | WAF |
| 118 | `path` | Scanner vulnerabilidades web | JAIL |
| 99 | `path` | Paneles de gestion | JAIL |

El más largo, el de `Scanner directorios+configs+paneles`, es un regex de 33 alternativas que a 12 px ronda los **1.900 px**: cabe tres veces y media en el modal, así que el tag se sale del diálogo por la derecha.

**El mismo defecto está en dos sitios.** El detalle expandible de una fila (`renderFilterDetails`, `:490-515`) pinta los mismos valores con la misma construcción —`<Flex wrap gap={4}>` más `<Tag>`— y la tabla no declara `scroll={{ x }}`, de modo que la fila expandida desborda igual. No es un defecto del diálogo: es del tag.

**Y nada lo contiene.** No hay una sola regla `.ant-tag` en `index.css` ni en `App.css` (búsqueda sin resultados) y ningún tag del proyecto declara un ancho máximo.

## What Changes

1. **El tag trunca su contenido con elipsis en vez de desbordar.** Una constante de estilo en `templates_page.tsx`: `maxWidth: "100%"`, `overflow: "hidden"`, `textOverflow: "ellipsis"` y `whiteSpace: "nowrap"`. El `max-width: 100%` es lo que impide el desbordamiento a cualquier ancho de contenedor —no hay ningún valor en píxeles que se quede corto o largo— y el resto es lo que convierte lo que no cabe en una elipsis.
2. **Cada uno de esos tags va dentro de un `Tooltip` de antd que muestra el valor completo.** El texto visible se recorta; el valor íntegro aparece al pasar el ratón. El `Tooltip` envuelve al `Tag` y su `title` es el valor sin recortar.
3. **Se aplica a los dos sitios que desbordan hoy**: la previsualización del diálogo de aplicar plantilla (filtros y scope) y el detalle expandible de la fila.

**Decisiones tomadas al abrir el change:**

- **Se trunca, no se parte el texto.** El regex de 314 caracteres partido en seis líneas dentro de un tag convierte la previsualización —que existe para decidir de un vistazo qué regla se crea— en un muro de texto, y desplaza hacia abajo todo lo demás del diálogo. El truncado deja la previsualización compacta y legible, y no pierde información porque el valor completo está en el tooltip.
- **El tooltip es el componente `Tooltip` de antd**, no el atributo `title` nativo, pese a que el proyecto ya usa el nativo para lo mismo en las tablas (`custom_table.tsx:281`, `ellipsis: { showTitle: true }`). Es una elección de presentación: se prefiere el tooltip con el estilo de la interfaz. Conlleva una consecuencia en los tests: el valor completo no está en el DOM hasta que el tooltip se abre, así que la comprobación tiene que pasar el ratón por encima.

## Non-goals

- **No se parte el texto** (`white-space: normal` + `overflow-wrap`): era la alternativa y se descartó por lo dicho arriba.
- **No se usa el atributo `title` nativo**, por lo dicho arriba.
- **No se fija un ancho en píxeles**: el límite es el del contenedor (`max-width: 100%`), así que el tag no puede desbordar en ningún ancho de ventana.
- **No se toca el catálogo.** Los 314 caracteres del `path` son correctos y se quedan como están: el problema es cómo se pinta, no lo que dice.
- **No se añade `scroll={{ x }}` a la tabla.** El problema no es que falte scroll: es que el tag no se encoge. Añadirlo taparía el síntoma en la tabla y dejaría intacto el del modal.
- **No se revisan los tags que no llevan datos** —pipeline, acción, peso, activo y modo—: son etiquetas fijas y cortas.
- **No hay cambio de contrato de API ni de backend**: es un cambio de presentación.
- **No se hace focusable el tag.** El tooltip de antd se abre con el ratón; con el teclado no, porque un `Tag` es un `span` que no entra en el orden de tabulación. El valor íntegro sigue en el DOM del tag, así que un lector de pantalla lo lee completo; lo que se pierde es que un usuario de teclado vidente pueda desplegar el tooltip. Hacerlo focusable convertiría cada tag en un tab-stop —hasta dieciséis en el diálogo— a cambio de eso, así que queda fuera.

## Impact

- `frontend/src/pages/admin/templates_page.tsx` — la constante de estilo y su aplicación con `Tooltip` en la previsualización del diálogo (`:745-833`) y en `renderFilterDetails` (`:490-515`)
- `frontend/src/pages/admin/templates_page.test.tsx` — los tests del contrato nuevo; los del tooltip tienen que pasar el ratón por encima del tag para que el valor completo llegue al DOM
- `openspec/specs/templates/spec.md` — un requisito nuevo (`ADDED`) sobre la previsualización
- **Ninguna aserción existente se debilita**: los tests de hoy comprueban qué texto aparece, no con qué estilo, y el texto completo sigue en el DOM del tag (el truncado es sólo visual); la plantilla del caso largo sale del fixture, que ya construye plantillas con `makeRuleTemplate(overrides)`
- **Verificación manual en el navegador**: jsdom no calcula layout, así que la elipsis de verdad —los píxeles— sólo se ve en un navegador. Los tests fijan el mecanismo: el estilo que trunca, el tooltip con el valor completo y que el texto íntegro sigue en el DOM

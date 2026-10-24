# Proposal: templates-listing

## Why

La biblioteca de plantillas es hoy ilegible y contiene una pestaña duplicada **y
rota**.

**Legibilidad.** `TemplatesPage` son **3 pestañas → acordeón de ~14 categorías → 42
tarjetas** de 380 px de ancho mínimo (`frontend/src/pages/admin/templates_page.tsx:564-657`):
24 WAF + 18 JAIL, lo que fijan los tests de `backend/src/templates.rs`, más 9
tarjetas de perfiles. El grupo «🔴 Must Have» reúne **16 de las 42** plantillas, de
modo que la etiqueta de urgencia deja de discriminar. Cada tarjeta acumula severidad,
pipeline, acción, cuatro etiquetas de filtro y un `+N more`. No existe **ningún
filtro** por severidad, categoría o acción: solo un buscador de texto. El resultado
es el que describe el usuario: «alguien entra aquí y se vuelve loco».

**La pestaña de perfiles está rota.** Lee `p.max_retry`, `p.find_time_seconds`,
`p.ban_time_seconds`… (`frontend/src/models/template.ts:37-44`), pero el backend
serializa `max_requests`, `window_seconds`, `ban_seconds`… (`backend/src/templates.rs:38-44`)
**sin `rename`**: los valores llegan `undefined`, y `confirmApplyProfile` construye
con ellos el cuerpo del `POST /api/v1/rate-limit-profiles`
(`templates_page.tsx:330-337`), campos que `JSON.stringify` **descarta**. Aplicar un
perfil desde *Templates* crea, en el mejor caso, un perfil sin límites.

## What Changes

- **A. Un listado único.** Las 42 plantillas de regla pasan a **una sola tabla**, sin
  pestañas y sin acordeón: `Name`, `Pipeline`, `Category`, `Severity`, `Action`,
  `Profile` (solo JAIL) y una acción de aplicación. Filtros por columna (pipeline,
  severidad, categoría, acción) más el buscador de texto; **`must_have` deja de ser
  un grupo y pasa a ser un filtro**. La fila se puede expandir para ver los filtros
  completos de la plantilla, que dejan de ocupar el espacio de la lista.

- **B. Los perfiles salen de Templates.** Se eliminan la pestaña *Rate Limit
  Profiles*, su modal y su `confirmApplyProfile`, y el bloque `profiles` deja de
  viajar en la respuesta de `GET /api/v1/templates`. Con ello el desajuste de nombres
  descrito en *Why* desaparece de raíz: el código que lo sufría se borra. Los 9
  perfiles siguen existiendo como datos de la tabla `rate_limit_profiles`, servidos
  por `GET /api/v1/rate-limit-profiles` y gestionados en `RateLimitProfilesPage` — un
  único sitio para el concepto «perfil».

- **C. La cascada del código muerto.** `RateLimitProfileTemplate` y
  `all_rate_limit_profile_templates()` quedan sin consumidor en el backend y se
  retiran, junto con los tests que asertan `profiles.len() == 9`.

## Non-goals

- No se cambia el catálogo: ni el número de plantillas (24 WAF / 18 JAIL), ni sus
  filtros, pesos, severidades o categorías.
- No se añade «New from template» a `RateLimitProfilesPage`; queda para otro change.
- No se unifica el flujo *Apply* con `RuleDialog`: sigue habiendo un modal propio que
  hace `POST /api/v1/rules`.
- No se toca el orden de prioridad (`must_have` primero, pesos) que fija la spec
  `templates`: cambia **dónde se ve**, no **cuál es**.
- No se tocan las migraciones SQL que siembran los 9 perfiles en
  `rate_limit_profiles`: los seeds reales siguen ahí. Solo se retira la **copia
  estática en Rust**.

## Impact

- `frontend/src/pages/admin/templates_page.tsx` — reescritura de la presentación; retirada de la pestaña de perfiles y de su flujo de aplicación
- `frontend/src/models/template.ts` — `TemplatesResponse` pierde `profiles`; se elimina el tipo `RateLimitProfileTemplate`
- `backend/src/http/template.rs` — la respuesta deja de incluir `profiles`
- `backend/src/templates.rs` — se retiran `RateLimitProfileTemplate`, `all_rate_limit_profile_templates()` y sus tests
- `AGENTS.md` §Templates — la cifra documentada (81 plantillas: 35 WAF + 46 JAIL) no coincide con el código (42): se corrige, y se refleja que `profiles` ya no está en el payload
- Sin migraciones, sin cambios en la base de datos, sin cambios en el pipeline WAF/Jail

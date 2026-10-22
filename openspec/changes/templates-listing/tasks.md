# Tasks: templates-listing

> **Alcance:** presentación de la biblioteca de plantillas (listado único con filtros)
> y retirada del bloque de perfiles de rate limit de `GET /api/v1/templates`.
> **NO** se toca el catálogo (número, filtros, pesos, severidades, categorías), ni el
> pipeline WAF/Jail, ni la base de datos, ni las migraciones que siembran los 9
> perfiles.
> **Baseline:** `npx vitest run`, `npx tsc --noEmit`, `cargo test` y
> `cargo clippy -- -D warnings` en verde sobre `develop` antes de empezar. Los
> recuentos se anotan y no deben bajar.

## 1. Preparación

- [ ] 1.1 Anotar los baselines de frontend y backend con sus recuentos. Verificación: los números quedan escritos en esta tarea.
- [ ] 1.2 Verificar que `RateLimitProfileTemplate` y `all_rate_limit_profile_templates()` no tienen más consumidores que `backend/src/http/template.rs` y los tests de `backend/src/templates.rs`. Verificación: `grep -rn "RateLimitProfileTemplate\|all_rate_limit_profile_templates" backend/src` y el listado de consumidores queda escrito aquí.
- [ ] 1.3 Inventariar los tests de `backend/src/templates.rs` que asertan `profiles.len() == 9` y decidir cuáles se eliminan. Verificación: la lista queda escrita aquí.
- [ ] 1.4 Confirmar que el número de plantillas del catálogo no cambia: 24 WAF y 18 JAIL. Verificación: los tests de conteo de `backend/src/templates.rs` siguen en verde sin tocarlos.

## 2. Tests (RED)

- [ ] 2.1 RED: test de que la respuesta de `GET /api/v1/templates` **no** incluye la clave `profiles` y sí `waf` y `jail`. Verificación: `cargo test` falla en aserción.
- [ ] 2.2 RED: test de componente (`@testing-library/react`) de que la biblioteca renderiza **42 filas en un único listado** y ninguna pestaña de perfiles. Verificación: `npx vitest run` falla en aserción.
- [ ] 2.3 RED: test de que el filtro por pipeline reduce el listado a las 18 plantillas JAIL y el filtro "must have" a las 16 marcadas. Verificación: el test falla en aserción.
- [ ] 2.4 RED: test de que el orden por defecto pone primero las plantillas `must_have`. Verificación: el test falla en aserción.

## 3. GREEN

- [ ] 3.1 `backend/src/http/template.rs`: retirar el bloque `profiles` de la respuesta.
- [ ] 3.2 `backend/src/templates.rs`: eliminar `RateLimitProfileTemplate`, `all_rate_limit_profile_templates()` y los tests de conteo de perfiles que ya no aplican.
- [ ] 3.3 `frontend/src/models/template.ts`: quitar `profiles` de `TemplatesResponse` y eliminar el tipo `RateLimitProfileTemplate`.
- [ ] 3.4 `frontend/src/pages/admin/templates_page.tsx`: sustituir pestañas + acordeón + tarjetas por el listado único con filtros; eliminar `renderProfilesSection`, `openProfileApplyModal`, `confirmApplyProfile` y todo el estado asociado a perfiles.
- [ ] 3.5 Mantener intacto el flujo *Apply* de las plantillas de regla (modal propio + `POST /api/v1/rules`): debe seguir funcionando para WAF y JAIL.
- [ ] 3.6 `AGENTS.md`: corregir §Templates (42 plantillas: 24 WAF + 18 JAIL) y reflejar que `profiles` ya no viaja en el payload de `/api/v1/templates`.

## 4. REFACTOR

- [ ] 4.1 `cargo clippy -- -D warnings` limpio: es lo que detecta el código muerto de la cascada.
- [ ] 4.2 `cargo fmt --check`.
- [ ] 4.3 `npx tsc --noEmit` sin errores.
- [ ] 4.4 `npm run lint` sin errores ni avisos nuevos y **sin** `eslint-disable`.
- [ ] 4.5 `cargo test` y `npx vitest run` en verde, sin bajar de los baselines.

## 5. Verificación manual

- [ ] 5.1 La biblioteca se ve como un listado y ya no como tarjetas en pestañas.
- [ ] 5.2 Los filtros por pipeline, severidad, categoría y "must have" funcionan y se combinan con la búsqueda.
- [ ] 5.3 Ya no existe ninguna pestaña ni acción de perfiles de rate limit en la página.
- [ ] 5.4 Aplicar una plantilla sigue creando la regla correctamente y la regla aparece en *Rules*.

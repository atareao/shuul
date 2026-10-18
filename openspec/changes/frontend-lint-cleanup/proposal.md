# Frontend lint cleanup: 114 pre-existing ESLint errors

## Intent

Dejar `pnpm run lint` en verde para que el CI (`ci.yml`, añadido en `v0.14.0`)
pueda proteger de verdad el frontend. El linter nunca se ejecutó en CI, así que
la deuda se acumuló sin que nadie la viera.

**Objetivo: cero errores sin relajar la configuración de ESLint.** No se permiten
`eslint-disable`, ni añadir excepciones al config, ni degradar reglas a warning.
El arreglo es tipar de verdad.

## Scope

Solo `frontend/src/`. No se toca `eslint.config.js`, ni el backend, ni la
configuración del CI.

## Impact

Refactor puro de tipos: **sin cambios de comportamiento observable**. Los tres
elementos que lo verifican ya existen:

- `pnpm run lint` — debe pasar a 0 errores (hoy 114 en 23 ficheros)
- `pnpm run build` (`tsc -b && vite build`) — el typecheck es la red de seguridad principal
- `pnpm run test` (vitest) — `src/pages/admin/logs_page.test.tsx` como test de caracterización

## Estado de partida (medido con `eslint -f json`)

| Regla | Errores |
|---|---|
| `@typescript-eslint/no-explicit-any` | 82 |
| `@typescript-eslint/no-unsafe-function-type` | 10 |
| `@typescript-eslint/no-unused-vars` | 9 |
| `@typescript-eslint/no-empty-object-type` | 8 |
| `prefer-const` | 4 |
| `no-empty` | 1 |

| Fichero | Errores |
|---|---|
| `src/components/dialogs/custom_dialog.tsx` | 13 |
| `src/components/custom_table.tsx` | 12 |
| `src/components/dialogs/rule_dialog.tsx` | 10 |
| `src/pages/admin/dashboard_page.tsx` | 9 |
| `src/components/dialogs/rate_limit_profile_dialog.tsx` | 8 |
| `src/common/utils.ts` | 7 |
| `src/pages/admin/templates_page.tsx` | 7 |
| `src/components/register.tsx` | 6 |
| `src/components/signin.tsx` | 6 |
| `src/common/types.ts` | 4 |
| `src/pages/admin/bans_page.tsx` | 4 |
| `src/pages/admin/settings_page.tsx` | 4 |
| `src/components/auth_context.tsx` | 3 |
| `src/components/mode_context.tsx` | 3 |
| `src/layouts/admin_layout.tsx` | 3 |
| `src/layouts/public_layout.tsx` | 2 |
| `src/pages/admin/charts_page.tsx` | 2 |
| `src/pages/admin/help_page.tsx` | 2 |
| `src/pages/admin/logs_page.tsx` | 2 |
| `src/pages/admin/rate_limit_profiles_page.tsx` | 2 |
| `src/pages/public/home_page.tsx` | 2 |
| `src/pages/public/login_page.tsx` | 2 |
| `src/pages/admin/rules_page.tsx` | 1 |

## Qué NO es este cambio

- No cambia ninguna funcionalidad ni ningún endpoint.
- No reescribe el CI ni el config del linter.
- No arregla los `dependabot` PRs abiertos (se beneficiarán solos cuando esto entre).

# Tasks: frontend-lint-cleanup

> Refactor puro (`skip_specs: true`). La "RED" aquí es la medición del linter y
> la "GREEN" es que `lint` + `build` + `test` pasen. Prohibido `eslint-disable`,
> cambios en `eslint.config.js` o degradar reglas.

## Task 0: Baseline y red de seguridad
- [x] MEDIR: `cd frontend && npx eslint . -f json` → **114 errores en 23 ficheros**
- [x] VERIFICAR: `pnpm run test` → **5/5 tests pasan** (1 fichero, `logs_page.test.tsx`)
- [x] VERIFICAR: `pnpm run build` (`tsc -b && vite build`) → **verde** (36.6 s)
- [x] ANOTAR: recuento por regla = 82 any · 10 Function · 9 unused · 8 empty-object · 4 prefer-const · 1 no-empty

## Task 1: Auto-fixables (`prefer-const`, `no-empty`)
- [x] `npx eslint . --fix` y revisar el diff (no debe tocar nada más)
- [x] `pnpm run lint` → el recuento baja; `pnpm run build` sigue verde

## Task 2: `no-explicit-any` en `src/common/` (11)
- [x] `src/common/types.ts` (4) — tipar el helper de debounce y los genéricos
- [x] `src/common/utils.ts` (7) — sustituir `any` por genéricos reales (`<T>`, `unknown` + narrowing)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 3: `no-explicit-any` en diálogos (31)
- [x] `src/components/dialogs/custom_dialog.tsx` (13) — parametrizar `CustomDialog<T>`
- [x] `src/components/dialogs/rule_dialog.tsx` (10)
- [x] `src/components/dialogs/rate_limit_profile_dialog.tsx` (8)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 4: `no-explicit-any` en `custom_table.tsx` (12)
- [x] Tipar `CustomTable<T>` y sus callbacks (`clientFilter`, `extraHeaderContent`…)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 5: `no-explicit-any` en páginas (29)
- [x] `src/pages/admin/dashboard_page.tsx` (9)
- [x] `src/pages/admin/templates_page.tsx` (7)
- [x] `src/pages/admin/bans_page.tsx` (4)
- [x] `src/pages/admin/settings_page.tsx` (4)
- [x] `src/pages/admin/charts_page.tsx` (2)
- [x] `src/pages/admin/help_page.tsx` (2)
- [x] `src/pages/admin/logs_page.tsx` (2)
- [x] `src/pages/admin/rate_limit_profiles_page.tsx` (2)
- [x] `src/pages/public/home_page.tsx` (2)
- [x] `src/pages/public/login_page.tsx` (2)
- [x] `src/pages/admin/rules_page.tsx` (1)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 6: `no-unsafe-function-type` y `no-empty-object-type` (18)
- [x] `src/components/auth_context.tsx` (3) — reemplazar `Function` por firmas explícitas
- [x] `src/components/mode_context.tsx` (3)
- [x] `src/components/register.tsx` (6)
- [x] `src/components/signin.tsx` (6) — e interfaces vacías por `Record<string, never>` o tipo alias
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 7: `no-unused-vars` restantes (9)
- [x] Eliminar imports/variables sin usar (usar el prefijo `_` solo si el parámetro es posicional obligatorio)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Task 8: Layouts
- [x] `src/layouts/admin_layout.tsx` (3), `src/layouts/public_layout.tsx` (2)
- [x] `pnpm run lint`, `pnpm run build` y `pnpm run test` verdes

## Final verification
- [x] `cd frontend && pnpm run lint` → **0 errores**
- [x] `cd frontend && pnpm run build` → verde (incluye `tsc -b`)
- [x] `cd frontend && pnpm run test` → verde
- [x] `git diff --stat frontend/eslint.config.js` → **vacío** (no se ha tocado el config)
- [x] `grep -rn "eslint-disable" frontend/src` → **sin resultados nuevos**
- [x] `cd backend && cargo test && cargo clippy -- -D warnings` → sin regresiones
- [x] Abrir PR `feature/frontend-lint-cleanup` → `develop` y comprobar CI en verde

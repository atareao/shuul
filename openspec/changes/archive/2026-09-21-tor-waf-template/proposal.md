# Proposal: Add WAF template for Tor exit nodes

## Why

Shuul ya tiene detección de IPs de Tor exit nodes via `TorService` y el campo `is_tor` en reglas, pero no existe ningún template WAF preconfigurado para bloquear tráfico Tor. Los usuarios deben crear la regla manualmente. Un template "Tor exit nodes" permitiría bloquear Tor con un solo clic.

## What Changes

- Añadir campo `is_tor: Option<bool>` a `RuleTemplate` (Rust) y su builder method `is_tor()`.
- Añadir campo `is_tor: boolean | null` a `RuleTemplate` (TypeScript frontend).
- Crear template WAF "Tor exit nodes" con `is_tor: true`, `allow: false`, `must_have: false`, peso 300 (último en evaluarse, después de Geo blocking).
- Actualizar `confirmApply` en frontend para enviar `is_tor` al crear la regla.
- Actualizar conteo de templates WAF de 23 a 24.

## Capabilities

### Modified Capabilities

- `templates`: Se modifica el requisito "Template count" (WAF pasa de 23 a 24). Se añade un nuevo requisito "Tor exit nodes template" con su escenario.
- `tor-filter`: No cambia (el campo `is_tor` ya existe en `Rule` y `NewRule`).

## Impact

- **Backend**: `backend/src/templates.rs` — añadir campo `is_tor` a `RuleTemplate`, builder method, y nuevo template.
- **Frontend**: `frontend/src/models/template.ts` — añadir `is_tor` a la interfaz. `frontend/src/pages/admin/templates_page.tsx` — enviar `is_tor` en `confirmApply`.
- **Tests**: Actualizar conteo de templates WAF (23→24).
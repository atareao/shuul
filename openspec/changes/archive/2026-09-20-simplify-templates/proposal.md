# Proposal

## Why

Los templates WAF y JAIL en `backend/src/templates.rs` han crecido hasta 2470 líneas con 81 templates, la mayoría con enorme redundancia estructural (14 campos `None` por template) y conceptual (46 templates JAIL que solo varían en path regex + profile_id). Esto dificulta el mantenimiento, la navegación y la aplicación de templates por parte del usuario. Además, varios templates tienen inconsistencias (ej: `allow: false` en pipeline `jail`) y pesos arbitrarios que no se alinean con el nuevo flujo weight ASC.

## What Changes

- **Reducir WAF templates de 35 → 21**, fusionando templates redundantes (ej: Laravel debug bar + Telescope + Ignition en uno solo)
- **Reducir JAIL templates de 46 → 16**, eliminando la explosión CMS-específica y usando patrones genéricos
- **Eliminar boilerplate** usando un builder pattern en Rust (cada template pasa de ~25 a ~8 líneas)
- **Ordenar templates por prioridad** (más necesario → menos necesario) dentro de cada pipeline
- **Corregir inconsistencias**: templates JAIL con `allow: false` se mueven a WAF o se corrigen a `allow: true`
- **Añadir templates nuevos**: Health & Webhooks (P6), Recidive (P7)
- **Alinear pesos (weight)** con el flujo weight ASC del pipeline
- **Actualizar frontend** para reflejar la nueva organización y categorización

## Capabilities

### New Capabilities
- `templates`: Define el catálogo de templates WAF y JAIL, su estructura, orden de prioridad y comportamiento esperado

### Modified Capabilities
- `waf-pipeline`: Los templates WAF se reordenan por prioridad (más necesario primero) y se actualizan los pesos para el flujo weight ASC

## Impact

- **Backend**: `backend/src/templates.rs` se reescribe completamente (~2470 → ~700 líneas)
- **Frontend**: `frontend/src/pages/admin/templates_page.tsx` se actualiza para reflejar nuevo orden y categorías
- **Frontend**: `frontend/src/models/template.ts` se mantiene igual (interfaces no cambian)
- **No breaking**: La API `/api/v1/templates` mantiene el mismo contrato (`{ waf: [], jail: [], profiles: [] }`)
- **No breaking**: Los templates aplicados como reglas existentes no se modifican
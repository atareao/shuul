# Proposal: Fix is_tor NOT NULL constraint when creating rules from templates

## Why

Al crear una regla desde un template, el frontend no envía el campo `is_tor` en el body JSON. El backend recibe `is_tor: None` y hace `bind(rule.is_tor)`, lo que envía NULL a SQLite. La columna `is_tor` tiene `NOT NULL DEFAULT 0`, pero el bind explícito de NULL sobreescribe el default, causando el error `NOT NULL constraint failed: rules.is_tor`.

## What Changes

- En `Rule::create()`, cambiar `bind(rule.is_tor)` por `bind(rule.is_tor.unwrap_or(false))`, siguiendo el mismo patrón que `weight`, `mode`, `pipeline`, `allow` y `active`.
- No hay cambios en la base de datos, ni en el frontend, ni en templates.

## Capabilities

### skip_specs: true

Este cambio es puramente de implementación. No altera ningún comportamiento a nivel de spec:
- El campo `is_tor` en `NewRule` sigue siendo `Option<bool>`.
- La columna `is_tor` en DB sigue siendo `NOT NULL DEFAULT 0`.
- El frontend puede seguir sin enviar `is_tor` (se usará `false` por defecto).
- Templates no necesitan campo `is_tor`.

## Impact

- **Archivo modificado**: `backend/src/models/rule.rs` (una línea en `create()`).
- **Sin cambios de API**: el endpoint `POST /api/v1/rules` sigue aceptando los mismos payloads.
- **Sin migraciones**: no se toca la base de datos.
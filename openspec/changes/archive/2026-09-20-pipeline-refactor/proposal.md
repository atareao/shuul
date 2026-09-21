# Pipeline Refactor: Remove Whitelist/Blacklist, WAF before Ban

## Intent

Simplificar el pipeline WAF eliminando las entidades Whitelist y Blacklist como conceptos separados. La funcionalidad de "siempre permitir" y "siempre denegar" se cubre mediante WAF rules con `allow=true`/`allow=false` y peso bajo. El orden del pipeline pasa a ser: **WAF rules → Ban check → default allow**.

## Scope

### Backend Rust
- Eliminar modelos `whitelist.rs` y `blacklist.rs`
- Eliminar handlers HTTP `whitelist.rs` y `blacklist.rs`
- Eliminar rutas `/api/v1/whitelist` y `/api/v1/blacklist` de `main.rs`
- Eliminar campos `whitelist` y `blacklist` de `AppState`
- Eliminar migraciones SQL de whitelist/blacklist
- Reordenar pipeline en `shuul.rs`: WAF rules primero, Ban check después
- Eliminar categorías `whitelist`/`blacklist` de `should_log()`

### Frontend React
- Eliminar páginas `whitelist_page.tsx` y `blacklist_page.tsx`
- Eliminar modelos `whitelist.ts` y `blacklist.ts`
- Eliminar rutas y navegación de whitelist/blacklist

### Documentación
- Actualizar `AGENTS.md` con el nuevo flujo del pipeline
- Actualizar `openspec/specs/waf-pipeline/spec.md`

## Impact

- **Breaking change**: API endpoints `/api/v1/whitelist` y `/api/v1/blacklist` desaparecen
- **Breaking change**: Settings `safe_paths`, `trusted_ips`, `trusted_user_agents` se eliminan
- **No breaking**: WAF rules existentes siguen funcionando igual
- **No breaking**: Jail pipeline no se modifica
- **No breaking**: Ban manager no se modifica
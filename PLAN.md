# Plan: Añadir columna `pipeline` a `rules`

## 1. Migración SQL
Crear `backend/migrations/20260827000001_add_pipeline_to_rules.up.sql`:
```sql
ALTER TABLE rules ADD COLUMN pipeline VARCHAR NOT NULL DEFAULT 'waf';
```

## 2. Backend — `backend/src/models/rule.rs`
- Añadir `pub pipeline: String` a `Rule`
- Añadir `pub pipeline: Option<String>` a `NewRule`
- Añadir `pub pipeline: String` a `UpdateRule`
- Añadir `pub pipeline: Option<String>` a `ReadRuleParams`
- `from_row()`: `row.get("pipeline")`
- `create()`: añadir pipeline a INSERT y bind
- `update()`: añadir pipeline a UPDATE y bind
- `count_paged()` / `read_paged()`: filtro pipeline como exact-match

## 3. Backend — `backend/src/http/shuul.rs`
En loop de matching, saltar reglas `pipeline == "jail"`:
```rust
if cache_rule.rule.pipeline == "jail" { continue; }
```

## 4. Backend — `backend/src/http/report.rs`
En loop de matching, saltar reglas `pipeline == "waf"`:
```rust
if cache_rule.rule.pipeline == "waf" { continue; }
```

## 5. Backend — `backend/src/http/rule.rs`
Añadir `pipeline` a la query UPSERT de `import_handler`.

## 6. Frontend — `frontend/src/models/rule.ts`
- Añadir `pipeline?: string` a la interfaz Rule
- Eliminar `getRuleType()`, `RuleType` y `type: 'tag'` virtual

## 7. Frontend — `frontend/src/pages/admin/rules_page.tsx`
- Sustituir columna virtual `type` por columna real `pipeline` con `type: 'tag'`
- Opciones: waf (blue), jail (green)
- Eliminar opción `both`
- `clientFilter` filtra por `record.pipeline`

## 8. Frontend — `frontend/src/components/dialogs/rule_dialog.tsx`
- Añadir `pipeline: "waf"` a DEFAULT_VALUES
- Añadir pipeline a initializeFromItem() y formatForApi()
- Tabs: General (active, pipeline, name, description, weight, allow*, store*, mode*, rate_limit_profile_id**), Network, Location, Request
- *allow y mode ocultos cuando pipeline == "jail"
- **rate_limit_profile_id oculto cuando pipeline == "waf"
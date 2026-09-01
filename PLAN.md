# Plan de Mejora de UX — Vistas de Reglas y Peticiones

## Objetivo

Mejorar la experiencia de usuario en las vistas de administración de reglas y peticiones, simplificando los campos visibles y añadiendo un diálogo de detalle para las peticiones.

## Arquitectura

Cambios en backend (modelo `Request` con JOIN a `rules`) y frontend (modelo `Record`, simplificación de tablas, nuevo diálogo de detalle). No se modifican rutas ni lógica de negocio existente.

## Tareas

### Tarea 1: Backend — Añadir `rule_name` al modelo Request

**Archivos:**
- Modificar: `backend/src/models/request.rs`

- [ ] **Paso 1:** Añadir campo `rule_name: Option<String>` al struct `Request`
      ```rust
      pub struct Request {
          // ... campos existentes ...
          pub rule_name: Option<String>,
      }
      ```

- [ ] **Paso 2:** Modificar `from_row()` para extraer `rule_name` del resultado
      ```rust
      rule_name: row.get("rule_name").ok(),
      ```

- [ ] **Paso 3:** Modificar `read_paged()` para usar LEFT JOIN con `rules`
      ```sql
      SELECT requests.*, rules.name as rule_name
      FROM requests
      LEFT JOIN rules ON requests.rule_id = rules.id
      ```
      Mantener el resto de la cláusula (WHERE, ORDER BY, LIMIT/OFFSET) igual.

- [ ] **Paso 4:** Modificar `read()` (lectura individual) para incluir el mismo LEFT JOIN.
      ```sql
      SELECT requests.*, rules.name as rule_name
      FROM requests
      LEFT JOIN rules ON requests.rule_id = rules.id
      WHERE requests.id = $1
      ```

- [ ] **Paso 5:** Dejar `count_paged()` sin cambios (solo cuenta registros, no necesita JOIN).

- [ ] **Paso 6:** Dejar `create_bulk()` y `create()` sin cambios (solo insertan, no leen `rule_name`).

### Tarea 2: Frontend — Actualizar modelo Record

**Archivos:**
- Modificar: `frontend/src/models/record.ts`

- [ ] **Paso 1:** Añadir campo opcional `rule_name` a la interfaz `Record`
      ```typescript
      export interface Record {
          // ... campos existentes ...
          rule_name?: string;
      }
      ```

### Tarea 3: Frontend — Simplificar campos visibles en RulesPage

**Archivos:**
- Modificar: `frontend/src/pages/admin/rules_page.tsx`

- [ ] **Paso 1:** Localizar la definición de columnas de la tabla de reglas.

- [ ] **Paso 2:** Mantener visibles solo estos campos:
      - `active`
      - `allow`
      - `store`
      - `weight`
      - `name`
      - `description`
      - `mode`
      - `profile` (rate_limit_profile_id)

- [ ] **Paso 3:** Ocultar (poner `visible: false` o eliminar la columna) estos campos:
      - `id`
      - `ip_address`
      - `protocol`
      - `fqdn`
      - `path`
      - `query`
      - `city_name`
      - `country_name`
      - `country_code`

- [ ] **Paso 4:** Verificar que `custom_table.tsx` ya aplica `ellipsis: { showTitle: true }` en todas las columnas — no es necesario añadirlo.

### Tarea 4: Frontend — Simplificar RequestsPage + diálogo de detalle

**Archivos:**
- Modificar: `frontend/src/pages/admin/requests_page.tsx`
- Crear: `frontend/src/components/dialogs/request_detail_dialog.tsx`

- [ ] **Paso 1:** En `requests_page.tsx`, mantener visibles solo estos campos:
      - `created_at`
      - `rule_name` (en lugar de `rule_id`)
      - `ip_address`
      - `fqdn`
      - `path`
      - `user_agent` (con label "Agent")
      - `country_name` (con label "Country")

- [ ] **Paso 2:** Ocultar estos campos:
      - `protocol`
      - `query`
      - `method`
      - `referer`
      - `content_type`
      - `accept_language`
      - `x_request_id`
      - `city_name`
      - `country_code`
      - `rule_id`

- [ ] **Paso 3:** Reemplazar la columna de acciones actual (botón "Rule") por un botón "Details" que abra `RequestDetailDialog`.

- [ ] **Paso 4:** Crear `RequestDetailDialog` en `frontend/src/components/dialogs/request_detail_dialog.tsx`:
      - Modal de solo lectura con dos tabs:
        - **General**: `created_at`, `ip_address`, `fqdn`, `path`, `user_agent`, `country_name`, `rule_name`
        - **Details**: `protocol`, `query`, `method`, `referer`, `content_type`, `accept_language`, `x_request_id`, `city_name`, `country_code`
      - Botón "Create Rule from Request" al pie del diálogo que abre el `CreateRuleFromRequestDialog` existente.

- [ ] **Paso 5:** Importar y renderizar `RequestDetailDialog` en `requests_page.tsx`, pasando el registro seleccionado como prop.
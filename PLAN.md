# Three-mode `log_all_requests` — Implementation Plan

## Objetivo

Extender `log_all_requests` de `bool` (`true`/`false`) a tres modos (`"all"`, `"pass"`, `"audit"`) para que los usuarios puedan registrar solo eventos "pass" sin el ruido de safe_path, trusted_ip, trusted_ua, log_only, allow, ni eventos del pipeline Jail.

## Arquitectura

La configuración se almacena como string en la tabla `settings` (clave `log_all_requests`). El modelo `Settings` parsea el string en Rust como `String` con tres valores aceptados. Un helper `LogLevel` centraliza la lógica de decisión para cada categoría de log. Los pipelines WAF (`shuul.rs`) y Jail (`report.rs`) usan este helper en lugar del booleano directo. El frontend cambia de `<Switch>` a `<Select>` con tres opciones.

### Mapa de archivos

| Archivo | Cambio |
|---|---|
| `backend/migrations/20260903000000_add_log_all_requests_setting.up.sql` | Actualizar valor por defecto de `'true'` a `'all'` |
| `backend/src/models/settings.rs` | `log_all_requests: bool` → `String`. Default `"all"`. Parseo: string directo. Guardado: string directo. |
| `backend/src/http/shuul.rs` | Reemplazar 6 usos de `if log_all_requests` con helper `LogLevel::should_log()` |
| `backend/src/http/report.rs` | Reemplazar 4 usos de `if log_all_requests` con helper |
| `backend/src/http/settings.rs` | `SettingsResponse.log_all_requests` y `UpdateSettingsPayload.log_all_requests`: `bool` → `String`. Validar valor en `update_settings`. |
| `frontend/src/pages/admin/settings_page.tsx` | `Switch` → `Select` con opciones `"all"`/`"pass"`/`"audit"`. Actualizar interface y help text. |

## Tareas

### Tarea 1: Migration SQL — actualizar valor por defecto

**Archivos:**
- Modificar: `backend/migrations/20260903000000_add_log_all_requests_setting.up.sql:1-2`

- [ ] **Paso 1:** Cambiar el valor insertado de `'true'` a `' all'`
      La migración inserta `('log_all_requests', 'true')`. Cambiar a `(''log_all_requests', 'all')`.

### Tarea 2: Modelo Settings — cambiar tipo y parseo

**Archivos:**
- Modificar: `backend/src/models/settings.rs:32, 47, 184, 248`

- [ ] **Paso 1:** Cambiar el campo `log_all_requests: bool` a `log_all_requests: String` en la struct `Settings`
- [ ] **Paso 2:** En `Default::default()`, cambiar `log_all_requests: true` a `log_all_requests: "all".to_string()`
- [ ] **Paso 3:** En `Settings::load()`, reemplazar el parseo a bool por asignación directa del string:
      ```rust
      let log_all_requests = map.remove("log_all_requests").unwrap_or_else(|| "all".to_string());
      ```
- [ ] **Paso 4:** En `Settings::save()`, reemplazar el ternario por el string directo:
      ```rust
      ("log_all_requests", settings.log_all_requests.clone()),
      ```

### Tarea 3: shuul.rs — nuevo gating con LogLevel helper

**Archivos:**
- Modificar: `backend/src/http/shuul.rs:91`

- [ ] **Paso 1:** Añadir al inicio del archivo (o a `models/settings.rs`) la función helper:

      ```rust
      /// Determina si una categoría de log debe registrarse según el modo actual.
      fn should_log(mode: &str, category: &str) -> bool {
          match mode {
              "all" => true,
              "pass" => matches!(category, "pass"),
              _ => false, // "audit" — nunca logea eventos no-audit
          }
      }
      ```

- [ ] **Paso 2:** Reemplazar los 6 usos de `if log_all_requests` por `if should_log(&log_all_requests, "<category>")`:

      | Línea actual | Category |
      |---|---|
      | `if log_all_requests { audit_log!("safe_path", ...) }` (línea 97) | `"safe_path"` |
      | `if log_all_requests { audit_log!("trusted_ip", ...) }` (línea 121) | `"trusted_ip"` |
      | `if log_all_requests { audit_log!("trusted_ua", ...) }` (línea 143) | `"trusted_ua"` |
      | `if log_all_requests { audit_log!("log_only", ...) }` (línea 228) | `"log_only"` |
      | `if log_all_requests { audit_log!("pass", ...) }` (línea 269) | `"pass"` |
      | `if log_all_requests { audit_log!("allow", ...) }` (línea 295) | `"allow"` |

- [ ] **Paso 3:** Verificar que `audit_log!("banned"...)` (línea 179) y `audit_log!("block"...)` (línea 312) **NO** tienen gate — deben loguearse siempre.

### Tarea 4: report.rs — nuevo gating con LogLevel helper

**Archivos:**
- Modificar: `backend/src/http/report.rs:55-57`

- [ ] **Paso 1:** Reemplazar los 4 usos de `log_all_requests` por `should_log`:

      | Línea actual | Category |
      |---|---|
      | `if log_all_requests { audit_log!("report_received", ...) }` (línea 59) | `"report_received"` |
      | `if log_all_requests { audit_log!("report_ok", ...) }` (línea 124) | `"report_ok"` |
      | `if log_all_requests { audit_log!("report_match", ...) }` (línea 143) | `"report_match"` |
      | `if log_all_requests { audit_log!("report_warn", ...) }` (línea 170) | `"report_warn"` |

- [ ] **Paso 2:** Verificar que `audit_log!("report_block"...)` (línea 195) y `audit_log!("report_ban"...)` (línea 228) **NO** tienen gate — deben loguearse siempre.

### Tarea 5: HTTP handlers de settings — cambiar tipos y validar

**Archivos:**
- Modificar: `backend/src/http/settings.rs:22, 33, 125-127`

- [ ] **Paso 1:** En `SettingsResponse`, cambiar `log_all_requests: bool` a `log_all_requests: String`
- [ ] **Paso 2:** En `UpdateSettingsPayload`, cambiar `log_all_requests: Option<bool>` a `log_all_requests: Option<String>`
- [ ] **Paso 3:** En `update_settings()`, añadir validación del valor antes de asignar (después de la validación de `default_rule_mode`):
      ```rust
      if let Some(ref val) = update.log_all_requests {
          match val.as_str() {
              "all" | "pass" | "audit" => {},
              _ => {
                  return Err(AppError::InvalidInput(
                      "log_all_requests must be 'all', 'pass', or 'audit'".to_string(),
                  ));
              },
          }
      }
      ```

- [ ] **Paso 4:** En la asignación (línea 125-127), cambiar a:
      ```rust
      if let Some(val) = update.log_all_requests {
          settings.log_all_requests = val;
      }
      ```

### Tarea 6: Frontend — de Switch a Select

**Archivos:**
- Modificar: `frontend/src/pages/admin/settings_page.tsx:9, 15, 30, 156, 185, 212-219`

- [ ] **Paso 1:** Cambiar la interfaz `Settings` (línea 9-16):
      ```typescript
      interface Settings {
          safe_paths: string[];
          trusted_ips: string[];
          trusted_user_agents: string[];
          default_rule_mode: string;
          log_retention_days: number;
          log_all_requests: string; // "all" | "pass" | "audit"
      }
      ```

- [ ] **Paso 2:** Cambiar `DEFAULT_SETTINGS` (línea 30):
      ```typescript
      log_all_requests: "all",
      ```

- [ ] **Paso 3:** Cambiar la firma de `handleGeneralSave` (línea 156):
      ```typescript
      private handleGeneralSave = (values: { default_rule_mode: string; log_retention_days: number; log_all_requests: string }) => {
      ```

- [ ] **Paso 4:** Reemplazar el `<Switch>` (líneas 212-219) por un `<Select>`:
      ```tsx
      <Form.Item
          label="Log Level"
          name="log_all_requests"
          help="Controls which events are logged. 'All' logs everything, 'Pass Only' logs only requests that pass without matching any rule, 'Audit Only' logs only blocks, bans, and enforcement actions."
      >
          <Select
              options={[
                  { value: 'all', label: 'All' },
                  { value: 'pass', label: 'Pass Only' },
                  { value: 'audit', label: 'Audit Only' },
              ]}
              style={{ width: 200 }}
          />
      </Form.Item>
      ```

- [ ] **Paso 5:** Eliminar `valuePropName="checked"` del `Form.Item` (incompatible con Select)

- [ ] **Paso 6:** Verificar que la importación de `Switch` ya no sea necesaria. Si `Switch` solo se usaba ahí, eliminarla de lalínea 2:
      ```typescript
      import { Card, Form, InputNumber, Input, Select, Button, Typography, message, Flex, Tabs, Tag } from 'antd';
      ```
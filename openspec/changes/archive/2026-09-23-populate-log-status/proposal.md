# Populate `status_code` in Jail audit logs

## Intent

Añadir el campo `status_code` a las llamadas `audit_log!` del pipeline Jail (`report.rs`) que actualmente no lo incluyen, para que el Log Viewer del frontend muestre el código HTTP real en la columna "Status".

## Scope

Solo `backend/src/http/report.rs`. No se modifica `shuul.rs` ni el frontend.

## Impact

- El `LogEntry` ya tiene el campo `status_code: Option<i32>` definido
- El frontend (`LogsPage`) ya renderiza la columna "Status" con `status_code`
- Solo falta pasar el valor desde las llamadas `audit_log!` en `report.rs`
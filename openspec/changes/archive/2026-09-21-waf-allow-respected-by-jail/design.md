# Design: WAF Allow Rules Respected by Jail Pipeline (B2)

## Overview

El Jail pipeline actual banea IPs directamente cuando se excede un threshold de rate limit. Esto permite que una IP sea baneada aunque el WAF pipeline la haya permitido explícitamente con una regla `allow=true`, porque el Jail pipeline no tiene conocimiento de la decisión del WAF.

La solución (B2) separa la responsabilidad: el Jail pipeline **marca** IPs como candidatas a ban, y el WAF pipeline **ejecuta** el ban. Como el WAF pipeline solo verifica candidatos cuando no hay match de WAF rules, las reglas `allow=true` se respetan automáticamente.

## Arquitectura

```
AppState:
  pending_bans: Mutex<Vec<PendingBan>>   ← nueva estructura

Jail pipeline (report.rs):
  Reporte → Match jail rules → rl.record(ip)
                                    │
                             ┌──────┴──────┐
                             │  threshold?  │
                             └──────┬──────┘
                                    │ sí
                                    ▼
                            push PendingBan { ip, rule_id, ... }
                            audit_log!("pending", ...)
                                    │
                                    ▼
                                200 OK

WAF pipeline (shuul.rs):
  Request → Match WAF rules
                │
         ┌──────┴──────┐
         │  allow=true? │── sí ──► 200 OK (fin)
         └──────┬──────┘
                │ no (allow=false o no match)
                ▼
         ┌──────────────┐
         │ allow=false  │──► 403 (fin, no llega al backend)
         └──────┬───────┘
                │ no match
                ▼
         Check ban_manager
                │
         ┌──────┴──────┐
         │  baneada?    │── sí ──► audit_log!("banned", ...) → 403
         └──────┬──────┘
                │ no
                ▼
         Check pending_bans
                │
         ┌──────┴──────┐
         │  hay match?  │── sí ──► ban_manager.ban()
         │              │         audit_log!("sanctioned", ...)
         │              │         → 403
         └──────┬──────┘
                │ no
                ▼
           200 OK
```

## Estructuras de datos

```rust
/// Pendiente de ban que el Jail pipeline marca y el WAF pipeline ejecuta
#[derive(Debug, Clone)]
struct PendingBan {
    ip: IpAddr,
    rule_id: i32,
    reason: String,
    ban_duration_seconds: Option<i64>,
    escalation_level: i32,
    created_at: Instant,
}

// En AppState:
pending_bans: Mutex<Vec<PendingBan>>,
```

## Eventos de auditoría

| Pipeline | Evento | Cuándo |
|---|---|---|
| WAF | `admitted` | WAF rule allow o log_only |
| WAF | `denied` | WAF rule deny |
| WAF | `banned` | IP ya baneada en ban_manager |
| WAF | `sanctioned` | PendingBan ejecutado → ban_manager.ban() |
| WAF | `unmatched` | No match WAF, no ban |
| Jail | `cleared` | Sin match jail o status ∉ fail_codes |
| Jail | `registered` | status ∈ fail_codes, rl.record(ip), sin threshold |
| Jail | `pending` | Threshold → PendingBan creado, pendiente de ejecutar |

## Cambios necesarios

### `backend/src/models/mod.rs` (AppState)
- Añadir `PendingBan` struct
- Añadir `pending_bans: Mutex<Vec<PendingBan>>` a AppState

### `backend/src/http/report.rs` (Jail pipeline)
- Eliminar toda la lógica de ban (`ban_manager.ban()`, `persist_ban()`)
- Cuando `rl.record(ip)` devuelve `true`:
  - Crear `PendingBan` y pushearlo a `app_state.pending_bans`
  - Audit log con evento `pending` (antes `sanctioned`)
- Añadir `"pending"` a `should_log` para modo audit

### `backend/src/http/shuul.rs` (WAF pipeline)
- En el bloque "No WAF rule matched", después del ban_manager check:
  - Iterar `pending_bans` para la IP
  - Para cada uno, verificar si request matchea la regla (`CacheRule::matches()`)
  - Si matchea → `ban_manager.ban()` + `persist_ban()` + audit_log `sanctioned`
- `should_log` ya incluye `"sanctioned"` en modo audit

### Background task
- Añadir cleanup de `PendingBan` expirados (default: 60s) en el loop de tareas periódicas

## Concurrencia

- `pending_bans` se accede con `Mutex`, siguiendo el mismo patrón que `rate_limiter` y `ban_manager`
- Orden de locks: `rules → pending_bans → rate_limiter → ban_manager`
- Todos los locks se liberan antes de cualquier `.await`

## Open Questions

- ¿Debemos limpiar `PendingBan` después de ejecutarlo? Sí, para evitar ejecutarlo múltiples veces.
- Timeout de cleanup: 60 segundos parece razonable (si el WAF pipeline no ve a la IP en 60s, el pending ban expira).
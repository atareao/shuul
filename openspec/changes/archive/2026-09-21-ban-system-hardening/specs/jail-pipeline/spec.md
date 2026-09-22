## ADDED Requirements

### Requirement: is_banned per-rule en Jail pipeline

El Jail pipeline debe verificar si el IP está baneado para la **regla específica** que está procesando, no para cualquier regla. Esto evita que un ban por regla A impida el ban por regla B.

#### Contract: Cambio en report_handler

```rust
// ANTES (report.rs ~line 298):
if ban_manager.is_banned(&ip).is_some() {
    continue;  // Salta TODAS las reglas restantes
}

// DESPUÉS:
if ban_manager.is_banned_for_rule(&ip, Some(*rule_id)) {
    continue;  // Solo salta esta regla
}
```

#### Scenario: IP baneada por regla A permite ban por regla B
- **GIVEN** un reporte con IP `1.2.3.4` y status_code `404`
- **AND** el IP tiene un ban activo para `rule_id=1` (Auth Brute Force)
- **AND** el reporte matchea dos reglas Jail: rule_id=1 (Auth Brute Force) y rule_id=3 (Path Scanning)
- **WHEN** el Jail pipeline procesa el reporte
- **THEN** para `rule_id=1`: `is_banned_for_rule(&ip, Some(1))` retorna `true` → skip (continue)
- **AND** para `rule_id=3`: `is_banned_for_rule(&ip, Some(3))` retorna `false` → procesa rate limiting normalmente
- **AND** si el threshold se excede para rule_id=3, la IP es baneada también para esa regla

#### Scenario: IP baneada por regla A no bloquea regla A (misma regla)
- **GIVEN** un reporte con IP `1.2.3.4` y status_code `404`
- **AND** el IP tiene un ban activo para `rule_id=3` (Path Scanning)
- **AND** el reporte matchea la regla rule_id=3
- **WHEN** el Jail pipeline procesa el reporte
- **THEN** `is_banned_for_rule(&ip, Some(3))` retorna `true` → skip (continue)
- **AND** no se evalúa rate limiting para rule_id=3

#### Scenario: IP sin bans procesa todas las reglas
- **GIVEN** un reporte con IP `1.2.3.4` y status_code `404`
- **AND** el IP no tiene ningún ban activo
- **AND** el reporte matchea 3 reglas Jail
- **WHEN** el Jail pipeline procesa el reporte
- **THEN** las 3 reglas se evalúan sin skip
- **AND** cada regla que excede threshold banea la IP para esa regla
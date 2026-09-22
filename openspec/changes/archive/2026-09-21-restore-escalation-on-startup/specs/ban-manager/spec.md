## ADDED Requirements

### Requirement: load_from_db restaura escalation_counts

`BanManager::load_from_db()` debe restaurar el mapa `escalation_counts` con el máximo `escalation_level` por IP encontrado en los bans activos cargados de la BD.

#### Contract: Cambio en load_from_db

```rust
// Dentro de load_from_db(), después del loop que carga los bans:
// Para cada IP, calcular el escalation_level máximo y restaurarlo en escalation_counts
for row in &rows {
    let ip: IpAddr = row.get("ip_address").parse()?;
    let escalation_level: i32 = row.get("escalation_level");
    // ... existing BanInfo creation ...
    
    // NUEVO: restaurar escalation_counts con el nivel máximo histórico
    let entry = manager.escalation_counts.entry(ip).or_insert_with(|| (0, Instant::now()));
    if (escalation_level as u32) > entry.0 {
        entry.0 = escalation_level as u32;
    }
}
```

#### Scenario: load_from_db restaura escalation_counts al nivel máximo

- **GIVEN** la tabla `bans` tiene 2 filas activas para `ip_address="1.2.3.4"`: una con `rule_id=1` y `escalation_level=5`, otra con `rule_id=2` y `escalation_level=3`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene una entrada para IP `1.2.3.4` con nivel `5` (el máximo)
- **AND** el próximo `ban(ip, Some(3), ...)` crea un BanInfo con `escalation_level=5` (no 0)

#### Scenario: load_from_db con IP sin bans no crea entradas en escalation_counts

- **GIVEN** la tabla `bans` está vacía
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` está vacío

#### Scenario: load_from_db con IP baneada una sola vez

- **GIVEN** la tabla `bans` tiene 1 fila activa para `ip_address="1.2.3.4"` con `escalation_level=7`
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene una entrada para IP `1.2.3.4` con nivel `7`

#### Scenario: load_from_db con múltiples IPs

- **GIVEN** la tabla `bans` tiene filas activas para IPs `1.2.3.4` (level 5) y `5.6.7.8` (level 3)
- **WHEN** se llama `load_from_db(pool)`
- **THEN** `manager.escalation_counts` contiene entradas para ambas IPs con sus respectivos niveles máximos
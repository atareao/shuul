# Design

## Context

Ver proposal.md para motivación. El archivo `backend/src/templates.rs` contiene 2470 líneas con 81 `RuleTemplate` structs literales, cada uno repitiendo 14 campos `Option<String>` con `None`. Los 46 templates JAIL son esencialmente el mismo patrón (path regex + profile_id) repetido para diferentes CMS. El frontend `templates_page.tsx` (1137 líneas) renderiza estos templates con categorías, severidades e iconos.

## Goals / Non-Goals

**Goals:**
- Reducir templates.rs de 2470 a ~700 líneas
- Eliminar redundancia estructural (builder pattern)
- Eliminar redundancia conceptual (fusionar templates CMS-específicos en genéricos)
- Ordenar templates por prioridad (más necesario primero)
- Corregir inconsistencias allow/pipeline
- Mantener API contract idéntico

**Non-Goals:**
- NO cambiar el modelo de datos `RuleTemplate` ni `RateLimitProfileTemplate`
- NO cambiar la API `/api/v1/templates`
- NO cambiar el pipeline WAF/Jail en `shuul.rs` o `report.rs`
- NO cambiar los rate limit profiles existentes

## Decisions

### 1. Builder pattern para RuleTemplate
**Decisión**: Crear `RuleTemplate::waf()` y `RuleTemplate::jail()` como constructores base, más métodos encadenados para cada campo.

**Alternativa considerada**: Macro `rule_template!{}`. Se descarta porque el builder es más legible, tipado y fácil de mantener.

**Ejemplo**:
```rust
// Antes: 25 líneas
RuleTemplate {
    name: "SQL injection probes".into(),
    description: "Detecta...".into(),
    category: "seguridad".into(),
    severity: "🔴 Alto".into(),
    ip_address: None, protocol: None, fqdn: None,
    path: None, query: Some(r"...".into()),
    city_name: None, country_name: None, country_code: None,
    user_agent: None, method: None, referer: None,
    content_type: None, accept_language: None, x_request_id: None,
    allow: false, pipeline: "waf".into(),
    rate_limit_profile_id: None, rate_limit_profile_name: None,
    requires_fqdn: false, must_have: true, weight: 100,
}

// Después: 8 líneas
RuleTemplate::waf("SQL injection probes", "Detecta...")
    .category("seguridad")
    .severity("alto")
    .query(r"(?i)(union\s+select|...)")
    .must_have(true)
    .weight(100)
```

### 2. Severidad simplificada
**Decisión**: Usar strings cortos (`"critico"`, `"alto"`, `"medio"`, `"bajo"`) en Rust y mapear a emojis en el frontend.

**Alternativa**: Mantener emojis en Rust. Se descarta porque mezcla presentación con datos.

### 3. Orden de templates por prioridad
**Decisión**: Los templates se listan en orden de más necesario a menos necesario dentro de cada pipeline. En WAF, los `must_have` van primero (pesos bajos), luego los específicos. En JAIL, los catch-all/scanners van primero, luego los de protección de servicios.

### 4. Corrección de inconsistencias
- PHPUnit probe (#72): `allow: false` + `pipeline: jail` → mover a WAF con `allow: false`
- Drupal xmlrpc (#73): `allow: false` + `pipeline: jail` → mover a WAF con `allow: false`
- WordPress xmlrpc (#52): `allow: false` + `pipeline: jail` → mover a WAF con `allow: false`

### 5. Pesos alineados con weight ASC
- **WAF must-have**: weight 10-100 (evalúan primero)
- **WAF específicos**: weight 100-300
- **JAIL catch-all/scanners**: weight 10-100
- **JAIL service protection**: weight 100-200
- **JAIL nicho**: weight 200-300

## Risks / Trade-offs

- **Riesgo**: Al fusionar templates CMS-específicos en genéricos, algunos usuarios podrían perder granularidad
  - **Mitigación**: Los templates genéricos cubren patrones más amplios (ej: `login` cubre wp-login, admin, user, etc.)
- **Riesgo**: Cambiar pesos existentes podría alterar orden de evaluación para reglas ya aplicadas
  - **Mitigación**: Los pesos solo afectan templates nuevos; reglas existentes mantienen su peso
- **Riesgo**: Frontend necesita actualizar mapeo de severidades (emojis → strings cortos)
  - **Mitigación**: El frontend ya tiene `SEVERITY_COLORS` map; se añade un paso de traducción
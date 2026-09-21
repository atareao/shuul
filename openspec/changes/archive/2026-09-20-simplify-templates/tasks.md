# Tasks

## 1. Builder pattern y estructura base

- [x] 1.1 Implementar `RuleTemplate::waf()` y `RuleTemplate::jail()` constructores base con valores por defecto (allow, pipeline, todos los filtros a None)
- [x] 1.2 Implementar métodos encadenados: `.category()`, `.severity()`, `.path()`, `.query()`, `.country_code()`, `.user_agent()`, `.method()`, `.ip_address()`, `.fqdn()`, `.city_name()`, `.country_name()`, `.referer()`, `.content_type()`, `.accept_language()`, `.x_request_id()`, `.allow()`, `.pipeline()`, `.rate_limit_profile()`, `.requires_fqdn()`, `.must_have()`, `.weight()`
- [x] 1.3 Verificar que `cargo build` compila sin errores tras el builder

## 2. WAF templates — Must-have (prioridad 1)

- [x] 2.1 Implementar template "SQL injection probes" con builder, must_have, weight 100
- [x] 2.2 Implementar template "XSS probes" con builder, must_have, weight 200
- [x] 2.3 Implementar template "Command injection probes" con builder, must_have, weight 100
- [x] 2.4 Implementar template "Log4j / Log4Shell" con builder, must_have, weight 10
- [x] 2.5 Implementar template "Path traversal" con builder, must_have, weight 100
- [x] 2.6 Implementar template "Archivos sensibles" (fusión de .env, .bak, .pem, .key, composer.json, .DS_Store, .swp) con builder, must_have, weight 100
- [x] 2.7 Implementar template "Directorios de sistema" (fusión de vendor, node_modules, .git, .env) con builder, must_have, weight 10
- [x] 2.8 Implementar template "Docker socket exposure" con builder, must_have, weight 10
- [x] 2.9 Implementar template "phpinfo exposure" con builder, must_have, weight 100
- [x] 2.10 Implementar template "User-Agent vacío" con builder, must_have, weight 200
- [x] 2.11 Implementar template "User-Agent bots conocidos" con builder, must_have, weight 200
- [x] 2.12 Implementar template "Apache/Nginx status" con builder, must_have, weight 100
- [x] 2.13 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 3. WAF templates — Específicos (prioridad 2)

- [x] 3.1 Implementar template "WordPress uploads" (wp-content/uploads/*.php) con builder, weight 100
- [x] 3.2 Implementar template "Adminer + phpMyAdmin" (fusión de Adminer + phpMyAdmin variants) con builder, weight 50
- [x] 3.3 Implementar template "Laravel" (fusión debug bar + Telescope + Ignition) con builder, weight 100
- [x] 3.4 Implementar template "Swagger / API docs" con builder, weight 200
- [x] 3.5 Implementar template "API admin endpoints" con builder, weight 100
- [x] 3.6 Implementar template "Drupal install" con builder, weight 50
- [x] 3.7 Implementar template "Symfony" (fusión _profiler + _wdt) con builder, weight 100
- [x] 3.8 Implementar template "PrestaShop install" con builder, weight 50
- [x] 3.9 Implementar template "Geo blocking" (fusión países + hosting/VPS) con builder, weight 300
- [x] 3.10 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 4. JAIL templates — Scanners y catch-all (prioridad 1)

- [x] 4.1 Implementar template "Scanner genérico 404" (P3, sin path filter, must_have) con builder, weight 9998
- [x] 4.2 Implementar template "Scanner vulnerabilidades web" (P9, shells, exploits, CVE) con builder, must_have, weight 10
- [x] 4.3 Implementar template "Scanner directorios+configs+paneles" (P3, fusión de #76+#77+#78) con builder, must_have, weight 10
- [x] 4.4 Implementar template "Global Shield catch-all" (P8, sin filtros) con builder, weight 9999
- [x] 4.5 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 5. JAIL templates — Protección de servicios (prioridad 2)

- [x] 5.1 Implementar template "Login endpoints" (P1, genérico: login, signin, auth, wp-login, user) con builder, weight 50
- [x] 5.2 Implementar template "Admin panels" (P1, genérico: admin, dashboard, wp-admin) con builder, weight 100
- [x] 5.3 Implementar template "xmlrpc" (P2, genérico) con builder, weight 50 — MOVIDO a WAF con allow=false
- [x] 5.4 Implementar template "Paneles de gestión" (P2, phpmyadmin, cpanel, jenkins, portainer, pgadmin) con builder, weight 50
- [x] 5.5 Implementar template "Auth endpoints sensibles" (P2, register, signup, reset, forgot, oauth, callback) con builder, weight 100
- [x] 5.6 Implementar template "API REST" (P4, genérico: /api/, /wp-json/, /rest/) con builder, weight 200
- [x] 5.7 Implementar template "GraphQL" (P4) con builder, weight 200
- [x] 5.8 Implementar template "Search endpoints" (P4) con builder, weight 200
- [x] 5.9 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 6. JAIL templates — Escenarios específicos (prioridad 3)

- [x] 6.1 Implementar template "File upload abuse" (P2) con builder, weight 100
- [x] 6.2 Implementar template "Comment / form spam" (P5) con builder, weight 200
- [x] 6.3 Implementar template "Webmail" (P1, roundcube, webmail, rainloop) con builder, weight 50
- [x] 6.4 Implementar template "Scraping por país" (P5) con builder, weight 200
- [x] 6.5 Implementar template "Health & Webhooks" (P6, NUEVO) con builder, weight 100
- [x] 6.6 Implementar template "Recidive" (P7, NUEVO, sin filtros) con builder, weight 9997
- [x] 6.7 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 7. Corrección de inconsistencias

- [x] 7.1 Mover PHPUnit probe (#72) de jail a WAF con allow=false
- [x] 7.2 Mover Drupal xmlrpc (#73) de jail a WAF con allow=false (fusionado con xmlrpc genérico)
- [x] 7.3 Mover WordPress xmlrpc (#52) de jail a WAF con allow=false (fusionado con xmlrpc genérico)
- [x] 7.4 Verificar que ningún template JAIL tiene allow=false
- [x] 7.5 Verificar que `cargo test` pasa y `cargo clippy -- -D warnings` no emite errores

## 8. Frontend — Actualizar mapeo de severidad

- [x] 8.1 Actualizar `SEVERITY_COLORS` en `templates_page.tsx` para mapear strings cortos ("critico", "alto", "medio", "bajo") a emojis y colores
- [x] 8.2 Verificar que `npx tsc --noEmit` no emite errores

## 9. Verificación final

- [x] 9.1 Ejecutar `cargo test` y confirmar que todos los tests pasan
- [x] 9.2 Ejecutar `cargo clippy -- -D warnings` y confirmar cero errores
- [x] 9.3 Ejecutar `cargo fmt --check` y confirmar formato correcto
- [x] 9.4 Ejecutar `npx tsc --noEmit` en frontend y confirmar cero errores
- [x] 9.5 Verificar que la API `/api/v1/templates` devuelve 23 WAF + 18 JAIL + 9 profiles
# Changelog
## [0.19.1] - 2026-10-24

### Bug Fixes

- *(templates)* Los tags de filtro truncan con elipsis y el valor íntegro va en un tooltip

### Documentation

- *(openspec)* Archiva los siete changes publicados y pliega sus deltas en las specs
- *(openspec)* Redacta el Purpose de las tres specs creadas por el archive
- *(template-filters-overflow)* Spec, proposal y ledger del change
## [0.19.0] - 2026-10-24

### Documentation

- *(jail-weight-coherence)* Proposal, deltas de spec y tasks
- *(agents)* El peso de una regla de Jail es siempre 0
- *(jail-weight-coherence)* El rechazo del pipeline inválido y el legado con otra capitalización
- *(jail-weight-coherence)* Marca las tareas cerradas del REFACTOR

### Features

- *(jail-weight-coherence)* Deja de arrastrar un peso para Jail (GREEN)
- *(jail-weight-coherence)* Normaliza el peso de las reglas de Jail (GREEN)
- *(jail-weight-coherence)* La UI nombra las reglas rechazadas al importar (REFACTOR)

### Miscellaneous Tasks

- Release v0.19.0

### Refactor

- *(jail-weight-coherence)* Centraliza la comparación con el pipeline jail (REFACTOR)
- *(jail-weight-coherence)* Normaliza también la etiqueta del pipeline legado (REFACTOR)
- *(jail-weight-coherence)* Acota el aviso de rechazos y endurece la tolerancia (REFACTOR)

### Testing

- *(jail-weight-coherence)* Añade los tests del peso de Jail (RED)
- *(frontend)* Fija que el Weight no se ofrece para Jail (RED)
## [0.18.1] - 2026-10-23

### Documentation

- *(openspec)* Propone secret-hygiene (el secreto nunca al log, aviso si es débil)

### Miscellaneous Tasks

- Release v0.18.1

### Testing

- *(frontend)* Da margen a los tests que ensanchan el catálogo
## [0.18.0] - 2026-10-23

### Documentation

- *(openspec)* Propone pagination-defaults (10 filas por defecto y rangos 10/50/100)
- *(openspec)* Propone ban-row-identity (identidad de fila en la tabla de baneos)

### Features

- *(frontend)* Arranca las tablas en 10 filas con los rangos 10/50/100
- *(frontend)* Identifica cada fila de la tabla de baneos por su partición

### Miscellaneous Tasks

- Release v0.18.0
## [0.17.0] - 2026-10-22

### Bug Fixes

- *(frontend)* Corrige el tipo de FILTER_DETAIL_FIELDS que rompía el build

### Documentation

- *(plan)* Registra el cierre de la fase 0, los changes del 21-22 y abre la FASE 7
- *(openspec)* Propone admin-ui-fixes (paginación, preferencias del Log Viewer, retirada del gráfico)
- *(openspec)* Propone templates-listing (listado único filtrable, perfiles fuera)
- *(openspec)* Propone tcp-url-alias (aceptar tcp:// como sinónimo de http://)

### Features

- *(frontend)* CustomTable como único paginador, preferencias del Log Viewer sin URL y retirada del gráfico Evolution by Method
- *(templates)* Listado único filtrable y perfiles fuera de la API
- *(backend)* Acepta tcp:// como sinónimo de http:// en las URL de transporte

### Miscellaneous Tasks

- *(frontend)* Declara @testing-library/react y user-event, que estaban en uso sin declarar
- *(frontend)* Declara @testing-library/react y user-event, que el test importa
- Release v0.17.0

### Styling

- *(frontend)* Rejilla estable en la barra de filtros de templates

### Testing

- *(spec)* Archiva ban-manager-persistence y consolida la spec
## [0.16.1] - 2026-10-22

### Bug Fixes

- *(matcher)* Un campo restringido sin valor ya no casa por ausencia
- *(ban-manager)* Un baneo sobrevive al reinicio
- *(review)* Corrige los hallazgos de la auditoría

### Documentation

- *(openspec)* Cierra la fase 0 del transporte de logs por la API de Docker
- *(openspec)* Propuesta del change missing-header-fields
- *(openspec)* Documenta los campos de cabecera y cierra el change
- *(openspec)* Propuesta del change ban-manager-persistence
- *(openspec)* Aprueba el change ban-manager-persistence con B2
- *(ban-manager)* Documenta el contrato de persistencia y cierra el change

### Miscellaneous Tasks

- Release v0.16.1

### Testing

- *(spec)* Archiva docker-logs-transport en openspec/specs/
- *(spec)* Archiva missing-header-fields en openspec/specs/
## [0.16.0] - 2026-10-21

### Bug Fixes

- *(jail)* Mostrar el endpoint efectivo y avisar del path base ignorado

### Documentation

- *(plan)* Plan de deuda técnica y decisiones de la sesión
- *(plan)* Registrar fases 1, 2 y 5 hechas, la investigación de 3 y 4, y las medidas de tower-http
- *(plan)* Registrar las fases 3 y 4 con sus medidas
- *(plan)* Cerrar la fase 6 y corregir la versión del motor de §0.2
- *(openspec)* Add docker-logs-transport change proposal
- *(jail)* Documentar el transporte de logs por la API de Docker
- *(jail)* Alinear las referencias a gatewayd fuera de §2
- *(openspec)* Declarar 4.8 y 6.7 y el escenario del path base
- *(openspec)* Cerrar 4.8 y 4.9
- *(jail)* Describir los dos backends en el comentario de módulo
- *(jail)* Desambiguar los títulos de §2 y acotar el alcance del no-replay
- *(openspec)* Cerrar el registro de tasks del change
- *(plan)* Cerrar las tareas hechas de la fase 0

### Features

- *(jail)* Implementar la precedencia, la petición y el endpoint del backend de Docker
- *(jail)* Decodificar los frames de la API de Docker
- *(jail)* Entregar las cargas de Docker al pipeline Jail
- *(jail)* Avisar del backend de Docker no disponible
- *(jail)* Nombrar la causa del error de la API de Docker
- *(jail)* Acotar la longitud declarada en la cabecera de un frame

### Miscellaneous Tasks

- *(docker)* Build the frontend on node 22
- Release v0.16.0

### Other

- *(dependabot)* Keep the react-refresh lint plugin out of the npm group

### Refactor

- *(jail)* Cerrar los cabos del RED y el gate del CI

### Testing

- *(jail)* Parsear la línea de la wire en el fixture de since
## [0.15.4] - 2026-10-20

### Documentation

- *(git-flow)* Fix release PR direction and warn against --delete-branch

### Miscellaneous Tasks

- Release v0.15.4

### Other

- *(release)* Fail with the cause when develop is missing
- *(dependabot)* Target develop and group patch and minor bumps
## [0.15.3] - 2026-10-20

### Bug Fixes

- *(jail)* Preserve the JournalError source chain (M-7)

### Documentation

- *(jail)* Complete end-to-end journal setup guide
- *(jail)* Document the no-replay startup verification
- *(openspec)* Fill the TBD purposes

### Miscellaneous Tasks

- *(openspec)* Archive journald-no-history-replay
- *(openspec)* Archive journal-error-source-chain
- Sync Cargo.lock with 0.15.2
- Release v0.15.3

### Refactor

- *(jail)* Apply the M-7 audit findings
## [0.15.2] - 2026-10-19

### Bug Fixes

- *(jail)* Never feed the journal backlog to the rate limiter

### Miscellaneous Tasks

- Release v0.15.2
## [0.15.1] - 2026-10-19

### Miscellaneous Tasks

- Release v0.15.1
## [0.15.0] - 2026-10-18

### Features

- *(jail)* Read Traefik access logs over the gatewayd Unix socket

### Miscellaneous Tasks

- Release v0.15.0
## [0.14.2] - 2026-10-18

### Bug Fixes

- *(ci)* Stop release-prepare from bumping on a re-run
- *(ci)* Stop release-prepare from bumping on a re-run (#46)
- *(ci)* Make vampus work in CI and fail loudly when it does not
- *(ci)* Make vampus work in CI and fail loudly when it does not (#49)

### Documentation

- Repair historical release tags and their changelog sections

### Miscellaneous Tasks

- Release v0.14.1
- Release v0.14.2

### Other

- Publish the container image to GHCR instead of Docker Hub
- Publish the container image to GHCR instead of Docker Hub (#47)
## [0.14.1] - 2026-10-18

### Documentation

- *(spec)* Add frontend-lint-cleanup change proposal

### Refactor

- *(frontend)* Fix ESLint errors in common utils, contexts and layouts
- *(frontend)* Fix ESLint errors in CustomTable and dialogs
- *(frontend)* Fix ESLint errors in admin and public pages
- *(frontend)* Fix 114 pre-existing ESLint errors (#44)

### Testing

- *(spec)* Record frontend lint baseline (114 errors, tests+build green)
- *(spec)* Mark frontend-lint-cleanup tasks complete and archive
## [0.14.0] - 2026-10-18

### Bug Fixes

- *(logs)* Show FQDN column, remove timestamp, add ellipsis
- *(report)* Populate status_code in jail audit logs
- *(report)* Populate status_code in jail audit logs

### Features

- *(charts)* Show rule name instead of rule ID in Top Rules chart
- *(charts)* Show rule name instead of rule ID in Top Rules chart
- *(jail)* Feed the Jail pipeline from systemd-journal-gatewayd
- *(jail)* Feed the Jail pipeline from systemd-journal-gatewayd (#41)

### Other

- Add gitflow automation workflows and GIT_FLOW.md
## [0.13.0] - 2026-09-22

### Refactor

- Fix syntax errors and refactor tests for PendingBan/TorService (#32)
## [0.12.0] - 2026-09-21

### Bug Fixes

- *(ban-manager)* Restore escalation_counts on startup from DB
- *(ban-manager)* Restore escalation_counts on startup from DB

### Features

- Replace duplicate bans for same IP+rule_id (#28)
## [0.11.0] - 2026-09-08

### Bug Fixes

- *(charts)* Use scale.color.range for evolution chart colors
- *(charts)* Use colorField + group instead of seriesField + dodgeX for evolution chart colors
- Add trailing-slashless routes for DELETE/POST/PATCH on bans, rules, and rate-limit-profiles
- Add trailing-slashless routes for POST/PATCH/DELETE on bans, rules, rate-limit-profiles

### Documentation

- Rewrite documentation after major architecture changes

### Features

- Add in-memory log viewer with configurable ring buffer
- Improve Log Viewer with toggle filters, fix CustomTable and chart colors (#23)

### Refactor

- Fix clippy warnings and improve code quality across backend
- Fix clippy warnings and improve code quality across backend
## [0.10.0] - 2026-09-05

### Bug Fixes

- *(charts)* Use colorField + group instead of seriesField + dodgeX for evolution chart colors

### Features

- Update rate limit profiles and add scanner/global templates
- Add three-mode log level (all/pass/audit) and fix noisy GeoIP debug logs
- Add three-mode log level (all/pass/audit)

### Miscellaneous Tasks

- Bump version to 0.8.0 and update CHANGELOG
- Add cliff.toml and regenerate CHANGELOG.md

### Performance

- Reduce CPU usage in WAF pipeline with no active rules
- Add GeoIP cache (LRU+TTL) to avoid repeated MaxMind lookups

### Refactor

- Improve frontend UX, auth flow, and component patterns
## [0.8.0] - 2026-09-03

### Bug Fixes

- *(frontend)* Move Method/UA/ContentType to Request tab, Referer to Network
- *(frontend)* Add missing ruleName/ruleDescription/ruleWeight/ruleActive/ruleStore to State interface and constructor

### Documentation

- Add WAF/Jail architecture doc and update project docs

### Features

- *(backend)* Add pipeline field to rules for WAF/Jail separation
- *(backend)* Serve templates from database instead of hardcoded file
- *(frontend)* Update models for pipeline field and DB-backed templates
- *(frontend)* Rewrite templates page with DB-backed data and 3-tab layout
- *(frontend)* Replace virtual type column with real pipeline column in rules page
- *(frontend)* Redesign rule dialog with 4-tab layout and consistent field styling
- *(frontend)* Add clientFilter, extraHeaderContent, and tag type to CustomTable
- *(backend)* Restore hardcoded templates with pipeline support
- Add `weight` field to RuleTemplate with severity-based defaults
- Add 4 scanner Jail templates + tighten rate limit profiles
- Production deployment config with Traefik, env-based migrations path
- Production deployment config with Traefik

### Other

- *(frontend)* Show clear preview of what will be created in apply modals
- Editable name/description/weight/active/store in apply modal, switches in same row

### Refactor

- Migrate from PostgreSQL to SQLite
- Replace request storage with StatsCollector and remove `store` field
- Retune rate limit profiles for real-world scenarios

### Styling

- Fix missing newline at end of template.rs
- Replace Nivo charts with @ant-design/charts
- Reduce Rate Limit Profiles columns to essentials
- Reorganize Rate Limit Profile dialog into tabs
- Move Name/Description outside tabs in Rate Limit Profile dialog
## [0.6.0] - 2026-09-02

### Bug Fixes

- Persist bans across restarts and fix report rate limiter key

### Features

- Add ellipsis tooltip and column widths to rules and requests tables
- Consolidate schema, status code rate limiting, rules export/import
- Add rule_name to requests and rate_limit_profile_name to rules via LEFT JOIN
- *(frontend)* Update models with rule_name, rate_limit_profile_name, and ban id
- *(frontend)* Add request detail dialog with create rule action
- Pure rate limiters as side-effect rules + cached rate limit profiles (#14)

### Miscellaneous Tasks

- Formatting and misc changes

### Refactor

- Align Rule model with schema, add Settings to AppState, 7 profile templates

### Styling

- *(frontend)* Improve UX - simplify fields, add ellipsis, fix bans page, reorder sidebar
## [0.3.3] - 2026-08-23

### Bug Fixes

- Validate email format and disable registration when SSO is configured
- Validate email format and disable registration when SSO is configured
- Make /users/any public so login page shows sign-in instead of register
- Show only PocketID SSO button when SSO is configured (no local login/register)
- Auto-login after successful registration and show error messages

### Features

- Enforce SSO-only mode - no local login, no local register
- Validate OIDC id_token with JWKS and remove dead code
- Reload rules cache on CRUD, add Create Rule from Request dialog
- Ellipsis tooltip and column widths for rules/requests tables

### Miscellaneous Tasks

- Merge develop into main (SSO-only mode, fixes)
- Bump time from 0.3.44 to 0.3.47
- Bump time from 0.3.46 to 0.3.47 in /backend
- Bump react-router from 7.10.1 to 7.12.0 in /frontend
- Bump react-router from 7.10.1 to 7.12.0 in /frontend
- Bump maxminddb from 0.26.0 to 0.27.0
- Bump maxminddb from 0.26.0 to 0.27.0 in /backend

### Refactor

- SSO-only auth, RwLock thread safety, remove User model
## [0.3.2] - 2026-08-21

### Bug Fixes

- Correct justfile push recipe to use docker.io registry

### Miscellaneous Tasks

- Fix security vulnerabilities
## [0.3.1] - 2026-08-21

### Miscellaneous Tasks

- Fix all 15 Rust compiler warnings
- Fix all 15 Rust compiler warnings
- Fix all 15 Rust compiler warnings
- Bump version to 0.3.1
## [0.3.0] - 2026-08-21

### Features

- Expand rule templates from 9 to 30 services
- Add SSO with PocketID (OIDC)
- Add settings page with retention config and daily cleanup

### Miscellaneous Tasks

- Bump version to 0.3.0
## [0.2.0] - 2026-08-21

### Bug Fixes

- Address code review issues - dialogMessages, type mismatches
- Address code review issues - ban duration override, mutex error logging, CircularTimestamps init

### Features

- Add rate limiting columns to rules and create bans table
- Add CircularTimestamps and RateLimiter modules
- Add BanManager with escalation and decay
- Extend Rule models with rate limiting fields
- Extend shuul pipeline with ban check and rate limiter
- Add ban CRUD endpoints
- Add rule templates catalog and endpoint
- Add background task for ban and rate limiter cleanup
- Extend frontend Rule model and rules page with rate limiting fields
- Add bans page with route and menu item
- Add active bans count to dashboard

### Miscellaneous Tasks

- Bump version to 0.2.0

### Refactor

- Centralize error handling with AppError and convert handlers to Result<>

### Testing

- Add integration tests for rate limiting and bans
## [0.0.1] - 2025-10-06

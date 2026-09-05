# Changelog
## [0.10.0] - 2026-09-05

### Features

- Update rate limit profiles and add scanner/global templates
- Add three-mode log level (all/pass/audit) and fix noisy GeoIP debug logs
- Add three-mode log level (all/pass/audit)

### Miscellaneous Tasks

- Bump version to 0.8.0 and update CHANGELOG

### Other

- V0.9.0
- V0.9.0 (#22)
- V0.10.0 (#24)

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
- V0.7.0 — SQLite migration

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

- Add rule_name to requests and rate_limit_profile_name to rules via LEFT JOIN
- *(frontend)* Update models with rule_name, rate_limit_profile_name, and ban id
- *(frontend)* Add request detail dialog with create rule action
- Pure rate limiters as side-effect rules + cached rate limit profiles (#14)

### Miscellaneous Tasks

- Formatting and misc changes

### Other

- V0.4.0 — UX improvements, ban persistence, logging fixes

### Styling

- *(frontend)* Improve UX - simplify fields, add ellipsis, fix bans page, reorder sidebar
## [0.5.0] - 2026-08-31

### Features

- Consolidate schema, status code rate limiting, rules export/import

### Other

- V0.4.0
- V0.5.0

### Refactor

- Align Rule model with schema, add Settings to AppState, 7 profile templates
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
- Add ellipsis tooltip and column widths to rules and requests tables
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

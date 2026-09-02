## [0.6.0] - 2026-09-02

### 💼 Other

- V0.4.0 — UX improvements, ban persistence, logging fixes
## [0.5.0] - 2026-08-31

### 💼 Other

- V0.4.0
- V0.5.0
## [0.3.3] - 2026-08-23

### 🚀 Features

- Enforce SSO-only mode - no local login, no local register
- Validate OIDC id_token with JWKS and remove dead code
- Reload rules cache on CRUD, add Create Rule from Request dialog
- Add ellipsis tooltip and column widths to rules and requests tables
- Ellipsis tooltip and column widths for rules/requests tables

### 🐛 Bug Fixes

- Validate email format and disable registration when SSO is configured
- Validate email format and disable registration when SSO is configured
- Make /users/any public so login page shows sign-in instead of register
- Show only PocketID SSO button when SSO is configured (no local login/register)
- Auto-login after successful registration and show error messages

### 🚜 Refactor

- SSO-only auth, RwLock thread safety, remove User model

### ⚙️ Miscellaneous Tasks

- Merge develop into main (SSO-only mode, fixes)
- Bump time from 0.3.44 to 0.3.47
- Bump time from 0.3.46 to 0.3.47 in /backend
- Bump react-router from 7.10.1 to 7.12.0 in /frontend
- Bump react-router from 7.10.1 to 7.12.0 in /frontend
- Bump maxminddb from 0.26.0 to 0.27.0
- Bump maxminddb from 0.26.0 to 0.27.0 in /backend
## [0.3.2] - 2026-08-21

### 🐛 Bug Fixes

- Correct justfile push recipe to use docker.io registry

### ⚙️ Miscellaneous Tasks

- Fix security vulnerabilities
## [0.3.1] - 2026-08-21

### ⚙️ Miscellaneous Tasks

- Fix all 15 Rust compiler warnings
- Fix all 15 Rust compiler warnings
- Fix all 15 Rust compiler warnings
- Bump version to 0.3.1
## [0.3.0] - 2026-08-21

### 🚀 Features

- Expand rule templates from 9 to 30 services
- Add SSO with PocketID (OIDC)
- Add settings page with retention config and daily cleanup

### ⚙️ Miscellaneous Tasks

- Bump version to 0.3.0
## [0.2.0] - 2026-08-21

### 🚀 Features

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

### 🐛 Bug Fixes

- Address code review issues - dialogMessages, type mismatches
- Address code review issues - ban duration override, mutex error logging, CircularTimestamps init

### 🚜 Refactor

- Centralize error handling with AppError and convert handlers to Result<>

### 🧪 Testing

- Add integration tests for rate limiting and bans

### ⚙️ Miscellaneous Tasks

- Bump version to 0.2.0
## [0.0.1] - 2025-10-06

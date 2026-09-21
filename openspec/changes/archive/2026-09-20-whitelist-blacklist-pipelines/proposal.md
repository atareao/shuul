# Proposal

## Why

The current WAF pipeline mixes three conceptually distinct concerns in a single `Settings` struct: operational configuration (log retention, default mode), bypass lists (safe paths, trusted IPs, trusted UAs), and security policies. This makes the system harder to reason about, configure, and extend. Separating whitelist and blacklist into first-class pipelines with dedicated tables, APIs, and UI clarifies the security model and matches the mental model of any systems administrator.

## What Changes

- **NEW** `whitelist` table + model + CRUD API + frontend page — entries of type `ip`, `range`, `country`
- **NEW** `blacklist` table + model + CRUD API + frontend page — entries of type `ip`, `range`, `country`
- **MODIFY** WAF pipeline (`shuul.rs`) — new evaluation order: WHITELIST → BLACKLIST → BANNED → WAF rules
- **MODIFY** `Settings` — remove `safe_paths`, `trusted_ips`, `trusted_user_agents` fields
- **REMOVE** `safe_paths` feature entirely (no replacement)
- **REMOVE** `trusted_user_agents` feature entirely (no replacement)
- **MIGRATE** existing `trusted_ips` settings into `whitelist` entries of type `range`
- **MODIFY** `AppState` — add `whitelist` and `blacklist` fields
- **MODIFY** `settings_page.tsx` — remove 3 tabs (Safe Paths, Trusted IPs, Trusted UAs)
- **NEW** `WhitelistPage` and `BlacklistPage` frontend components
- **NEW** sidebar entries for whitelist and blacklist in admin layout

## Capabilities

### New Capabilities

- `whitelist`: IP, CIDR range, and country code whitelisting — entries that bypass all security checks
- `blacklist`: IP, CIDR range, and country code blacklisting — entries that are blocked before any other check
- `waf-pipeline`: The WAF request evaluation pipeline with its four ordered phases

### Modified Capabilities

- *(none — no existing specs to modify)*

## Impact

- **Backend**: New models (`whitelist.rs`, `blacklist.rs`), new HTTP handlers, modified `shuul.rs`, modified `settings.rs`, modified `AppState`, new DB migration
- **Frontend**: New pages, modified `settings_page.tsx`, modified `admin_layout.tsx`, modified `App.tsx`
- **Database**: New `whitelist` and `blacklist` tables; `settings` table retains orphaned keys (ignored by new code)
- **Breaking**: `safe_paths` and `trusted_user_agents` settings are removed — any existing configuration is lost
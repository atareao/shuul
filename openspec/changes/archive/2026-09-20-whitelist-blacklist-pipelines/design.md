# Design

## Overview

Separate the WAF pipeline into four distinct phases (WHITELIST → BLACKLIST → BANNED → WAF) with dedicated whitelist and blacklist tables, models, APIs, and frontend pages.

## Architecture

### New Modules

```
backend/src/models/
├── whitelist.rs    # WhitelistEntry, NewWhitelistEntry, CRUD methods
├── blacklist.rs    # BlacklistEntry, NewBlacklistEntry, CRUD methods

backend/src/http/
├── whitelist.rs    # CRUD handlers for whitelist API
├── blacklist.rs    # CRUD handlers for blacklist API

frontend/src/models/
├── whitelist.ts    # WhitelistEntry interface
├── blacklist.ts    # BlacklistEntry interface

frontend/src/pages/admin/
├── whitelist_page.tsx    # Whitelist CRUD page
├── blacklist_page.tsx    # Blacklist CRUD page
```

### Modified Modules

```
backend/src/models/
├── settings.rs     # Remove safe_paths, trusted_ips, trusted_user_agents
├── mod.rs          # Add whitelist, blacklist modules

backend/src/http/
├── shuul.rs        # New pipeline: WHITELIST → BLACKLIST → BANNED → WAF
├── settings.rs     # Remove whitelist/blacklist fields from SettingsResponse
├── mod.rs          # Add whitelist, blacklist modules

backend/src/main.rs # Add whitelist_router(), blacklist_router() to protected_routes

frontend/src/
├── App.tsx                     # Add routes for /admin/whitelist, /admin/blacklist
├── layouts/admin_layout.tsx    # Add sidebar items
├── pages/admin/settings_page.tsx  # Remove 3 tabs
```

### Data Flow

#### Startup
1. AppState loads whitelist and blacklist from DB (like rules)
2. Each entry precompiles its regex/IpNet for fast matching

#### Request (shuul.rs)
1. Build NewRequest from headers (GeoIP if needed)
2. WHITELIST phase: iterate whitelist entries, check match
   - Match → audit_log + stats + return 200 OK
3. BLACKLIST phase: iterate blacklist entries, check match
   - Match → audit_log + stats + return 403 FORBIDDEN
4. BANNED phase: check BanManager (existing logic)
   - Match → audit_log + stats + return 403 FORBIDDEN
5. WAF phase: match against cached rules (existing logic)
   - Match → audit_log + stats + return 200/403
   - No match → audit_log + stats + return 200 OK

#### CRUD Operations
1. Admin creates/updates/deletes whitelist or blacklist entry via API
2. Handler modifies DB
3. Handler reloads the in-memory list in AppState

### Lock Ordering

Same as existing pattern: all MutexGuard released before any `.await`.
```
whitelist → blacklist → rules → rate_limiter → ban_manager
```

### Migration

New migration `20260920000000_add_whitelist_blacklist.up.sql`:
1. CREATE TABLE whitelist
2. CREATE TABLE blacklist

No data migration. The old `safe_paths`, `trusted_ips`, and `trusted_user_agents` keys in the settings table are simply ignored by the new code (the updated `Settings::load()` no longer looks for them).

### Frontend Components

Both WhitelistPage and BlacklistPage use the existing `CustomTable` + `CustomDialog` pattern:
- CustomTable with columns: id, entry_type (tag), value, description, created_at, actions
- CustomDialog auto-generates form from FieldDefinition
- entry_type rendered as tag (ip=blue, range=green, country=orange)
- Auto-refresh after CRUD operations
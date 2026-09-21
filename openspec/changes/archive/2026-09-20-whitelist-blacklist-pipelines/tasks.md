# Tasks

## Phase 1: Database & Models

- [ ] Create migration `20260920000000_add_whitelist_blacklist.up.sql` with whitelist and blacklist tables
- [ ] Create `backend/src/models/whitelist.rs` with WhitelistEntry, NewWhitelistEntry, CRUD methods
- [ ] Create `backend/src/models/blacklist.rs` with BlacklistEntry, NewBlacklistEntry, CRUD methods
- [ ] Update `backend/src/models/mod.rs` to declare and re-export new modules
- [ ] Update `backend/src/models/settings.rs`: remove safe_paths, trusted_ips, trusted_user_agents fields and all related logic

## Phase 2: HTTP Handlers

- [ ] Create `backend/src/http/whitelist.rs` with CRUD handlers (list, create, update, delete)
- [ ] Create `backend/src/http/blacklist.rs` with CRUD handlers (list, create, update, delete)
- [ ] Update `backend/src/http/mod.rs` to declare and re-export new handler modules
- [ ] Update `backend/src/http/settings.rs`: remove whitelist/blacklist fields from SettingsResponse and UpdateSettingsPayload
- [ ] Update `backend/src/main.rs`: add whitelist_router() and blacklist_router() to protected_routes

## Phase 3: WAF Pipeline

- [ ] Update `AppState` to add `whitelist: Mutex<Vec<WhitelistEntry>>` and `blacklist: Mutex<Vec<BlacklistEntry>>`
- [ ] Add whitelist/blacklist loading at startup (like rules)
- [ ] Modify `shuul.rs`: replace safe_paths/trusted_ips/trusted_ua blocks with WHITELIST → BLACKLIST → BANNED → WAF pipeline
- [ ] Add `whitelist` and `blacklist` categories to `should_log()` function
- [ ] Add whitelist/blacklist reload on CRUD operations

## Phase 4: Frontend

- [ ] Create `frontend/src/models/whitelist.ts` with WhitelistEntry interface
- [ ] Create `frontend/src/models/blacklist.ts` with BlacklistEntry interface
- [ ] Create `frontend/src/pages/admin/whitelist_page.tsx` with CustomTable CRUD
- [ ] Create `frontend/src/pages/admin/blacklist_page.tsx` with CustomTable CRUD
- [ ] Update `frontend/src/pages/admin/settings_page.tsx`: remove 3 tabs (Safe Paths, Trusted IPs, Trusted UAs)
- [ ] Update `frontend/src/App.tsx`: add lazy imports and routes for /admin/whitelist and /admin/blacklist
- [ ] Update `frontend/src/layouts/admin_layout.tsx`: add sidebar items for Whitelist and Blacklist

## Phase 5: Verification

- [ ] Run `cargo check` to verify backend compiles
- [ ] Run `cargo test` to verify all tests pass
- [ ] Run `cargo clippy -- -D warnings` to verify no lint warnings
- [ ] Run `cargo fmt --check` to verify formatting
- [ ] Run frontend build to verify TypeScript compiles
- [ ] Run `openspec validate whitelist-blacklist-pipelines` to validate the change
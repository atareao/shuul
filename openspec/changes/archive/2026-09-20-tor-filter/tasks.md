# Tasks: Tor Network IP Filter

## TDD Checklist

### Phase 1: TorService (RED → GREEN → REFACTOR) ✅

- [x] **RED**: Write tests for TorService
  - [x] Test: `is_exit_node()` returns `None` before first refresh
  - [x] Test: `is_exit_node()` returns `Some(true)` for known Tor IP after refresh
  - [x] Test: `is_exit_node()` returns `Some(false)` for non-Tor IP after refresh
  - [x] Test: `refresh()` handles network failure gracefully (keeps stale data)
- [x] **GREEN**: Implement TorService
  - [x] Create `backend/src/models/tor_service.rs`
  - [x] Implement `new()`, `refresh()`, `is_exit_node()`
  - [x] Parse `torbulkexitlist` format (one IP per line)
  - [x] Use `reqwest::Client` with 10s timeout
- [x] **REFACTOR**: Clean up TorService
  - [x] Run `cargo clippy -- -D warnings`
  - [x] Run `cargo fmt --check`

### Phase 2: Backend models (Rule, CacheRule, NewRequest, NewRule, UpdateRule) ✅

- [x] **RED**: Write tests for model changes
  - [x] Test: `Rule::from_row()` reads `is_tor` correctly
  - [x] Test: `CacheRule::from_rule()` copies `is_tor`
  - [x] Test: `Rule::create()` persists `is_tor`
  - [x] Test: `Rule::update()` updates `is_tor`
  - [x] Test: `matches()` with Tor filter (match, no match, neutral)
- [x] **GREEN**: Implement model changes
  - [x] Add `is_tor: Option<bool>` to `Rule` struct
  - [x] Add `is_tor: Option<bool>` to `CacheRule` struct
  - [x] Add `is_tor: Option<bool>` to `NewRequest` struct
  - [x] Add `is_tor: Option<bool>` to `NewRule` struct
  - [x] Add `is_tor: Option<bool>` to `UpdateRule` struct
  - [x] Update `from_row()` to read `is_tor`
  - [x] Update `from_rule()` to copy `is_tor`
  - [x] Update `create()` SQL and binds
  - [x] Update `update()` SQL and binds
  - [x] Update `matches()` with `check_tor` logic
- [x] **REFACTOR**: Clean up models
  - [x] Run `cargo clippy -- -D warnings`
  - [x] Run `cargo fmt --check`

### Phase 3: NewRequest::from_request() + report.rs integration ✅

- [x] **RED**: Write tests for from_request with TorService
  - [x] Test: `from_request()` with TorService returns `is_tor = Some(true)` for Tor IP
  - [x] Test: `from_request()` without TorService returns `is_tor = None`
  - [x] Test: `from_request()` without IP returns `is_tor = None`
- [x] **GREEN**: Implement from_request changes
  - [x] Add `tor_service: Option<&TorService>` parameter
  - [x] Populate `is_tor` field
  - [x] Update caller in `shuul.rs` (WAF pipeline)
  - [x] Update caller in `report.rs` (Jail pipeline — manual NewRequest construction)
- [x] **REFACTOR**: Clean up
  - [x] Run `cargo clippy -- -D warnings`
  - [x] Run `cargo fmt --check`

### Phase 4: AppState and background task ✅

- [x] **GREEN**: Integrate TorService into AppState
  - [x] Add `tor_service: TorService` to `AppState`
  - [x] Initialize in `main.rs`
  - [x] Spawn background refresh task (30s first delay, then 1800s interval)
- [x] **REFACTOR**: Clean up
  - [x] Run `cargo clippy -- -D warnings`
  - [x] Run `cargo fmt --check`

### Phase 5: Frontend ✅

- [x] **GREEN**: Implement frontend changes
  - [x] Add `is_tor?: boolean` to Rule interface
  - [x] Add `is_tor: false` to DEFAULT_VALUES
  - [x] Add `is_tor` to `initializeFromItem()`
  - [x] Add `is_tor` to `formatForApi()`
  - [x] Add Switch for Tor Exit Node in Network tab
- [x] **REFACTOR**: Clean up
  - [x] Run `npm run lint`
  - [x] Run `npx tsc --noEmit`

### Phase 6: Integration tests ✅

- [x] **GREEN**: Verify all tests pass
  - [x] `cargo test --lib` — 42/42 pass
  - [x] `cargo clippy --lib -- -D warnings` — zero warnings
  - [x] `cargo fmt --check` — clean

### Phase 7: Archive

- [ ] Run `openspec archive tor-filter` to merge deltas into main specs
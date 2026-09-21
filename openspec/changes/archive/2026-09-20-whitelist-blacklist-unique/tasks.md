# Tasks — whitelist-blacklist-unique

## RED (Write failing tests)
- [x] Whitelist: test DELETE via query param (not JSON body)
- [x] Whitelist: test 409 on duplicate (entry_type, value) create
- [x] Whitelist: test 409 on duplicate (entry_type, value) update
- [x] Whitelist: test same value different entry_type allowed (200)
- [x] Blacklist: test DELETE via query param (not JSON body)
- [x] Blacklist: test 409 on duplicate (entry_type, value) create
- [x] Blacklist: test 409 on duplicate (entry_type, value) update
- [x] Blacklist: test same value different entry_type allowed (200)

## GREEN (Minimal implementation)
- [x] Backend: Change DELETE handler to use `Query<DeleteParams>` in whitelist.rs
- [x] Backend: Change DELETE handler to use `Query<DeleteParams>` in blacklist.rs
- [x] Backend: Add `AppError::Conflict` variant
- [x] Backend: Add duplicate check in `WhitelistEntry::create()`
- [x] Backend: Add duplicate check in `WhitelistEntry::update()`
- [x] Backend: Add duplicate check in `BlacklistEntry::create()`
- [x] Backend: Add duplicate check in `BlacklistEntry::update()`
- [x] Migration: Add UNIQUE INDEX on whitelist(entry_type, value)
- [x] Migration: Add UNIQUE INDEX on blacklist(entry_type, value)

## REFACTOR
- [x] Run `cargo check` — zero errors
- [x] Run `cargo test` — all green (56 passed)
- [x] Run `cargo clippy -- -D warnings` — zero warnings in our code

## CONSOLIDATE
- [ ] Archive change: `openspec archive whitelist-blacklist-unique`
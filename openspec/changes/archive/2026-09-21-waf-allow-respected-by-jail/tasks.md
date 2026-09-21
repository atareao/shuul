# Tasks

## 1. PendingBan data structure

- [ ] 1.1 Define `PendingBan` struct in `backend/src/models/` with fields: ip, rule_id, reason, ban_duration_seconds, escalation_level, created_at. Verify with `cargo check`.
- [ ] 1.2 Add `pending_bans: Mutex<Vec<PendingBan>>` field to `AppState`. Verify with `cargo check`.

## 2. Jail pipeline — mark pending bans instead of executing

- [ ] 2.1 In `report.rs`, replace the `ban_manager.ban()` + `persist_ban()` block with logic that creates a `PendingBan`, pushes it to `app_state.pending_bans`, and logs a `pending` audit event. Keep `rl.record(ip)` and stats. Add `"pending"` to `should_log` for audit mode. Verify with `cargo test`.

## 3. WAF pipeline — execute pending bans

- [ ] 3.1 In `shuul.rs`, in the "No WAF rule matched" block, add logic to iterate `pending_bans` for the request's IP, verify the request matches the ban's rule_id via `CacheRule::matches()`, and if so, execute the ban via `BanManager::ban()` + `persist_ban()` + audit_log `sanctioned`. Verify with `cargo test`.

## 4. PendingBan cleanup

- [ ] 4.1 Add a background task that cleans up expired `PendingBan` entries every 60 seconds. Verify with `cargo test`.

## 5. Verification

- [ ] 5.1 Run full test suite (`cargo test`) and confirm all tests pass.
- [ ] 5.2 Run linter (`cargo clippy -- -D warnings`) and confirm zero warnings.
- [ ] 5.3 Run formatter check (`cargo fmt --check`) and confirm formatting is correct.
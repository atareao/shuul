# Tasks — rename-log-events

## TDD Task Checklist

### Backend: shuul.rs (WAF pipeline)

- [ ] **RED**: Write tests for new event names in shuul tests
- [ ] **GREEN**: Rename `audit_log!("block", ...)` → `audit_log!("denied", ...)`
- [ ] **GREEN**: Rename `audit_log!("allow", ...)` → `audit_log!("admitted", ...)`
- [ ] **GREEN**: Rename `audit_log!("log_only", ...)` → `audit_log!("admitted", ...)`
- [ ] **GREEN**: Remove `audit_log!("pass", ...)` (no log when no match + not banned)
- [ ] **GREEN**: Update `should_log` function to match new event names
- [ ] **REFACTOR**: Run `cargo test` and `cargo clippy -- -D warnings`

### Backend: report.rs (Jail pipeline)

- [ ] **RED**: Write tests for new event names in report tests
- [ ] **GREEN**: Remove `audit_log!("report_received", ...)`
- [ ] **GREEN**: Remove `audit_log!("report_match", ...)`
- [ ] **GREEN**: Rename `audit_log!("report_ok", ...)` → `audit_log!("cleared", ...)`
- [ ] **GREEN**: Rename `audit_log!("report_skip", ...)` → `audit_log!("cleared", ...)`
- [ ] **GREEN**: Remove `audit_log!("report_block", ...)` (replaced by registered/sanctioned)
- [ ] **GREEN**: Rename `audit_log!("report_ban", ...)` → `audit_log!("sanctioned", ...)`
- [ ] **GREEN**: Add `audit_log!("registered", ...)` for match + fail_code without ban
- [ ] **GREEN**: Make `registered` and `sanctioned` mutually exclusive (not sequential)
- [ ] **GREEN**: Update `should_log` function to match new event names
- [ ] **REFACTOR**: Run `cargo test` and `cargo clippy -- -D warnings`

### Frontend: logs_page.tsx

- [ ] **GREEN**: Update `EVENT_COLORS` mapping with new event names
- [ ] **GREEN**: Add persistence for event filters via URL search params
- [ ] **GREEN**: Add persistence for auto-refresh via URL search params
- [ ] **REFACTOR**: Verify frontend builds and tests pass

### Verification

- [ ] Run `cargo test` — all tests pass
- [ ] Run `cargo clippy -- -D warnings` — zero warnings
- [ ] Run `cargo fmt --check` — formatting clean
- [ ] Run `openspec validate rename-log-events` — valid
- [ ] Run `openspec archive rename-log-events` — archive complete
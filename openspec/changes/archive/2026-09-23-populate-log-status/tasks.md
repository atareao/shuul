# Tasks: populate-log-status

## TDD Checklist

### Task 1: Add status_code to `cleared` (no matches) audit_log in report.rs
- [x] RED: Write test verifying `cleared` event includes `status_code` from payload
- [x] GREEN: Add `"status_code": payload.status_code` to `audit_log!("cleared", ...)` at line 129
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 2: Add status_code to `pending` audit_log in report.rs
- [x] RED: Write test verifying `pending` event includes `status_code` from payload
- [x] GREEN: Add `"status_code": payload.status_code` to `audit_log!("pending", ...)` at line 220
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Task 3: Add status_code to `registered` audit_log in report.rs
- [x] RED: Write test verifying `registered` event includes `status_code` from payload
- [x] GREEN: Add `"status_code": payload.status_code` to `audit_log!("registered", ...)` at line 257
- [x] REFACTOR: `cargo clippy -- -D warnings` + `cargo fmt`

### Final verification
- [x] `cargo test` all pass
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo fmt --check` clean
# Tasks

## 1. RED — Write failing tests

- [x] 1.1 Add test `test_refresh_non_2xx_preserves_stale_data` that simulates a non-2xx HTTP response and verifies the existing set is preserved. Run `cargo test tor_service` to confirm the new test fails (RED).

## 2. GREEN — Implement HTTP status check

- [x] 2.1 Modify `TorService.refresh()` to check `resp.status().is_success()` before processing the body. If not successful, log a warning and return early without modifying the exit_nodes set. Run `cargo test tor_service` to confirm all tests pass (GREEN).

## 3. REFACTOR — Clean up

- [x] 3.1 Run `cargo clippy -- -D warnings` and `cargo fmt --check` to ensure zero warnings and proper formatting.
- [x] 3.2 Run full test suite with `cargo test` to confirm no regressions.
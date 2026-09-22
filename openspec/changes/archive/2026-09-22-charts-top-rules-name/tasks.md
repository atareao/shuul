# Tasks: charts-top-rules-name

## TDD Checklist

### Phase 2 — RED (Write failing tests)

- [x] Write test for `read_top_rules` returning rule names from cache
- [x] Write test for `read_top_rules` fallback when rule not in cache
- [x] Run tests and confirm new tests fail (RED)

### Phase 2 — GREEN (Minimal implementation)

- [x] Modify `read_top_rules` to look up rule names from `AppState.rules`
- [x] Run tests and confirm all pass (GREEN)

### Phase 2 — REFACTOR (Clean & consolidate)

- [x] Run `cargo clippy -- -D warnings`
- [x] Run `cargo fmt --check`
- [x] Run full test suite
- [x] Archive change proposal
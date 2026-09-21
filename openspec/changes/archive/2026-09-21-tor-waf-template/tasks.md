# Tasks: Tor WAF template

## TDD Checklist

### 1. Backend: Add `is_tor` to RuleTemplate struct
- [ ] Add `is_tor: Option<bool>` field to `RuleTemplate` in `backend/src/templates.rs`
- [ ] Add `is_tor()` builder method
- [ ] Update `RuleTemplate::waf()` and `RuleTemplate::jail()` constructors to set `is_tor: None`

### 2. Backend: Add Tor exit nodes template
- [ ] Create WAF template "Tor exit nodes" with `is_tor: Some(true)`, `allow: false`, `weight: 300`, `must_have: false`, category `"tor"`
- [ ] Place it after Geo blocking in the template list

### 3. Backend: Update tests
- [ ] Update `test_waf_template_count` from 23 to 24
- [ ] Add test `test_tor_exit_nodes_template_exists`
- [ ] Add test `test_tor_exit_nodes_template_appears_after_geo_blocking`

### 4. Frontend: Add `is_tor` to RuleTemplate interface
- [ ] Add `is_tor: boolean | null` to `RuleTemplate` in `frontend/src/models/template.ts`

### 5. Frontend: Send `is_tor` in confirmApply
- [ ] Add `is_tor: template.is_tor ?? null` to the body in `confirmApply` in `templates_page.tsx`

### 6. Verification
- [ ] Run `cargo test --lib` (all pass)
- [ ] Run `cargo clippy -- -D warnings` (no warnings)
- [ ] Run `npx tsc --noEmit` (frontend type checks pass)
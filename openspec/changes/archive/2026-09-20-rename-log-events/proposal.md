# Rename Audit Log Events — Implementation Proposal

## Intent

Rename audit log event names across the WAF and Jail pipelines to be clearer,
more semantic, and distinguishable at a glance. Remove redundant or
intermediate events. Add `registered` and `sanctioned` as mutually exclusive
Jail outcomes.

## Scope

- **Backend:** `backend/src/http/shuul.rs`, `backend/src/http/report.rs`
- **Frontend:** `frontend/src/pages/admin/logs_page.tsx`
- **Specs:** `openspec/specs/waf-pipeline/spec.md`, `openspec/specs/jail-pipeline/spec.md`

## Impact

| File | Change |
|---|---|
| `backend/src/http/shuul.rs` | Rename audit log event strings; update `should_log` |
| `backend/src/http/report.rs` | Rename audit log event strings; update `should_log`; split `registered` vs `sanctioned` |
| `frontend/src/pages/admin/logs_page.tsx` | Update `EVENT_COLORS`; add URL search param persistence for event filters and auto-refresh |
| `openspec/specs/waf-pipeline/spec.md` | Update audit log scenarios to new event names |
| `openspec/specs/jail-pipeline/spec.md` | New spec covering Jail pipeline, events, and rate limiting flow |

## Event Name Mapping

### WAF Pipeline

| Old | New | Semantics |
|---|---|---|
| `block` | `denied` | WAF rule matched with `allow=false` |
| `banned` | `banned` | IP is currently banned by Jail (no WAF rule matched) |
| `allow` | `admitted` | WAF rule matched with `allow=true` |
| `log_only` | `admitted` | Merged with `allow` — both mean WAF allowed the request |
| `pass` | *(removed)* | No log when no rule matches and IP not banned |

### Jail Pipeline

| Old | New | Semantics |
|---|---|---|
| `report_received` | *(removed)* | Too verbose, no value |
| `report_ok` | `cleared` | No jail rules matched |
| `report_match` | *(removed)* | Intermediate detail, no value |
| `report_skip` | `cleared` | Merged with `report_ok` |
| `report_block` | *(removed)* | Replaced by `registered` / `sanctioned` |
| `report_ban` | `sanctioned` | Threshold exceeded, IP banned |
| *(new)* | `registered` | Match + fail_code, stats recorded, no ban yet |

### Key Design Decisions

- `registered` and `sanctioned` are **mutually exclusive** per request (not
  sequential). A single Jail pipeline evaluation produces exactly one of:
  `cleared`, `registered`, or `sanctioned`.
- `admitted` consolidates both `allow` and `log_only` — the user only needs to
  know the request was let through by WAF, not whether a rule explicitly
  allowed it or it was in log-only mode.
- `pass` is removed entirely: silence means "no rule matched and IP not
  banned", which is the common case and doesn't need a log entry.

## Capabilities

### New

- `jail-pipeline` — Define the Jail pipeline behavior, audit log events, and
  rate limiting flow.

### Modified

- `waf-pipeline` — Update audit log categories requirement to reflect new
  event names.

## Tasks

1. Update `openspec/specs/waf-pipeline/spec.md` — rename audit log events in
   scenarios.
2. Create `openspec/specs/jail-pipeline/spec.md` — full spec with new event
   names.
3. Update `backend/src/http/shuul.rs` — rename event strings, update
   `should_log`.
4. Update `backend/src/http/report.rs` — rename event strings, update
   `should_log`, implement `registered`/`sanctioned` mutual exclusion.
5. Update `frontend/src/pages/admin/logs_page.tsx` — `EVENT_COLORS`, URL
   search param persistence.
6. Archive change proposal.
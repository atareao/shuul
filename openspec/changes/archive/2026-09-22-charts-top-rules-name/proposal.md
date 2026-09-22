# Change Proposal: Show rule name instead of rule ID in Top Rules chart

## Why

The "Top Rules" pie chart on the Charts page displayed numeric rule IDs (e.g., "42", "15") which are meaningless to users. Operators need to see human-readable rule names to quickly identify which rules are triggering most frequently.

## What Changes

**Backend** — `read_top_rules` handler in `backend/src/http/stats.rs`:
- Before: returned `(rule_id.to_string(), count, percentage)` — first element was the numeric ID as string
- After: locks `AppState.rules`, builds a `HashMap<i32, String>` from cached `CacheRule` entries, returns `rule.name` if found, or `"Unknown rule #{id}"` fallback

**Frontend**: No changes needed — the pie chart already uses the first tuple element as the display `name`.

## Scope
- **Backend**: `backend/src/http/stats.rs` — `read_top_rules` handler
- **Frontend**: No changes needed (already uses first tuple element as `name`)

## Impact
- Backend: `read_top_rules` now looks up rule names from the in-memory cache (`AppState.rules`) instead of returning raw rule IDs.
- API contract: The first element of each tuple in the response changes from rule ID (string) to rule name (string).
- Frontend: No changes required — the pie chart already uses the first element as the display name.
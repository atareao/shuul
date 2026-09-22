# Persist Log Viewer State Across Views

## Intent
Preserve the user's event filter selection (`hiddenEvents`) and `autoRefresh` toggle state across navigation between admin views. Currently these are lost when the user navigates away from `/admin/logs` and returns, because the component unmounts and resets to defaults.

## Scope
- **Frontend only**: `frontend/src/pages/admin/logs_page.tsx`
- No backend changes required
- No new dependencies

## Impact
- `hiddenEvents` and `autoRefresh` are persisted in `localStorage` under a `logsPage` key
- Constructor reads from `localStorage` instead of hardcoded defaults
- Every state change writes to `localStorage` immediately
- `componentWillUnmount` also persists current state as a safety net
- Backward compatible: users without the key get the same defaults as before
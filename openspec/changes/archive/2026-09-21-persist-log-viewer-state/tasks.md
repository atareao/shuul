# Tasks: persist-log-viewer-state

## TDD Checklist

### RED phase
- [x] Write test: State restored from localStorage on mount
- [x] Write test: State defaults when localStorage is empty
- [x] Write test: Toggling event filter persists immediately
- [x] Write test: Toggling auto-refresh persists immediately
- [x] Write test: State survives navigation away and back

### GREEN phase
- [x] Implement: Read `hiddenEvents` and `autoRefresh` from localStorage in constructor
- [x] Implement: Write to localStorage on every state change (toggleEventFilter, toggleAutoRefresh)
- [x] Implement: Write to localStorage in componentWillUnmount as safety net
- [x] Implement: Start/stop poll timer based on restored autoRefresh value

### REFACTOR phase
- [x] Run linter: `npm run lint`
- [x] Run type check: `npx tsc --noEmit`
- [x] Run tests: `npx vitest run`
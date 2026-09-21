# Whitelist / Blacklist — UNIQUE constraint + DELETE fix

## Why

Two bugs affect the whitelist and blacklist CRUD:

1. **DELETE returns 400**: The frontend sends `DELETE /api/v1/whitelist?id=3` (query param), but the backend expects a JSON body `{"id": N}`. Axum's `Json<T>` extractor fails on empty body → 400 Bad Request. This makes it impossible to delete entries from the UI.

2. **Duplicate entries allowed**: Without a UNIQUE constraint on `(entry_type, value)`, administrators can accidentally create duplicate whitelist/blacklist entries (e.g., the same IP whitelisted twice). This causes confusion and wastes resources.

## What Changes

### DELETE fix
- Change DELETE handlers in `whitelist.rs` and `blacklist.rs` from `Json<DeleteParams>` to `Query<DeleteParams>`
- No frontend changes needed — the frontend already sends query params correctly

### UNIQUE constraint
- New migration `20260920000001_add_whitelist_blacklist_unique.up.sql` with `CREATE UNIQUE INDEX` on both tables
- New `AppError::Conflict` variant mapping to HTTP 409
- New `check_unique()` method in both models, called at the start of `create()` and `update()`
- Duplicate detection before DB insertion for clear error messages

## Impact

- DELETE now works from the frontend (no more 400)
- Creating/updating a whitelist or blacklist entry with an existing `(entry_type, value)` pair returns 409 Conflict
- Backward compatible — no data model changes, only new constraints
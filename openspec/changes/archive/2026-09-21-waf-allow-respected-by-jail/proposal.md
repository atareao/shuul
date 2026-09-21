# Proposal: WAF Allow Rules Are Respected by Jail Pipeline

## Why

The WAF and Jail pipelines are completely independent. When a WAF rule with `allow=true` matches a request (e.g., a trusted IP whitelisted via WAF), the WAF pipeline returns 200 OK immediately. However, the Jail pipeline — which runs separately via the Traefik plugin reporting to `POST /api/v1/report` — has no knowledge of the WAF's decision. It evaluates ALL jail rules independently and can ban the IP based on rate limits, even though the WAF explicitly allowed the traffic.

This defeats the purpose of WAF allow rules: users expect that a WAF rule with `allow=true` for a specific IP means "trust this IP, don't block it."

## What Changes

- **Jail pipeline (`report.rs`)**: Before applying rate limiting for a matched jail rule, check if any WAF rule with `allow=true` matches the request. If so, skip rate limiting for that request entirely.
- **No changes to WAF pipeline (`shuul.rs`)**: The WAF pipeline already works correctly.
- **No changes to the ReportPayload or plugin**: The check is done server-side by examining the existing cached rules.

## Capabilities

### Modified Capabilities
- `jail-pipeline`: Add a new requirement: "Jail pipeline respects WAF allow decisions" — before applying rate limiting, the Jail pipeline checks if any WAF rule with `allow=true` matches the request. If so, the request is skipped (no rate limiting, no ban).

## Impact

- **`backend/src/http/report.rs`**: Add logic to check for matching WAF allow rules before processing jail rules. This is a server-side change only.
- **No API changes**: The `/api/v1/report` endpoint signature remains unchanged.
- **No database changes**: No schema migrations needed.
- **No plugin changes**: The Traefik plugin continues to work as before.
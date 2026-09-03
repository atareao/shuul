# Shuul: WAF + Rate Limiter Architecture

## Overview

Shuul operates **two independent pipelines** over a single set of rules:

| Pipeline | Endpoint | Role | Analogy |
|---|---|---|---|
| **WAF** | `POST /` (shuul) | Intercept, match, allow/deny, persist | Firewall |
| **Jail** | `POST /report` (report) | Post-factum rate limiting + ban | fail2ban |

Both pipelines share the **same rule definitions** from the `rules` table, but each uses different fields and has different behavior.

---

## Pipeline 1: WAF (`shuul.rs`)

### Purpose

Decide whether a request is **allowed** or **blocked** in real time, and whether to **store** it for analytics.

### Flow

```
Request → Safe paths? → Trusted IPs? → Trusted UAs? → Banned IP? → Rule match → Allow/Deny
```

1. **Safe paths** — if request path matches a `safe_paths` regex → ALLOW (skip all checks)
2. **Trusted IPs** — if request IP is in a `trusted_ips` CIDR → ALLOW (skip all checks)
3. **Trusted UAs** — if request User-Agent matches a `trusted_user_agents` regex → ALLOW
4. **Banned IP** — if IP is in the in-memory ban list → **403 FORBIDDEN**
5. **Rule matching** — iterate rules by `weight ASC`, first match wins:
   - `mode = "off"` → skip
   - `mode = "log_only"` → allow=true (log without blocking)
   - `mode = "enforce"` → apply `allow` and `store` from the rule
6. **Persist** — if `store = true`, save the request to DB (or cache)
7. **Response** — `200 OK` if allow, `403 FORBIDDEN` if deny

### Fields used by WAF

| Field | Purpose |
|---|---|
| `weight` | Priority (lower = evaluated first) |
| `mode` | `enforce`, `log_only`, or `off` |
| `allow` | Whether to allow (`true`) or block (`false`) |
| `store` | Whether to persist the request |
| All filter fields | Matching conditions (see below) |

### WAF does NOT

- Evaluate rate limits
- Ban IPs
- Check `fail_codes`
- Have side effects

---

## Pipeline 2: Jail / Rate Limiter (`report.rs`)

### Purpose

Track **failed responses** per IP and ban IPs that exceed thresholds. Inspired by fail2ban: multiple independent jails, each with its own filter and rate limit profile.

### Flow

```
Traefik sends report (ip, status_code, path, method)
    ↓
Match against ALL rules (no break)
    ↓
For each matching rule with rate_limit_profile_id:
    ↓
    Load profile → check if status_code ∈ fail_codes
    ↓
    If yes → record hit in RateLimiter (per rule_id, per IP)
    ↓
    If threshold exceeded → ban IP in memory + persist to DB
    ↓
200 OK (fire-and-forget)
```

### Key difference from WAF

The report pipeline iterates **ALL** matching rules, not just the first one. This is the fail2ban model: multiple jails can independently track and ban the same IP.

Example:

| Rule | Weight | Filter | Rate limit | fail_codes |
|---|---|---|---|---|
| R1 | 200 | path = `/api/login` | 5/60s | 401 |
| R2 | 300 | (none) | 100/600s | 401,403,404 |

A request to `/api/login` that returns 401 will be counted by **both** R1 and R2. If either threshold is exceeded, the IP is banned.

### Fields used by Jail

| Field | Purpose |
|---|---|
| `rate_limit_profile_id` | FK to the rate limit profile (defines max_retry, find_time, ban_time, fail_codes, etc.) |
| All filter fields | Matching conditions (same as WAF) |

### Jail does NOT

- Intercept requests (it's post-factum)
- Set `allow` or `store`
- Return anything other than 200 OK

---

## Rule Types (UI)

In the frontend, each rule is classified into one of three types based on its fields:

| Type | Badge | Condition | Example |
|---|---|---|---|
| **WAF** | 🔵 Blue | Has filters (ip, path, etc.) but **no** rate limit profile | Block requests from China, log all traffic |
| **Jail** | 🟢 Green | Has rate limit profile but **no** filters | Rate limit 100/600s for all traffic |
| **WAF + Jail** | 🟣 Purple | Has both filters AND rate limit profile | Block `/api/login` + rate limit 5/60s on 401 |

The `Type` column in the rules table shows this classification. A `Select` filter above the table lets you show only WAF, only Jail, or both.

---

## Filter Fields

All filter fields are **optional regex patterns**. A rule matches a request if **all** defined filters match. Undefined filters are ignored.

| Field | Request source | Example |
|---|---|---|
| `ip_address` | `X-Forwarded-For` | `^192\.168\.` |
| `protocol` | `X-Forwarded-Proto` | `^https$` |
| `fqdn` | `X-Forwarded-Host` | `^api\.example\.com$` |
| `path` | `X-Forwarded-Uri` (path only) | `^/api/login` |
| `query` | `X-Forwarded-Uri` (query only) | `^token=.*` |
| `city_name` | GeoIP (MaxMind) | `^Madrid` |
| `country_name` | GeoIP (MaxMind) | `^China` |
| `country_code` | GeoIP (MaxMind) | `^CN$` |
| `user_agent` | `X-Forwarded-User-Agent` or `User-Agent` | `^curl/` |
| `method` | `X-Forwarded-Method` | `^(GET\|POST)$` |
| `referer` | `X-Forwarded-Referer` or `Referer` | `^https://myapp\.com` |
| `content_type` | `X-Forwarded-Content-Type` or `Content-Type` | `^application/json` |
| `accept_language` | `X-Forwarded-Accept-Language` or `Accept-Language` | `^es` |
| `x_request_id` | `X-Forwarded-X-Request-Id` or `X-Request-Id` | `^req-` |

### Matching logic

```
If filter IS defined AND request value EXISTS → regex MUST match
If filter IS defined AND request value IS NULL → condition passes (true)
If filter is NOT defined → condition passes (true)
```

All conditions must pass for the rule to match.

---

## Rate Limit Profiles

Defined in the `rate_limit_profiles` table. Referenced by rules via `rate_limit_profile_id`.

| Field | Description |
|---|---|
| `max_retry` | Maximum failed requests before ban |
| `find_time_seconds` | Sliding window duration |
| `ban_time_seconds` | Ban duration (if no escalation) |
| `bantime_increment` | Enable escalation (multipliers) |
| `bantime_multipliers` | Escalation multipliers `[1,2,4,8]` |
| `bantime_maxtime_seconds` | Maximum ban duration |
| `ban_count_decay_days` | Days before ban count resets |
| `fail_codes` | Status codes that count as failures, e.g. `[401,403,404]` |

### fail_codes

The `fail_codes` field is what makes shuul different from traditional rate limiters. Instead of counting all requests, it only counts requests that resulted in specific HTTP status codes. This means:

- A user browsing normally (200 OK) is never counted
- A bot getting 401 on every login attempt is counted
- A scraper getting 403 on every page is counted

This prevents false positives from legitimate traffic spikes.

---

## Behavioral Comparison

| Scenario | WAF | Jail |
|---|---|---|
| Request to `/api/login` returns 200 | Allow (if rule allows). Store if configured. | Ignored (200 ∉ fail_codes) |
| Request to `/api/login` returns 401 | Same as above | Counted. If threshold exceeded → ban |
| Request to `/` with no matching rule | Allow (default). Not stored. | Ignored (no matching rule with rate limit) |
| Request from banned IP | 403 FORBIDDEN | N/A (Traefik doesn't report banned IPs) |
| Request from China, rule blocks China | 403 FORBIDDEN | N/A (blocked before reaching backend) |

---

## Concurrency Model

Both pipelines follow the same pattern:

1. **Acquire lock** (rules, rate_limiter, ban_manager)
2. **Do sync work** (match, record, ban in memory)
3. **Release lock**
4. **Do async work** (persist to DB)

All `MutexGuard` instances are dropped before any `.await` to satisfy Rust `Send` constraints (required by axum/tokio).

---

## Example Configurations

### WAF only: Block requests from China

```
Rule:
  name: Block China
  weight: 100
  mode: enforce
  allow: false
  store: true
  country_code: ^CN$
  rate_limit_profile_id: null
```

### Jail only: Rate limit all traffic

```
Rule:
  name: Global rate limit
  weight: 200
  mode: enforce
  allow: true
  store: false
  rate_limit_profile_id: 1  # max_retry=100, find_time=600s, fail_codes=[401,403,404]
```

### WAF + Jail: Block + rate limit login endpoint

```
Rule:
  name: Login protection
  weight: 300
  mode: enforce
  allow: true
  store: true
  path: ^/api/login
  rate_limit_profile_id: 2  # max_retry=5, find_time=60s, fail_codes=[401]
```

### Multiple jails: Two independent rate limits

```
Rule 1:
  name: Strict login rate
  weight: 200
  path: ^/api/login
  rate_limit_profile_id: 2  # 5/60s on 401

Rule 2:
  name: Global fail rate
  weight: 300
  rate_limit_profile_id: 1  # 100/600s on 401,403,404
```

Both rules match requests to `/api/login`. Both track independently. The IP is banned if **either** threshold is exceeded.
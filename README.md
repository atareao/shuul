# Shuul

The gatekeeper for your data — ForwardAuth service for Traefik with
fail2ban-style rate limiting based on HTTP response codes.

## Architecture

Shuul operates **two independent pipelines** over a single set of rules:

```
                    ┌──────────────────────────────────────────────┐
                    │                   Traefik                    │
                    │                                              │
                    │  Request ──► [ForwardAuth] ──► [Plugin] ──► │
                    │                              ▲               │
                    └──────────────────────────────┼───────────────┘
                                                   │
                    ┌──────────────────────────────┼───────────────┐
                    │           shuul              │               │
                    │                              │               │
                    │  ┌───────────────────────────┘               │
                    │  │                                           │
                    │  │  Pipeline 1: WAF (POST /shuul)            │
                    │  │  ┌──────────┐                             │
                    │  │  │ Matching │  First match wins           │
                    │  │  ├──────────┤                             │
                    │  │  │ Allow /  │  allow, store, mode         │
                    │  │  │ Deny     │                             │
                    │  │  └──────────┘                             │
                    │  │  200 OK or 403 FORBIDDEN                  │
                    │  │                                           │
                    │  │  Pipeline 2: Jail (POST /report)          │
                    │  │  ┌──────────┐                             │
                    │  │  │ Matching │  ALL matches count           │
                    │  │  ├──────────┤                             │
                    │  │  │ Rate     │  fail2ban-style: max_retry, │
                    │  │  │ Limiter  │  find_time, ban_time,       │
                    │  │  │          │  fail_codes, escalation     │
                    │  │  ├──────────┤                             │
                    │  │  │ Ban      │  Memory + DB persistence    │
                    │  │  │ Manager  │                             │
                    │  │  └──────────┘                             │
                    │  │  200 OK (fire-and-forget)                 │
                    │  │                                           │
                    │  └───────────────────────────────────────────┘
                    │                                              │
                    │  POST /api/v1/report ◄── Plugin async POST   │
                    │       │               (status_code report)   │
                    │       ▼                                      │
                    │  For EACH matching rule with rate limit:     │
                    │  If status_code ∈ fail_codes → rate + ban    │
                    └──────────────────────────────────────────────┘
```

### Components

1. **shuul (WAF)** — ForwardAuth service that authorizes requests before they reach
   your backend. Returns 200 OK (allow) or 403 (deny). **No rate limiting** — pure
   matching + allow/deny.
2. **shuul (Jail)** — Post-factum rate limiter. Receives backend HTTP response codes
   from the Traefik plugin and applies fail2ban-style rate limiting. Multiple
   independent jails (rules) can track the same IP simultaneously.
3. **traefik-shuul-reporter** — Traefik middleware plugin that captures the
   backend's actual HTTP response code and reports it back to shuul.
4. **PostgreSQL** — Persistent storage for rules, rate limit profiles, bans,
   requests, and settings.

---

## Features

### Rule Engine (WAF)
- Match requests by: IP, FQDN, path, query, country, user-agent, method,
  referer, content-type, accept-language, x-request-id
- Regex patterns for flexible matching
- Weight-based priority ordering (lower weight = evaluated first)
- Modes: `enforce` (block if not allowed), `log_only` (log but don't block),
  `off` (ignore rule)
- Actions: `allow` (let through) or `deny` (block with 403)
- Optional `store` to capture request metadata to database
- Template library with 40+ preconfigured rules for common services
  (WordPress, Nextcloud, Grafana, phpMyAdmin, etc.)

### Rate Limiting (Jail — fail2ban-style)
- **Post-factum**: rate limiting happens AFTER the backend responds, not during
  the request. Zero latency impact.
- **Multiple independent jails**: ALL matching rules with rate limit profiles
  are evaluated independently. Each rule is a separate "jail" like fail2ban.
- **fail_codes**: only HTTP status codes defined in the profile count as failures
  (e.g., `[401, 403, 404]`). 200 OK responses are never counted.
- Per-rule rate limit profiles with independent configuration
- Sliding window counter (find_time): 3 failures in 10 minutes
- Automatic ban with configurable duration
- Ban escalation: multipliers `[1, 2, 4, 8]` for repeat offenders
- Ban count decay over configurable days
- IP whitelist (`ignoreip`)

### Rule Types (UI)

Each rule is classified into one of three types in the frontend:

| Type | Badge | Condition | Example |
|---|---|---|---|
| **WAF** | 🔵 Blue | Has filters but **no** rate limit profile | Block requests from China |
| **Jail** | 🟢 Green | Has rate limit profile but **no** filters | Rate limit 100/600s for all traffic |
| **WAF + Jail** | 🟣 Purple | Has both filters AND rate limit profile | Block `/api/login` + rate limit 5/60s |

A `Select` filter above the rules table lets you show only WAF, only Jail, or both.

### Geo IP
- MaxMind GeoLite2 database integration
- City, country name, and country code matching
- Block traffic from specific countries

### Export / Import
- Export all rules as JSON (`GET /api/v1/rules/export`)
- Import rules from JSON (`POST /api/v1/rules/import`)
- Upsert by name — reimporting replaces existing rules
- Useful for backup, migration between instances, or CI/CD

### SSO / OIDC
- Authentication via OIDC provider (PocketID)
- JWT-based session management
- Automatic metadata refresh every 30 minutes

---

## Quick Start

### Prerequisites
- Docker and Docker Compose
- Traefik v3.x configured as reverse proxy

### 1. Database

shuul uses PostgreSQL. The schema is consolidated in a single migration:

```
backend/migrations/20260826000001_initial_schema.up.sql
```

All previous incremental migrations have been consolidated into this file.

### 2. Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — | PostgreSQL connection string |
| `PORT` | `3000` | HTTP listen port |
| `SECRET` | — | JWT signing secret |
| `OIDC_ISSUER_URL` | — | OIDC issuer URL (PocketID) |
| `OIDC_CLIENT_ID` | — | OIDC client ID |
| `OIDC_CLIENT_SECRET` | — | OIDC client secret |
| `OIDC_REDIRECT_URL` | `http://localhost:3000/api/v1/auth/callback` | OIDC callback URL |
| `MAXMIND_DB_PATH` | `geo/GeoLite2-City.mmdb` | Path to GeoIP database |
| `CACHE_ENABLED` | `false` | Enable request caching |
| `CACHE_SIZE` | `10` | Max cached requests |
| `RUST_LOG` | `debug` | Log level |

### 3. Run with Docker

```bash
docker compose up -d
```

### 4. Traefik Configuration

Add the shuul reporter plugin:

```yaml
# traefik.yml
experimental:
  plugins:
    shuul-reporter:
      moduleName: github.com/atareao/traefik-shuul-reporter
      version: v0.1.0
```

Configure the middleware chain:

```yaml
# dynamic.yml
http:
  middlewares:
    shuul-auth:
      forwardAuth:
        address: "http://shuul:3000/api/v1/shuul"

    shuul-reporter:
      plugin:
        shuul-reporter:
          shuulUrl: "http://shuul:3000/api/v1/report"
          timeoutMs: 500
          reportClientIP: true

  routers:
    my-router:
      middlewares:
        - shuul-auth      # 1st: ForwardAuth (allow/deny)
        - shuul-reporter  # 2nd: Capture backend response (rate limit)
```

---

## API Reference

All endpoints are prefixed with `/api/v1`.

### Public Endpoints (no auth)

| Method | Path | Description |
|---|---|---|
| `POST` | `/shuul` | **WAF** — ForwardAuth: validate and filter request |
| `GET` | `/util` | Utility endpoints (geo lookup) |
| `GET` | `/health` | Health check |
| `GET` | `/auth/sso` | OIDC SSO redirect |
| `GET` | `/auth/callback` | OIDC callback |
| `GET` | `/auth/sso-status` | SSO configuration status |
| `POST` | `/report` | **Jail** — Receive backend status code report from plugin |

### Protected Endpoints (JWT required)

| Method | Path | Description |
|---|---|---|
| `GET/POST` | `/rules` | List / Create rules |
| `PATCH` | `/rules` | Update rule |
| `DELETE` | `/rules` | Delete rule |
| `GET` | `/rules/export` | **Export all rules as JSON** |
| `POST` | `/rules/import` | **Import rules from JSON (upsert by name)** |
| `GET/POST` | `/rate-limit-profiles` | List / Create profiles |
| `PATCH` | `/rate-limit-profiles` | Update profile |
| `DELETE` | `/rate-limit-profiles` | Delete profile |
| `GET` | `/bans` | List active bans |
| `POST` | `/bans` | Manually ban IP |
| `DELETE` | `/bans` | Unban IP |
| `GET/PUT` | `/settings` | Global settings |
| `GET` | `/templates/rules` | Rule templates |
| `GET` | `/templates/rate-limit-profiles` | Profile templates |

### WAF Endpoint (`POST /api/v1/shuul`)

Called by Traefik as ForwardAuth for every incoming request.

**Logic:**
1. Safe paths → ALLOW (skip all checks)
2. Trusted IPs → ALLOW (skip all checks)
3. Trusted User-Agents → ALLOW (skip all checks)
4. Banned IP → 403 FORBIDDEN
5. Match against rules (first match wins by weight):
   - `mode = "off"` → skip
   - `mode = "log_only"` → allow=true
   - `mode = "enforce"` → apply allow/store
6. Persist if `store = true`
7. 200 OK or 403 FORBIDDEN

**No rate limiting is evaluated in this endpoint.**

### Report Endpoint (`POST /api/v1/report`)

Called by the Traefik plugin to report a backend HTTP response.

**Request:**
```json
{
  "ip_address": "192.168.1.100",
  "status_code": 401,
  "path": "/wp-login.php",
  "method": "POST"
}
```

**Logic:**
1. Match IP/path/method against **ALL** active rules (fail2ban-style)
2. For **each** matching rule with `rate_limit_profile_id`:
   - Load the rate limit profile
   - Check if `status_code` is in the profile's `fail_codes`
   - If yes → increment rate limiter counter for that IP
   - If threshold exceeded → ban the IP
3. Always returns 200 OK (fire-and-forget)

### Rule Export / Import

**Export:** `GET /api/v1/rules/export`
```json
{
  "status": 200,
  "message": "OK",
  "data": [
    {
      "id": 1,
      "name": "WordPress - wp-login",
      "weight": 100,
      "allow": false,
      "path": "^/wp-login\\.php",
      "active": true,
      ...
    }
  ]
}
```

**Import:** `POST /api/v1/rules/import`
```json
{
  "rules": [
    {
      "name": "WordPress - wp-login",
      "weight": 100,
      "allow": false,
      "path": "^/wp-login\\.php",
      ...
    }
  ]
}
```

Response:
```json
{
  "status": 200,
  "message": "OK",
  "data": { "imported": 15 }
}
```

Rules are upserted by `name` — existing rules with the same name are
replaced. The rule cache is reloaded after import.

---

## Rate Limit Profiles

### Default Profiles

| Name | Max Retry | Find Time | Ban Time | Escalate | Fail Codes | Use Case |
|---|---|---|---|---|---|---|
| **Strict** | 3 | 10 min | 24h | Yes | `401,403,404,429` | Critical endpoints (admin, API keys) |
| **Moderate** | 5 | 10 min | 1h | Yes | `401,403,404` | Login pages, user authentication |
| **Relaxed** | 30 | 5 min | 30 min | No | `401,403` | Public endpoints, read-only APIs |
| **Scraping** | 60 | 1 min | 10 min | Yes | `401,403,404,429,500` | High-traffic, anti-scraping |

### Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | — | Unique profile name |
| `description` | string | — | Human-readable description |
| `max_retry` | int | 5 | Max failures before ban |
| `find_time_seconds` | int | 600 | Sliding window (seconds) |
| `ban_time_seconds` | int | 3600 | Initial ban duration (seconds) |
| `bantime_increment` | bool | false | Enable ban escalation |
| `bantime_multipliers` | int[] | [1,2,4,8] | Escalation multipliers |
| `bantime_maxtime_seconds` | int | 604800 | Max ban duration (1 week) |
| `ban_count_decay_days` | int | 30 | Days before ban count resets |
| `fail_codes` | int[] | [401,403,404] | **Status codes that count as failures** |

### fail_codes

The `fail_codes` field is what makes shuul different from traditional
rate limiters. Instead of counting every request, you define which
HTTP response codes count as "failures" for rate limiting purposes:

- **401 Unauthorized** — Failed login / missing auth
- **403 Forbidden** — Access denied by backend
- **404 Not Found** — Scanner/probe hitting non-existent paths
- **429 Too Many Requests** — Backend rate limit response
- **500 Internal Server Error** — Backend errors (anti-scraping)

This mirrors fail2ban's approach: define a filter (what counts as a
failure), then define an action (what to do when thresholds are met).

---

## Traefik Plugin: traefik-shuul-reporter

The [traefik-shuul-reporter](https://github.com/atareao/traefik-shuul-reporter)
plugin bridges the gap between shuul and the backend.

### Why it's needed

shuul's WAF pipeline only sees requests, never backend responses.
Without the plugin, shuul can only match and filter requests.
With the plugin, shuul's Jail pipeline knows:

- "This IP got 5 login failures in 10 minutes" (401 from backend)
- "This IP is scanning for PHP files" (404 from backend)
- "This IP is scraping aggressively" (429 from backend)

### How it works

```
Backend response ──► Plugin wraps ResponseWriter
                           │
                    captureWriter.WriteHeader(401)
                           │
                    Extract IP from X-Forwarded-For
                           │
                    POST /api/v1/report (async goroutine)
                           │
                    shuul evaluates: status_code ∈ fail_codes?
```

The plugin:
1. Wraps Go's `http.ResponseWriter` to intercept `WriteHeader()`
2. After the backend responds, extracts the client IP
3. Launches an async goroutine that POSTs the report to shuul
4. Returns immediately — zero latency impact on the response

### Installation

```bash
git clone git@github.com:atareao/traefik-shuul-reporter.git
cd traefik-shuul-reporter
go build -o plugin .
```

Or reference directly from Traefik:

```yaml
experimental:
  plugins:
    shuul-reporter:
      moduleName: github.com/atareao/traefik-shuul-reporter
      version: v0.1.0
```

---

## Configuration

### Frontend Settings (3 tabs)

The admin panel has a Settings page with 3 tabs:

| Tab | Fields |
|---|---|
| **General** | `log_retention_days` (1-365), `default_rule_mode` (enforce/log_only/off) |
| **Security** | `safe_paths` (regex patterns, one per line), `trusted_ips` (CIDR, one per line), `trusted_user_agents` (regex, one per line) |
| **Rules** | Export (download all rules as .json), Import (upload .json with rules array) |

### Rule Templates

shuul ships with 40+ preconfigured rule templates for:

- **CMS**: WordPress (wp-login, xmlrpc, wp-admin), Drupal, Joomla
- **Auth**: phpMyAdmin, Webmin, Jenkins, Grafana, Portainer
- **Email**: Roundcube, RainLoop, SnappyMail
- **Cloud**: Nextcloud, Home Assistant
- **API**: Generic login/token endpoints, OAuth callbacks
- **Security**: Known bot user-agents, common scanner paths

---

## Development

### Backend

```bash
cd backend
cargo check        # Verify compilation
cargo build        # Build binary
cargo test         # Run tests
```

### Frontend

```bash
cd frontend
npm install        # Install dependencies
npx tsc --noEmit   # Type-check
npm run dev        # Dev server (Vite)
```

### Database

The schema is a single migration file:

```bash
# Reset database (drops all tables, creates fresh)
psql -U postgres -d shuul -f backend/migrations/20260826000001_initial_schema.up.sql
```

---

## License

MIT
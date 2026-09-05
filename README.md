# Shuul

**The gatekeeper for your data** — WAF + fail2ban-style rate limiting for Traefik, powered by SQLite.

Shuul is a ForwardAuth service for Traefik that evaluates incoming requests against a configurable set of rules (WAF pipeline) and applies post-factum rate limiting based on backend HTTP response codes (Jail pipeline). It ships with a full React admin dashboard, 80+ preconfigured rule templates, and integrates via a Traefik middleware plugin.

---

## Architecture

```
                    ┌────────────────────────────────────────────────┐
                    │                    Traefik                     │
                    │                                                 │
                    │  Request ──► [ForwardAuth] ──► [Backend] ──►   │
                    │                                    │            │
                    │                           [shuul-reporter]     │
                    │                              plugin            │
                    └──────────────────────┬─────────────────────────┘
                                           │
                    ┌──────────────────────┼─────────────────────────┐
                    │        shuul         │                         │
                    │                      │                         │
                    │  Pipeline 1: WAF (POST /api/v1/shuul)         │
                    │  ┌──────────┐                                  │
                    │  │ Matching │  First match wins (by weight)    │
                    │  ├──────────┤                                  │
                    │  │ Allow /  │  enforce, log_only, off modes    │
                    │  │ Deny     │                                  │
                    │  └──────────┘                                  │
                    │  200 OK or 403 FORBIDDEN                        │
                    │                                                │
                    │  Pipeline 2: Jail (POST /api/v1/report)        │
                    │  ┌──────────┐                                  │
                    │  │ Matching │  ALL matching rules count        │
                    │  ├──────────┤                                  │
                    │  │ Rate     │  fail2ban-style: max_retry,      │
                    │  │ Limiter  │  find_time, fail_codes,          │
                    │  │          │  escalation, ban                 │
                    │  ├──────────┤                                  │
                    │  │ Ban      │  In-memory + SQLite persistence  │
                    │  │ Manager  │                                  │
                    │  └──────────┘                                  │
                    │  200 OK (fire-and-forget)                      │
                    └────────────────────────────────────────────────┘
```

### Two independent pipelines

| Pipeline | Endpoint | Role | Behaviour |
|---|---|---|---|
| **WAF** | `POST /api/v1/shuul` | ForwardAuth — intercept, match, allow/deny | First matching rule wins (by weight ASC). Safe paths, trusted IPs, and trusted UAs bypass all checks. |
| **Jail** | `POST /api/v1/report` | Post-factum rate limiter (fail2ban-style) | ALL matching rules count independently. Each rule with a rate limit profile is a separate "jail". |

### How it works

1. **Traefik** receives an incoming request
2. **ForwardAuth** (`shuul-auth` middleware) sends the request to `POST /api/v1/shuul`
3. **WAF pipeline** evaluates rules, returns 200 OK (allow) or 403 FORBIDDEN (deny)
4. If allowed, the request reaches your **backend**
5. After the backend responds, the **shuul-reporter plugin** captures the HTTP status code
6. **Jail pipeline** (`POST /api/v1/report`) receives the report and applies rate limiting:
   - Matches against ALL jail rules
   - If `status_code ∈ fail_codes` → increments the rate limiter counter
   - If threshold exceeded → bans the IP
7. The banned IP receives 403 on subsequent WAF checks

---

## Features

### Rule Engine (WAF)

- 14 filter fields: IP, FQDN, path, query, country, user-agent, method, referer, content-type, accept-language, x-request-id, protocol, city, country code
- Regex patterns for flexible matching
- Weight-based priority (lower weight = evaluated first)
- Three modes: `enforce` (block if not allowed), `log_only` (log but don't block), `off` (ignore)
- `allow` / `deny` actions
- Pipeline classification: WAF (filtering only), Jail (rate limiting), or Both
- Safe paths, trusted IPs, and trusted User-Agent bypass lists

### Rate Limiting (Jail — fail2ban-style)

- **Post-factum**: rate limiting happens AFTER the backend responds — zero latency impact on the request path
- **Multiple independent jails**: each matching rule is evaluated independently with its own rate limit profile
- **fail_codes**: only specific HTTP status codes count as failures (e.g., `[401, 403, 404]`)
- Sliding window counter per IP per profile
- Configurable ban duration with escalation (multipliers `[1, 2, 4, 8]`)
- Ban count decay over configurable days
- Circular ring-buffer implementation for O(1) rate limit checks

### Dashboard (React SPA)

- **Dashboard**: overview with rule counts, request totals, active bans, security checklist
- **Rules**: full CRUD with 4-tab editor (General, Network, Location, Request)
- **Rate Limit Profiles**: full CRUD with escalation configuration
- **Bans**: active ban table with manual ban/unban
- **Templates**: 80+ preconfigured rule templates in 3 categories (WAF, Jail, Rate Limit Profiles)
- **Charts**: time-series evolution (stacked bar, line, by-method), rankings (countries, rules, methods, paths, FQDNs) as donut charts
- **Settings**: global configuration (safe paths, trusted IPs, trusted UAs, log level)
- **Dark/light mode** with persisted preference
- **i18n**: Spanish, Valencian, English
- SSO/OIDC authentication via PocketID

### GeoIP

- MaxMind GeoLite2 City database integration
- City, country name, and country code matching
- moka LRU cache (10k entries, 1h TTL) for performance
- Block traffic from specific countries

### Templates Library

81 preconfigured rule templates and 9 rate limit profile templates for:

- **CMS**: WordPress (wp-login, xmlrpc, wp-admin), Drupal, Joomla, Magento, PrestaShop
- **Cloud**: Nextcloud, Home Assistant
- **Auth**: phpMyAdmin, Webmin, Jenkins, Grafana, Portainer, Kubernetes Dashboard
- **Email**: Roundcube, RainLoop, SnappyMail
- **Security**: Known bots, scanners, Log4j, SSTI, SQL injection, XSS, path traversal
- **API**: Generic login/token, OAuth callbacks, GraphQL
- **Infrastructure**: Traefik, Kibana, pgAdmin, Mailpit, Mailhog

### Export / Import

- Export all rules as JSON (`GET /api/v1/rules/export`)
- Import rules from JSON with upsert by name (`POST /api/v1/rules/import`)
- Useful for backup, migration, or CI/CD

### Data Storage

- **SQLite** — single file, zero administration
- Stats snapshot persisted every 30 minutes (survives restarts)
- Bans persisted to database immediately
- Settings stored as key-value pairs
- No external database server required

---

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Traefik v3.x configured as reverse proxy
- An OIDC provider (e.g., [PocketID](https://github.com/atareao/pocketid))
- MaxMind GeoLite2 City database (optional, for GeoIP features)

### 1. Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `SECRET` | ✅ | — | JWT signing secret (random string, minimum 32 chars) |
| `OIDC_ISSUER_URL` | ✅ | — | OIDC issuer URL (e.g., `https://auth.example.com`) |
| `OIDC_CLIENT_ID` | ✅ | — | OIDC client ID |
| `OIDC_CLIENT_SECRET` | ✅ | — | OIDC client secret |
| `OIDC_REDIRECT_URL` | ❌ | `http://localhost:3000/api/v1/auth/callback` | OIDC callback URL |
| `DATABASE_URL` | ❌ | `sqlite:///app/data/shuul.db?mode=rwc` | SQLite database path |
| `PORT` | ❌ | `3000` | HTTP listen port |
| `MAXMIND_DB_PATH` | ❌ | `geo/GeoLite2-City.mmdb` | Path to GeoIP database |
| `RUST_LOG` | ❌ | `info` | Log level (trace, debug, info, warn, error) |

### 2. Run with Docker

```bash
# Create directories for persistent data
mkdir -p data geo

# Download MaxMind GeoLite2 database (optional)
# Place GeoLite2-City.mmdb in geo/

# Copy and edit production environment
cp .env.prod.example .env
# Edit .env with your values

# Start
docker compose up -d
```

### 3. First Access

1. Open `https://shuul.yourdomain.com` in your browser
2. Click "Sign in with PocketID"
3. After SSO authentication, you'll be redirected to the admin dashboard
4. Go to **Templates** to apply preconfigured rules
5. Configure **Settings** for safe paths and trusted IPs

### 4. Traefik Configuration

Add the shuul-reporter plugin to your Traefik static config:

```yaml
# traefik.yml
experimental:
  plugins:
    shuul-reporter:
      moduleName: github.com/atareao/traefik-shuul-reporter
      version: v0.1.0
```

Configure the middleware chain in your dynamic config:

```yaml
# dynamic.yml
http:
  middlewares:
    shuul-auth:
      forwardAuth:
        address: "http://shuul:3000/api/v1/shuul"
        trustForwarders: true

    shuul-reporter:
      plugin:
        shuul-reporter:
          shuulUrl: "http://shuul:3000/api/v1/report"
          timeoutMs: 500
          reportClientIP: true

  routers:
    my-service:
      middlewares:
        - shuul-auth      # 1st: ForwardAuth (allow/deny)
        - shuul-reporter  # 2nd: Capture backend response (rate limit)
```

---

## API Reference

All endpoints are prefixed with `/api/v1`. Protected endpoints require a Bearer JWT token in the `Authorization` header.

### Public Endpoints (no auth)

| Method | Path | Description |
|---|---|---|
| `ANY` | `/shuul` | **WAF** — ForwardAuth: validate and filter request |
| `POST` | `/report` | **Jail** — Receive backend status code report from plugin |
| `GET` | `/health` | Health check |
| `GET` | `/util/complete?ip=` | GeoIP lookup for a single IP |
| `GET` | `/auth/sso` | OIDC SSO redirect |
| `GET` | `/auth/callback` | OIDC callback |
| `GET` | `/auth/sso-status` | SSO configuration status |
| `GET` | `/templates` | List predefined rule and profile templates |

### Protected Endpoints (JWT required)

| Method | Path | Description |
|---|---|---|
| `GET` | `/rules` | List rules (paged, sortable, filterable) |
| `POST` | `/rules` | Create rule |
| `PATCH` | `/rules` | Update rule |
| `DELETE` | `/rules?id=` | Delete rule |
| `GET` | `/rules/export` | Export all rules as JSON |
| `POST` | `/rules/import` | Import rules from JSON (upsert by name) |
| `GET` | `/rules/info?option=` | Rule count (total, active) |
| `GET` | `/rules/info/all` | Total + active rule counts |
| `GET/POST` | `/rate-limit-profiles` | List / Create profiles |
| `PATCH` | `/rate-limit-profiles` | Update profile |
| `DELETE` | `/rate-limit-profiles?id=` | Delete profile |
| `GET` | `/rate-limit-profiles/info?option=total` | Profile count |
| `GET` | `/bans` | List active bans (paged, sortable) |
| `POST` | `/bans` | Manually ban an IP |
| `DELETE` | `/bans?id= or ?ip_address=` | Unban IP |
| `GET` | `/bans/info` | Active ban count |
| `GET/PUT` | `/settings` | Get / Update global settings |
| `GET` | `/stats/info` | Stats summary (allowed, blocked totals) |
| `GET` | `/stats/top_countries` | Top 10 blocked countries |
| `GET` | `/stats/top_rules` | Top 10 blocking rules |
| `GET` | `/stats/top_methods` | Top 10 HTTP methods |
| `GET` | `/stats/top_paths` | Top 10 request paths |
| `GET` | `/stats/top_fqdns` | Top 10 FQDNs |
| `GET` | `/stats/evolution?unit=&last=` | Time-series blocked/allowed |
| `GET` | `/stats/evolution_by_method?unit=&last=` | Time-series per HTTP method |

### WAF Endpoint (`ANY /api/v1/shuul`)

Called by Traefik as ForwardAuth for every incoming request.

**Response:** `200 OK` (allow) or `403 FORBIDDEN` (deny)

**Logic:**
1. Safe paths → ALLOW (bypass all checks)
2. Trusted IPs → ALLOW (bypass all checks)
3. Trusted User-Agents → ALLOW (bypass all checks)
4. Banned IP → 403 FORBIDDEN
5. Match against rules (first match wins by weight ASC):
   - `mode = "off"` → skip
   - `mode = "log_only"` → allow=true
   - `mode = "enforce"` → apply allow/deny
6. 200 OK or 403 FORBIDDEN

**No rate limiting is evaluated in this endpoint.**

### Report Endpoint (`POST /api/v1/report`)

Called by the Traefik plugin to report a backend HTTP response. Always returns 200 OK (fire-and-forget).

```json
{
  "ip_address": "192.168.1.100",
  "status_code": 401,
  "path": "/wp-login.php",
  "method": "POST"
}
```

**Logic:**
1. Match IP/path/method against ALL active rules
2. For each matching rule with `rate_limit_profile_id`:
   - Load the rate limit profile
   - Check if `status_code` is in the profile's `fail_codes`
   - If yes → increment rate limiter counter for that IP
   - If threshold exceeded → ban the IP
3. Always returns 200 OK

### Rule Export / Import

**Export:** `GET /api/v1/rules/export`
```json
{
  "status": 200,
  "message": "OK",
  "data": [ { "id": 1, "name": "WordPress - wp-login", ... } ]
}
```

**Import:** `POST /api/v1/rules/import`
```json
{
  "rules": [ { "name": "WordPress - wp-login", "path": "^/wp-login\\.php", ... } ]
}
```

Rules are upserted by `name` — existing rules with the same name are replaced.

---

## Rate Limit Profiles

### Default Profiles (shipped with the application)

| ID | Name | Max Retry | Window | Ban Time | Escalate | Fail Codes |
|---|---|---|---|---|---|---|
| 1 | Auth Brute Force | 5 | 300s | 900s | Yes | 401 |
| 2 | Admin Guard | 5 | 300s | 3600s | Yes | 401, 403 |
| 3 | Path Scanning | 20 | 60s | 300s | Yes | 403, 404 |
| 4 | API Abuse | 100 | 60s | 300s | Yes | 401, 403, 429 |
| 5 | Scraping | 60 | 60s | 300s | Yes | 403, 429, 500 |
| 6 | Health & Webhooks | 100 | 60s | 60s | No | 500, 502, 503 |
| 7 | Recidive | 3 | 172800s | 604800s | Yes | 403, 429 |
| 8 | Global Shield | 300 | 60s | 300s | Yes | 403, 404, 429, 500, 502, 503 |
| 9 | Scanner Aggressive | 50 | 10s | 1800s | Yes | 403, 404, 405, 500 |

### Profile Fields

| Field | Type | Description |
|---|---|---|
| `max_retry` | int | Max failures before ban |
| `find_time_seconds` | int | Sliding window duration (seconds) |
| `ban_time_seconds` | int | Initial ban duration (seconds) |
| `bantime_increment` | bool | Enable ban escalation on repeat offences |
| `bantime_multipliers` | int[] | Escalation multipliers (default: `[1, 2, 4, 8]`) |
| `bantime_maxtime_seconds` | int | Maximum ban duration cap |
| `ban_count_decay_days` | int | Days before ban count resets |
| `fail_codes` | int[] | HTTP status codes that count as failures |

### fail_codes

The `fail_codes` field is what makes shuul unique. Instead of counting every request, you define which HTTP response codes count as "failures":

- **401 Unauthorized** — Failed login / missing auth
- **403 Forbidden** — Access denied (by backend or WAF)
- **404 Not Found** — Scanner probing non-existent paths
- **429 Too Many Requests** — Backend rate limit
- **500/502/503** — Backend errors (anti-scraping, resource abuse)

---

## Traefik Plugin: traefik-shuul-reporter

The [traefik-shuul-reporter](https://github.com/atareao/traefik-shuul-reporter) plugin bridges the gap between shuul and your backend. Without it, shuul only sees incoming requests (WAF). With it, shuul knows:

- "This IP got 5 login failures in 10 minutes" (401)
- "This IP is scanning for PHP files" (404)
- "This IP is scraping aggressively" (429)

### How it works

```mermaid
Backend response → Plugin wraps ResponseWriter
                    captureWriter.WriteHeader(401)
                    Extract IP from X-Forwarded-For
                    POST /api/v1/report (async goroutine)
                    shuul evaluates: status_code ∈ fail_codes?
```

The plugin intercepts `WriteHeader()` on the Go response writer, extracts the client IP, and sends an async POST to shuul's report endpoint — zero latency impact on the response.

---

## Development

### Backend (Rust / Axum)

```bash
cd backend
cargo check               # Verify compilation
cargo build               # Build binary
cargo test                # Run tests
cargo clippy              # Lint
cargo fmt                 # Format code

# Run backend only (frontend must be built first)
RUST_LOG=debug cargo run
```

### Frontend (React / TypeScript / Vite)

```bash
cd frontend
pnpm install              # Install dependencies
npx tsc --noEmit          # Type-check
pnpm run dev              # Dev server (Vite, port 5173)

# Build for production
pnpm build                # Outputs to dist/
```

### Full Stack (via just)

```bash
just frontend     # Build frontend, copy to backend/static
just backend      # Run backend
just dev          # Build frontend + copy + run backend
just build        # Build Docker image
```

### Database

SQLite — single file, zero configuration. Migrations run automatically on startup:

```bash
backend/migrations/
├── 20260902000000_initial_schema.up.sql      # Full initial schema
├── 20260903000000_add_log_all_requests_setting.up.sql
├── 20260903000001_update_rate_limit_profiles.up.sql
└── 20260903000002_add_scanner_aggressive_profile.up.sql
```

---

## Rule Types (UI Classification)

Each rule in the frontend is classified by type:

| Type | Badge | Condition | Example |
|---|---|---|---|
| **WAF** | 🔵 Blue | Has filters but **no** rate limit profile | Block requests from China |
| **Jail** | 🟢 Green | Has rate limit profile but **no** filters | Rate limit 100/600s all traffic |
| **WAF + Jail** | 🟣 Purple | Has both filters AND rate limit profile | Block `/api/login` + rate limit 5/60s |

---

## Security Checklist

1. ✅ Set a strong `SECRET` (minimum 32 random characters)
2. ✅ Configure OIDC with a proper identity provider
3. ✅ Apply WAF templates for your exposed services
4. ✅ Configure safe paths for health checks and webhooks
5. ✅ Set trusted IPs for your internal networks
6. ✅ Review rate limit profiles and adjust thresholds
7. ✅ Enable GeoIP blocking for countries with no business traffic
8. ✅ Test with `log_only` mode before switching to `enforce`

---

## Upgrading

Shuul uses SQLite with automatic migrations. To upgrade:

```bash
# Pull new image
docker compose pull

# Restart (migrations run automatically)
docker compose up -d
```

For major version upgrades, check the [releases page](https://github.com/atareao/shuul/releases).

---

## License

MIT
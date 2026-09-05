# Shuul User Guide

Shuul is a WAF (Web Application Firewall) and rate-limiting system that protects your web services. This guide covers everything you need to know to use the shuul admin dashboard effectively.

---

## Getting Started

### Accessing the Dashboard

1. Open `https://shuul.yourdomain.com` in your browser
2. Click **Sign in with PocketID** (SSO button)
3. Authenticate with your OIDC provider
4. You're redirected to the admin dashboard

If you see "401 Unauthorized", your session may have expired — sign in again.

### Layout

The admin interface has:

- **Sidebar** (left): navigation — Dashboard, Rules, Rate Limit Profiles, Bans, Templates, Charts, Settings
- **Header** (top): logout button and dark/light mode toggle
- **Content** (center): the active page

---

## Dashboard

The dashboard gives you a quick overview of your shuul instance:

- **Total Rules** — number of rules (WAF + Jail)
- **Active Rules** — rules currently in `enforce` or `log_only` mode
- **Total Requests** — sum of allowed + blocked requests (tracked since last restart/persist)
- **Filtered Requests** — blocked requests
- **Active Bans** — currently banned IP addresses
- **Security Checklist** — shows how many of the recommended templates you've applied out of the total

---

## Rules

Rules are the core of shuul. They define what traffic to allow, block, or rate-limit.

### Rule Types

| Type | Badge | Description |
|---|---|---|
| **WAF** | 🔵 Blue | Filters requests by matching fields (IP, path, country, etc.) |
| **Jail** | 🟢 Green | Rate-limits traffic using a profile (no filters — catches all) |
| **WAF + Jail** | 🟣 Purple | Both filters AND rate limiting — most powerful |

Use the pipeline filter above the table to show only WAF, only Jail, or both.

### Creating a Rule

1. Go to **Rules** in the sidebar
2. Click **Create** (top right)
3. Fill in the 4-tab form:

#### Tab 1: General

| Field | Description |
|---|---|
| **Name** | A descriptive name (e.g., "Block China traffic") |
| **Description** | What this rule does |
| **Active** | Toggle on to enable the rule |
| **Pipeline** | `waf`, `jail`, or `both` |
| **Mode** | `enforce` (block), `log_only` (log but don't block), `off` (disabled) |
| **Allow** | On = allow matching requests, Off = deny |
| **Weight** | Priority (lower number = evaluated first). Default: 100 |
| **Rate Limit Profile** | Select a profile to enable rate limiting (Jail) |

#### Tab 2: Network

Match by network-level attributes. All patterns are regex:

| Field | Example | Description |
|---|---|---|
| **IP Address** | `^192\.168\.` | Source IP or CIDR |
| **Protocol** | `^https?$` | HTTP protocol |
| **FQDN** | `^admin\.` | Hostname (Fully Qualified Domain Name) |
| **Referer** | `^https?://(www\.)?mydomain` | HTTP Referer header |

#### Tab 3: Location

Match by geographic location (requires MaxMind GeoIP database):

| Field | Example | Description |
|---|---|---|
| **City** | `^(Shanghai|Beijing)` | City name |
| **Country Name** | `^China$` | Full country name |
| **Country Code** | `^(CN|RU)$` | ISO 2-letter country code |

#### Tab 4: Request

Match by request attributes:

| Field | Example | Description |
|---|---|---|
| **Path** | `^/wp-login\.php` | URL path |
| **Query** | `action=login` | Query string |
| **Method** | `^(POST|PUT)$` | HTTP method |
| **User-Agent** | `^(python-requests|curl)` | User-Agent header |
| **Content-Type** | `^application/json` | Content-Type header |
| **Accept-Language** | `^en` | Accept-Language header |
| **X-Request-ID** | `^[a-f0-9]{8}` | Custom request ID |

### Editing a Rule

Click the **Edit** button on any rule row. The same 4-tab dialog opens with current values pre-filled.

### Deleting a Rule

Click the **Delete** button. Confirm in the dialog. This is permanent.

### Rule Matching Logic

```
For WAF rules:
  Rules are sorted by weight (ASC)
  First rule that matches → applies its action (allow/deny)
  If no rule matches → request passes (200 OK)

For Jail rules:
  ALL matching rules are evaluated independently
  Each rule has its own rate limit profile
  Each profile tracks the IP separately
```

### Rule Modes

| Mode | Behaviour |
|---|---|
| `enforce` | Actively block or allow matching requests |
| `log_only` | Log the match but always allow the request |
| `off` | Rule is ignored entirely |

Use `log_only` to test new rules before enabling them.

---

## Rate Limit Profiles

Profiles define the rate limiting behaviour — how many failures in what time window trigger a ban, and for how long.

### Default Profiles

| Name | Max Retry | Window | Ban Time | Best For |
|---|---|---|---|---|
| Auth Brute Force | 5 | 5 min | 15 min | Login pages |
| Admin Guard | 5 | 5 min | 1 hour | Admin panels |
| Path Scanning | 20 | 1 min | 5 min | Blocking scanners |
| API Abuse | 100 | 1 min | 5 min | API endpoints |
| Scraping | 60 | 1 min | 10 min | Anti-scraping |
| Health & Webhooks | 100 | 1 min | 1 min | Safe endpoints |
| Recidive | 3 | 48 hours | 1 week | Repeat offenders |
| Global Shield | 300 | 1 min | 5 min | Catch-all |
| Scanner Aggressive | 50 | 10 sec | 30 min | Aggressive scanners |

### Creating a Profile

1. Go to **Rate Limit Profiles** in the sidebar
2. Click **Create**
3. Fill in the 2-tab form:

#### Tab 1: General

| Field | Description |
|---|---|
| **Name** | Descriptive name |
| **Description** | What this profile is for |
| **Max Retry** | Number of failures before a ban |
| **Find Time (seconds)** | Sliding window for counting failures |
| **Fail Codes** | HTTP status codes that count as failures (e.g., `401, 403, 404`) |

#### Tab 2: Penalty

| Field | Description |
|---|---|
| **Ban Time (seconds)** | Default ban duration |
| **Max Ban Time (seconds)** | Maximum ban duration (with escalation) |
| **Ban Count Decay (days)** | Days before the ban counter resets |
| **Escalate** | Enable ban time escalation on repeat offences |
| **Multipliers** | Escalation multipliers (e.g., `1, 2, 4, 8`) |

### How Escalation Works

With escalation enabled and multipliers `[1, 2, 4, 8]`:

| Offence | Multiplier | Ban Duration (with 900s base) |
|---|---|---|
| 1st | 1× | 900s (15 min) |
| 2nd | 2× | 1800s (30 min) |
| 3rd | 4× | 3600s (1 hour) |
| 4th+ | 8× | 7200s (2 hours) |

The escalation counter decays after the configured `ban_count_decay_days` of clean behaviour.

### fail_codes

Only these HTTP status codes count as "failures" for rate limiting:

| Code | Meaning | Typical Use |
|---|---|---|
| 401 | Unauthorized | Failed login / missing auth |
| 403 | Forbidden | WAF block or backend access denied |
| 404 | Not Found | Scanner probing non-existent paths |
| 405 | Method Not Allowed | Probing with wrong HTTP methods |
| 429 | Too Many Requests | Backend rate-limit response |
| 500 | Internal Server Error | Backend crashing on malformed input |
| 502 | Bad Gateway | Backend upstream failure |
| 503 | Service Unavailable | Backend overload |

---

## Bans

The Bans page shows all actively banned IP addresses.

### Ban Information

Each ban shows:
- **IP Address** — the banned IP
- **Reason** — why it was banned (rule match or manual)
- **Duration** — ban period
- **Time Remaining** — countdown
- **Escalation Level** — current escalation tier
- **Rule** — the rule that triggered the ban

### Manually Banning an IP

1. Go to **Bans** in the sidebar
2. Click **Create**
3. Enter the IP address and a reason
4. The ban is applied immediately in-memory and persisted to SQLite

### Unbanning

Click **Delete** on any ban row. The IP is unbanned immediately.

### Automatic Cleanup

Expired bans are cleaned up every 60 seconds by a background task. No manual intervention needed.

---

## Templates

Templates are preconfigured rules you can apply with one click. Shuul ships with 81 rule templates and 9 rate limit profile templates.

### Browsing Templates

1. Go to **Templates** in the sidebar
2. Three tabs: **WAF Templates**, **Jail Templates**, **Rate Limit Profiles**
3. Use the search bar to filter templates
4. Expand categories to see all templates in that group

### Applying a Template

1. Find the template you want (e.g., "WordPress - wp-login")
2. Click **Apply**
3. Review the pre-filled values in the dialog
4. Adjust weight, mode, or other fields as needed
5. Click **Save**

The rule is created and becomes active immediately (if `active` is checked).

### Template Categories

| Category | Example Templates |
|---|---|
| **WordPress** | wp-login, xmlrpc, wp-admin, REST API |
| **Nextcloud** | Login, Shared Links |
| **Grafana** | Login panel |
| **phpMyAdmin** | Login page |
| **Security** | Known Bots, SQL Injection, XSS, Path Traversal |
| **CMS** | Drupal, Joomla, Magento, PrestaShop |
| **Auth** | Auth Brute Force, Admin Guard, API Abuse |
| **Scanner** | Path Scanning, Scanner Aggressive |

---

## Charts

The Charts page gives you visibility into traffic patterns and security events.

### Evolution Tab

| Chart | Description |
|---|---|
| **Summary Cards** | Total allowed, blocked, and block rate % |
| **Evolution** | Stacked bar chart: allowed vs blocked over time |
| **Block Rate** | Line chart: block percentage over time |
| **By Method** | Multi-line chart: request count by HTTP method |

### Rankings Tab

| Chart | Description |
|---|---|
| **Top Countries** | Donut chart: which countries generate the most blocked traffic |
| **Top Rules** | Donut chart: which rules trigger most blocks |
| **Top Methods** | Donut chart: which HTTP methods are most used |
| **Top Paths** | Donut chart: which paths receive the most requests |
| **Top FQDNs** | Donut chart: which hostnames receive the most traffic |

### Time Controls

Use the **Show last N** dropdown to change the time window:
- **Last hour** (minute granularity)
- **Last 24 hours** (hour granularity)
- **Last 7 days** (daily granularity)
- **Last 30 days** (daily granularity)

---

## Settings

The Settings page configures global behaviour.

### General Tab

| Field | Description |
|---|---|
| **Default Rule Mode** | Default mode for new rules (`enforce`, `log_only`, `off`) |
| **Log Retention** | Days to retain request logs (1–365) |
| **Log Level** | Controls which events are logged (`all`, `pass`, `audit`) |

### Safe Paths Tab

Paths that bypass all rule checking. One regex pattern per line.

Examples:
```
^/health$
^/webhook/
^/\.well-known/
^/favicon\.ico$
```

Requests to safe paths are immediately ALLOWED — no rules are evaluated.

### Trusted IPs Tab

IPs/CIDRs that bypass all rule checking. One entry per line.

Examples:
```
10.0.0.0/8
172.16.0.0/12
192.168.0.0/16
```

Trusted IP requests are immediately ALLOWED — no rules are evaluated.

### Trusted User Agents Tab

User-Agent patterns that bypass all rule checking. One regex per line.

Examples:
```
^Googlebot
^BingPreview
^Kube-Probe
^UptimeRobot
```

Trusted UA requests are immediately ALLOWED — no rules are evaluated.

### Log Level Options

| Level | Logs |
|---|---|
| `all` | Everything — request passes, blocks, safe paths, trusted IPs |
| `pass` | Only pass-through (no rule matched) events |
| `audit` | Only security events (bans, blocks, report blocks) |

---

## Export / Import

### Exporting Rules

1. Go to **Settings** → **Export** tab
2. Click **Export** to download all rules as JSON
3. The file contains every rule with all its fields

### Importing Rules

1. Go to **Settings** → **Import** tab
2. Click **Import** and select a JSON file
3. Rules are upserted by name — if a rule with the same name exists, it's replaced
4. The rule cache is reloaded after import

This is useful for:
- Backing up your configuration
- Migrating between shuul instances
- Version-controlling your rules

---

## Understanding the WAF + Jail Flow

```
                   ┌──────────────┐
                   │   Request    │
                   └──────┬───────┘
                          │
                    ┌─────▼──────┐
                    │  Safe      │──── ALLOW (skip all checks)
                    │  Path?     │
                    └─────┬──────┘
                          │ No
                    ┌─────▼──────┐
                    │  Trusted   │──── ALLOW (skip all checks)
                    │  IP?       │
                    └─────┬──────┘
                          │ No
                    ┌─────▼──────────┐
                    │  Trusted       │──── ALLOW (skip all checks)
                    │  User-Agent?   │
                    └─────┬──────────┘
                          │ No
                    ┌─────▼──────┐
                    │  Banned    │──── 403 FORBIDDEN
                    │  IP?       │
                    └─────┬──────┘
                          │ No
                    ┌─────▼──────────┐
                    │  Rule Match?   │──── 403 FORBIDDEN (if enforce + deny)
                    │  (WAF)         │──── 200 OK (if allow or log_only)
                    └─────┬──────────┘
                          │
                   ┌──────▼──────┐
                   │   Backend   │
                   │   Responds  │
                   └──────┬──────┘
                          │
                    ┌─────▼──────────┐
                    │  Plugin        │
                    │  captures      │
                    │  status_code   │
                    └─────┬──────────┘
                          │
                    ┌─────▼──────────┐
                    │  Jail: match   │
                    │  ALL rules,    │
                    │  rate limit,   │
                    │  ban if needed │
                    └────────────────┘
```

---

## Best Practices

### 1. Start with log_only

When creating a new rule, leave it in `log_only` mode for a few days. Check the charts to see how many requests it would block. Switch to `enforce` only when you're confident.

### 2. Layer your defences

Use WAF rules to filter obvious bad traffic (country blocks, known bots, sensitive paths). Use Jail rules to catch patterns that emerge dynamically (brute force, scraping).

### 3. Set up safe paths

Always add health check endpoints and webhook URLs to safe paths to avoid false positives:

```
^/healthz?$
^/readyz?$
^/webhook/
^/\.well-known/
```

### 4. Monitor the charts

Check the Evolution and Rankings tabs regularly. Sudden spikes in blocked traffic from a new country could indicate an attack. A sudden drop in allowed traffic could mean a rule is too aggressive.

### 5. Use escalation wisely

Login pages: always enable escalation (attackers don't give up after one ban).
Health checks: never enable escalation (false positives would be catastrophic).

### 6. Export regularly

Back up your rules with the export feature. A JSON file is all you need to restore your configuration on a new instance.

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl/Cmd + K` | Focus search (in tables) |
| `Ctrl/Cmd + N` | Create new item |
| `Escape` | Close dialog |

---

## Common Tasks

### "I want to block traffic from China"

1. Go to **Rules** → **Create**
2. General tab: Name = "Block China", Pipeline = WAF, Mode = enforce, Allow = Off
3. Location tab: Country Code = `^CN$`
4. Save

### "I want to rate-limit login attempts"

1. Go to **Templates** → **Jail Templates**
2. Find "Auth Brute Force" → **Apply**
3. This creates a rule with the Auth Brute Force profile (5 failures in 5 min → 15 min ban)
4. Now add a WAF+Jail rule or apply a WAF template for the login path

### "I want to unban an IP"

1. Go to **Bans**
2. Find the IP in the table
3. Click **Delete**

### "My rule isn't blocking anything"

1. Is the rule **active**? (checkbox in the form)
2. Is the mode set to **enforce**?
3. Is the weight low enough? Rules with lower weight run first.
4. Does the path pattern match exactly? Check with regex test tools.
5. Check the **Charts** → **Top Rules** to see if the rule is counting matches.
6. Switch to `log_only` and check the application logs: `docker compose logs shuul | grep LOG_ONLY`

---

## FAQ

**Q: How long do bans last?**
A: Depends on the profile. Default ban for Auth Brute Force is 15 min. Maximum with escalation is 7 days (Recidive profile).

**Q: Do bans survive a restart?**
A: Yes. Bans are persisted to SQLite immediately. On startup, all active bans are loaded from the database.

**Q: Do statistics survive a restart?**
A: Yes. Stats are snapshotted every 30 minutes and loaded on startup.

**Q: Can I run multiple shuul instances?**
A: Not with SQLite. SQLite is a single-file embedded database — only one container can write to it. For multi-instance, you'd need to migrate to a client-server database.

**Q: What happens if shuul is down?**
A: Traefik's ForwardAuth returns 503 when the auth service is unreachable. Your protected services become inaccessible until shuul recovers.

**Q: Does shuul add latency?**
A: The WAF pipeline adds ~1-5ms per request (rule matching is in-memory with precompiled regex). The Jail pipeline adds zero latency — it runs asynchronously after the response is sent.

**Q: Can I use shuul without the Traefik plugin?**
A: Yes. The WAF pipeline works without the plugin. You'll only miss the Jail (rate limiting) features.
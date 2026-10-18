# Self-Hosting Guide — Shuul

This guide walks you through deploying shuul on your own infrastructure. Shuul is designed to run as a Docker container alongside Traefik, protecting your services with WAF filtering and fail2ban-style rate limiting.

---

## Prerequisites

- **Docker + Docker Compose** (or Podman with podman-compose)
- **Traefik v3.x** running as a reverse proxy
- **An OIDC provider** (recommended: [PocketID](https://github.com/atareao/pocketid))
- **MaxMind GeoLite2 City database** (optional, for GeoIP features)
- A domain name pointed to your Traefik instance

---

## 1. Traefik Setup

### 1.1 Enable the shuul-reporter plugin

**Deprecated — use the journald reader instead.** The `traefik-shuul-reporter` plugin is deprecated. The recommended way to feed shuul's Jail pipeline is now the `systemd-journal-gatewayd` integration described in **section 2**: shuul streams Traefik's access logs out of the systemd journal instead of receiving an HTTP `POST` per request. It removes the extra HTTP call per request and the plugin's 500 ms timeout. The plugin still works, and the configuration below remains valid for existing deployments, but it is no longer the recommended path — new installations should not use it.

Add this to your Traefik static configuration (`traefik.yml`):

```yaml
experimental:
  plugins:
    shuul-reporter:
      moduleName: github.com/atareao/traefik-shuul-reporter
      version: v0.1.0
```

### 1.2 Create the middleware chain

In your Traefik dynamic configuration (or provider), add:

```yaml
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
```

### 1.3 Attach middleware to your routers

For each service you want to protect:

```yaml
http:
  routers:
    my-app:
      rule: "Host(`app.example.com`)"
      service: my-app
      middlewares:
        - shuul-auth      # 1st: ForwardAuth (WAF)
        - shuul-reporter  # 2nd: Status code reporting (Jail)
      tls: {}
```

**Why this order:** The WAF middleware runs first to deny malicious requests before they reach your backend. After your backend responds, the reporter plugin captures the status code and sends it to shuul's Jail pipeline.

---

## 2. Reading Traefik Access Logs via systemd-journal-gatewayd

The Jail pipeline (fail2ban-style rate limiting) needs to observe the status codes your backends return. The legacy `shuul-reporter` Traefik plugin pushed those observations to `POST /api/v1/report`; the current design **pulls** them instead, by streaming Traefik's access logs out of the systemd journal:

```
Traefik ──accessLog JSON──► journald (Storage=volatile → tmpfs/RAM)
                                 │
                                 │  systemd-journal-gatewayd  :19531
                                 ▼
              GET /entries?follow&CONTAINER_NAME=traefik
              Accept: application/json   (NDJSON, 1 JSON object per line)
                                 │
                                 ▼
                    Shuul (background task) ──► Jail pipeline
```

This removes the extra HTTP call per request, the plugin's 500 ms timeout, and any continuous access-log write to disk (the runtime journal can live entirely in RAM). The WAF pipeline is unaffected.

### 2.1 Prerequisites: Traefik access logs in JSON over journald

Traefik must emit access logs in JSON, and its container logs must reach the systemd journal. In Traefik's static configuration:

```yaml
accessLog:
  format: json
  # Do NOT set filePath: leave access logs on stdout so the container log driver captures them
```

In the `compose.yml` of the stack running Traefik:

```yaml
services:
  traefik:
    logging:
      driver: journald
```

With the `journald` log driver, every access-log line becomes a journal entry tagged with `CONTAINER_NAME=traefik` — the field shuul filters on. If your Traefik container is named differently, set `SHUUL_JOURNALD_CONTAINER` accordingly (see 2.6).

### 2.2 Install the gateway package

On Debian and Ubuntu, `systemd-journal-gatewayd` and its `.socket`/`.service` units live in a separate package — they are **not** part of the base `systemd` package:

```bash
sudo apt update && sudo apt install -y systemd-journal-remote
```

Other distributions:

- **Fedora / RHEL:** `sudo dnf install systemd-journal-remote`
- **Arch Linux:** the gatewayd units ship with the base `systemd` package (some derivatives also provide a separate `systemd-journal-remote` package).

### 2.3 Enable and start the socket

```bash
sudo systemctl enable --now systemd-journal-gatewayd.socket
```

The gateway is socket-activated: the `.socket` unit listens, and systemd starts `systemd-journal-gatewayd.service` on demand on the first connection.

Verify:

```bash
sudo systemctl status systemd-journal-gatewayd.socket
```

The status must show `active (listening)` with the default port `19531`.

### 2.4 Verify real-time reading

Snapshot — returns the entries currently in the journal for that container, then terminates:

```bash
curl -s http://localhost:19531/entries?CONTAINER_NAME=traefik \
  -H "Accept: application/json"
```

Live stream — stays open and prints new entries as they are written (like `journalctl -f`); it does **not** terminate on its own, stop it with Ctrl+C:

```bash
curl -sN "http://localhost:19531/entries?follow&CONTAINER_NAME=traefik" \
  -H "Accept: application/json"
```

`Accept: application/json` selects the Journal JSON Format: one self-contained JSON object per line (NDJSON), each carrying the entry fields (`MESSAGE`, `CONTAINER_NAME`, `__CURSOR`, `__REALTIME_TIMESTAMP`, …). The `CONTAINER_NAME=traefik` query parameter filters **server-side** (`KEY=match`), so only Traefik's entries are sent. Without `follow`, gatewayd returns the existing entries and closes the connection.

### 2.5 Keep the journal in RAM (`Storage=volatile`)

By default journald uses `Storage=auto`: if `/var/log/journal` exists, it writes to disk as well. For a zero-disk-write setup, switch the runtime journal to volatile:

```ini
# /etc/systemd/journald.conf.d/volatile.conf
[Journal]
Storage=volatile
RuntimeMaxUse=64M
```

Then restart journald:

```bash
sudo systemctl restart systemd-journald
```

With `Storage=volatile`, the journal lives in `/run/log/journal` — a **tmpfs mount, i.e. RAM** — and nothing is persisted. `RuntimeMaxUse` caps how much RAM it may consume; raise it (e.g. `256M`) for high-traffic deployments.

**Trade-off:** with volatile storage the journal is lost on host reboot, so there will be no Traefik access logs to read after a restart. This matches shuul's own behavior — its log viewer is an in-memory ring buffer that is dropped on container restart. Rate-limiting state itself lives in SQLite and is unaffected.

### 2.6 Give the shuul container network access

The container has its own network namespace, so `localhost:19531` inside the container is the container itself, not the host. Point shuul at the host's gatewayd endpoint.

**Recommended — `extra_hosts`:**

```yaml
services:
  shuul:
    extra_hosts:
      - "host.docker.internal:host-gateway"
    environment:
      - SHUUL_JOURNALD_URL=http://host.docker.internal:19531
      - SHUUL_JOURNALD_CONTAINER=traefik
```

`host-gateway` requires Docker 20.10 or newer.

**Alternative — the host's Docker bridge IP.** Any host address the container can reach works; the gateway of the default `docker0` bridge is usually `172.17.0.1`:

```yaml
    environment:
      - SHUUL_JOURNALD_URL=http://172.17.0.1:19531   # example: default docker0 bridge gateway
      - SHUUL_JOURNALD_CONTAINER=traefik
```

Both variables are optional:

| Variable | Default | Description |
|---|---|---|
| `SHUUL_JOURNALD_URL` | `http://localhost:19531` | Base URL of the gatewayd HTTP endpoint |
| `SHUUL_JOURNALD_CONTAINER` | `traefik` | Value matched against the `CONTAINER_NAME` field |

Because the default is `localhost`, a containerized shuul **must** set `SHUUL_JOURNALD_URL` to a host-reachable address.

**Both addresses work.** gatewayd listens on all interfaces (the default — see 2.7), so `host.docker.internal:host-gateway` and an explicit bridge address such as `172.17.0.1:19531` both reach it. Deciding *who* is allowed to connect is the firewall's job, not the listen address's — see 2.7.

**Degraded mode (fail-open):** if gatewayd is unreachable, shuul logs `journald gateway not available` and retries with a 30-second backoff. The process does not crash — the WAF pipeline keeps working and only the Jail pipeline stops receiving observations (so rate limiting stays inactive until the reader reconnects). This is consistent with the fire-and-forget design of the Jail pipeline.

### 2.7 Security hardening

`systemd-journal-gatewayd` performs **no authentication and no authorization**: anyone who can open a TCP connection to port `19531` can read the journal. Treat it as an unauthenticated internal service.

- **Never expose it to the Internet.** Do not publish port `19531` through Traefik and do not open it publicly in your firewall.
- **Keep the default listen address; make the firewall the single point of enforcement.** gatewayd listens on every interface out of the box. That is acceptable on a private host as long as the firewall only lets the Docker bridge subnets reach `19531` and blocks the rest.
- **Find the subnet(s) to allow:**

  ```bash
  # Subnet of your Docker bridge networks
  docker network inspect proxy -f '{{(index .IPAM.Config 0).Subnet}}'
  ```

  Repeat for every bridge network your containers use. The default `docker0` bridge is usually `172.17.0.0/16`.
- **Allow only those subnets with `ufw`:**

  ```bash
  sudo ufw allow from 172.17.0.0/16 to any port 19531 proto tcp
  ```

  With `nftables` or `iptables` the idea is the same: accept `19531` from the bridge subnets and drop it everywhere else. If your firewall's default inbound policy is not *deny*, add an explicit drop for `19531` as well.
- **Remember what the journal contains:** request paths, query strings (which may carry tokens or API keys), headers, user agents and client IPs. Anyone who can reach gatewayd can read all of it — restrict access accordingly.

**Advanced alternative — bind instead of firewalling.** If you prefer to pin the socket to a single address, override `ListenStream` with a drop-in:

```ini
# /etc/systemd/system/systemd-journal-gatewayd.socket.d/override.conf
[Socket]
ListenStream=
ListenStream=172.17.0.1:19531
```

```bash
sudo systemctl daemon-reload
sudo systemctl restart systemd-journal-gatewayd.socket
```

The empty `ListenStream=` resets the inherited value (a drop-in cannot remove a setting, and repeated assignments append), so the second line becomes the only address the socket listens on. Be aware that this adds a second place to keep in sync: the pinned address must be one your shuul container actually reaches, and `host.docker.internal` does not always resolve to it. Prefer the firewall rule unless you have a specific reason to bind.

---

## 3. OIDC Provider Setup

Shuul requires an OIDC provider for authentication. [PocketID](https://github.com/atareao/pocketid) is the recommended option.

### 3.1 Create a client in your OIDC provider

| Field | Value |
|---|---|
| Client ID | `shuul` (or your preference) |
| Client Secret | Generate a random secret |
| Redirect URI | `https://shuul.yourdomain.com/api/v1/auth/callback` |
| Grant Type | Authorization Code |
| Scopes | `openid`, `profile`, `email` |

### 3.2 Verify OIDC endpoints

Your provider should expose:
- `https://auth.yourdomain.com/.well-known/openid-configuration`

Shuul fetches metadata and JWKS automatically on startup.

---

## 4. MaxMind GeoIP Setup (Optional)

GeoIP features require the [GeoLite2 City database](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data/).

```bash
# Download the database
wget -O geo/GeoLite2-City.tar.gz "https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-City&license_key=YOUR_KEY&suffix=tar.gz"
tar -xzf geo/GeoLite2-City.tar.gz -C geo/
mv geo/GeoLite2-City_*/GeoLite2-City.mmdb geo/
rm -rf geo/GeoLite2-City_* geo/GeoLite2-City.tar.gz
```

Mount the file at the container path specified in `MAXMIND_DB_PATH` (default: `geo/GeoLite2-City.mmdb`).

---

## 5. Directory Structure

```
/path/to/shuul/
├── compose.yml              # Docker Compose file
├── .env                     # Environment variables
├── geo/
│   └── GeoLite2-City.mmdb   # MaxMind database (optional)
└── data/
    └── shuul.db              # SQLite database (auto-created)
```

---

## 6. Configuration

### 6.1 Environment Variables

Create a `.env` file:

```bash
# Required
SECRET=generate-a-random-string-with-at-least-32-chars
OIDC_ISSUER_URL=https://auth.yourdomain.com
OIDC_CLIENT_ID=shuul
OIDC_CLIENT_SECRET=your-client-secret
OIDC_REDIRECT_URL=https://shuul.yourdomain.com/api/v1/auth/callback

# Optional
DATABASE_URL=sqlite:///app/data/shuul.db?mode=rwc
PORT=3000
MAXMIND_DB_PATH=geo/GeoLite2-City.mmdb
RUST_LOG=info

# Optional — Jail pipeline feed from systemd-journal-gatewayd (see section 2)
SHUUL_JOURNALD_URL=http://host.docker.internal:19531
SHUUL_JOURNALD_CONTAINER=traefik
```

`SHUUL_JOURNALD_URL` and `SHUUL_JOURNALD_CONTAINER` are **optional**: if you omit them, the journald reader falls back to `http://localhost:19531` and `traefik` respectively. Note that `localhost` inside a container refers to the container itself, so a containerized shuul must set `SHUUL_JOURNALD_URL` to a host-reachable address.

### 6.2 Docker Compose

The project ships with a `compose.yml` ready for production:

```yaml
services:
  shuul:
    image: atareao/shuul:latest
    container_name: shuul
    restart: unless-stopped
    env_file: .env
    volumes:
      - ./data:/app/data           # SQLite database
      - ./geo:/app/geo             # MaxMind GeoIP database
    networks:
      - proxy                      # Traefik network
    healthcheck:
      test: curl -f http://localhost:3000/api/v1/health/
      interval: 60s
      timeout: 5s
      retries: 3
    labels:
      - traefik.enable=true
      - traefik.http.routers.shuul.rule=Host(`${FQDN}`)
      - traefik.http.routers.shuul.entrypoints=https
      - traefik.http.services.shuul.loadbalancer.server.port=3000

networks:
  proxy:
    external: true
```

If you feed the Jail pipeline from the systemd journal, also add the `extra_hosts` entry and the `SHUUL_JOURNALD_*` variables shown in **section 2.6** (see **section 14** for the full reference).

### 6.3 Production Checklist

- [ ] **SECRET** is a strong random string (`openssl rand -hex 32`)
- [ ] OIDC redirect URL uses HTTPS
- [ ] Traefik network is created and shared
- [ ] Data directory permissions are correct (container runs as `app` user, UID 10001)
- [ ] MaxMind database is downloaded and mounted
- [ ] `RUST_LOG` is set to `info` or `warn` (not `debug`) in production
- [ ] Health check is working
- [ ] Container restarts automatically
- [ ] `systemd-journal-gatewayd.socket` is enabled, reachable from the container, and bound to a private address (only if using the journald feed — see section 2)

---

## 7. First Run

### 7.1 Start the container

```bash
docker compose up -d
```

### 7.2 Check the logs

```bash
docker compose logs -f
```

Expected output:
```
🚀 Server started successfully
```

### 7.3 Verify health

```bash
curl https://shuul.yourdomain.com/api/v1/health/
# → "Up and running"
```

### 7.4 Access the dashboard

Navigate to `https://shuul.yourdomain.com` in your browser. Click "Sign in with PocketID" and authenticate.

### 7.5 Apply initial rules

1. Go to **Templates** → **WAF Templates**
2. Search for templates matching your services (WordPress, Nextcloud, etc.)
3. Click **Apply** for each template
4. Repeat for **Jail Templates**
5. Go to **Rules** and verify they're active

---

## 8. Protecting a Service

Here's a complete example protecting a WordPress instance:

### 8.1 Add the middleware chain

```yaml
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
    wordpress:
      rule: "Host(`blog.example.com`)"
      service: wordpress
      middlewares:
        - shuul-auth
        - shuul-reporter
```

### 8.2 Apply relevant templates in shuul

1. **WAF**: WordPress - wp-login, WordPress - xmlrpc, WordPress - wp-admin
2. **Jail**: Auth Brute Force (for wp-login)
3. **Custom rule**: Block countries with no business presence

### 8.3 Test with log_only mode

Set new rules to `log_only` first, check the logs, then switch to `enforce`.

---

## 9. Upgrading

```bash
docker compose pull
docker compose up -d
```

SQLite migrations run automatically on startup. The database file is never modified in a backward-incompatible way without a migration.

To check the current version:

```bash
curl -s https://shuul.yourdomain.com/ | grep -o "Shuul ([0-9.]*)"
```

---

## 10. Backup and Restore

### 10.1 Backup

```bash
#!/bin/bash
BACKUP_DIR="/backups/shuul"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p "$BACKUP_DIR"

# Backup SQLite database
cp /path/to/shuul/data/shuul.db "$BACKUP_DIR/shuul_$DATE.db"

# Backup environment (exclude secrets if needed)
cp /path/to/shuul/.env "$BACKUP_DIR/env_$DATE.txt"

# Export rules
curl -s -H "Authorization: Bearer $TOKEN" \
  https://shuul.yourdomain.com/api/v1/rules/export \
  > "$BACKUP_DIR/rules_$DATE.json"

# Keep only last 30 backups
ls -t "$BACKUP_DIR/shuul_*.db" | tail -n +31 | xargs rm -f
```

### 10.2 Restore

```bash
# Stop shuul
docker compose down

# Restore database
cp /backups/shuul/shuul_20250101_120000.db /path/to/shuul/data/shuul.db

# Restart
docker compose up -d
```

---

## 11. Monitoring

### Health Check

```bash
curl -f http://localhost:3000/api/v1/health/
```

### Log Viewer (Dashboard)

Access the **Logs** page at `/admin/logs` for a real-time view of WAF and Jail events. The log viewer requires no SSH access — all events are captured in an in-memory ring buffer and served through the API.

Features:
- **9-column table**: timestamp, event type, pipeline, IP, country, rule, path, method, status code
- **Auto-refresh**: polls every 3 seconds for near real-time updates
- **Event filter buttons**: dynamically generated based on event types present (block, allow, safe_path, report_ban, etc.)
- **Expandable rows**: click any row to see the full JSON event payload
- **Configurable capacity**: change the buffer size at runtime (1000, 5000, 10000, 20000 entries)
- **No persistence**: buffer is lost on container restart

### Log Levels

| Level | Use Case |
|---|---|
| `error` | Production — only errors |
| `warn` | Production — errors + warnings |
| `info` | Default — normal operation info |
| `debug` | Development — detailed debugging |
| `trace` | Extreme debugging — all matching details |

Set via `RUST_LOG` environment variable.

### Key Log Patterns

| Pattern | Meaning |
|---|---|
| `[ALLOW]` | Request passed through WAF |
| `[BLOCK]` | Request blocked by WAF rule |
| `[BANNED]` | Request blocked because IP is banned |
| `[REPORT_BLOCK]` | Jail pipeline counted a failure |
| `[REPORT_BAN]` | Jail pipeline banned an IP |
| `[SAFE_PATH]` | Request matched a safe path |
| `[TRUSTED_IP]` | Request from a trusted IP |
| `[TRUSTED_UA]` | Request from a trusted user agent |

---

## 12. Troubleshooting

### 12.1 "401 Unauthorized" on dashboard

- Check that your OIDC provider is running
- Verify `OIDC_ISSUER_URL` is reachable from the shuul container
- Check logs for OIDC initialization: `grep OIDC docker compose logs shuul`

### 12.2 WAF not blocking requests

- Ensure the rule is `active`
- Set `mode` to `enforce` (not `log_only`)
- Check rule weight — lower weight rules run first
- Verify the request matches the rule's filter criteria

### 12.3 Rate limiting not working

- Verify the rule has a `rate_limit_profile_id` set
- Check the profile's `fail_codes` include the status code your backend returns
- Ensure the Jail pipeline is receiving data: the `systemd-journal-gatewayd` reader must be connected (see section 2) and Traefik must be logging access logs in JSON to journald (see 2.1)
- Verify `SHUUL_JOURNALD_URL` and `SHUUL_JOURNALD_CONTAINER` point at the right host and container name (see 2.6)
- Check the shuul logs for `[REPORT_BLOCK]` / `[REPORT_BAN]` entries, and for `journald gateway not available`
- **Deprecated path:** if you still use the `shuul-reporter` plugin, ensure it is in the middleware chain **after** the backend, and check the plugin logs on the Traefik side

### 12.4 Database issues

SQLite stores data in a single file. If corrupt:

```bash
# Stop shuul
docker compose down

# Check integrity
sqlite3 data/shuul.db "PRAGMA integrity_check;"

# Recover if needed (last resort)
sqlite3 data/shuul.db ".clone data/shuul_recovered.db"

# Start fresh (all data lost)
mv data/shuul.db data/shuul.db.bak
docker compose up -d
```

### 12.5 "Failed to load bans from DB"

Ban format may have changed between versions. The application falls back to an empty ban manager — existing bans are logged as a warning. No data loss occurs.

### 12.6 "Unit systemd-journal-gatewayd.socket not found" / gatewayd not reachable

Symptoms: `systemctl enable --now systemd-journal-gatewayd.socket` fails with `Unit systemd-journal-gatewayd.socket not found`, or the shuul logs keep repeating `journald gateway not available`.

- On Debian/Ubuntu the units come from a **separate package**. Install it and retry:

  ```bash
  sudo apt update && sudo apt install -y systemd-journal-remote
  sudo systemctl enable --now systemd-journal-gatewayd.socket
  ```

- Confirm the socket is listening on `19531`:

  ```bash
  sudo systemctl status systemd-journal-gatewayd.socket
  ```

  The status must show `active (listening)` and the `19531` port.

- Test the endpoint from the host:

  ```bash
  curl -s "http://localhost:19531/entries?CONTAINER_NAME=traefik" -H "Accept: application/json"
  ```

  If it returns nothing, Traefik is not shipping JSON access logs to the journal (see 2.1).

- Test the endpoint from inside the shuul container:

  ```bash
  docker compose exec shuul curl -s "$SHUUL_JOURNALD_URL/entries?CONTAINER_NAME=$SHUUL_JOURNALD_CONTAINER" \
    -H "Accept: application/json"
  ```

  If this fails while the host test works, it's a network problem: make sure the firewall allows `19531` from the container's bridge subnet (see 2.7) and that `SHUUL_JOURNALD_URL` points at the host — not at `localhost`.

- Shuul degrades gracefully: with gatewayd down, the WAF keeps working and only the Jail pipeline stops receiving data. Nothing crashes, so every upstream symptom points back to the checks above.

---

## 13. Security Considerations

- **Shuul does not rate-limit itself.** Traefik's ForwardAuth is synchronous — slow responses from shuul affect all protected services.
- The `SECRET` is used for JWT signing. Rotate it periodically.
- OIDC tokens expire after 60 minutes. Users are redirected to re-authenticate.
- SQLite is not designed for multi-instance deployments. Run a single shuul container.
- Traefik's `trustForwarders: true` is required for correct IP extraction behind reverse proxies.
- The shuul-reporter plugin sends async requests — network issues do not affect backend latency.
- **`systemd-journal-gatewayd` has no authentication.** Never expose port `19531` publicly — keep it off Traefik and restrict it to the Docker bridge subnets with the firewall (see 2.7).
- **The journal holds sensitive data** — request paths, query strings, headers, user agents and client IPs. Anyone who can reach gatewayd can read it, so restrict access accordingly.

---

## 14. Reference: compose.yml Reference

```yaml
services:
  shuul:
    image: atareao/shuul:latest
    container_name: shuul
    restart: unless-stopped
    env_file: .env
    extra_hosts:
      - "host.docker.internal:host-gateway"   # reach systemd-journal-gatewayd on the host
    environment:
      - SHUUL_JOURNALD_URL=http://host.docker.internal:19531   # optional (default: http://localhost:19531)
      - SHUUL_JOURNALD_CONTAINER=traefik                       # optional (default: traefik)
    volumes:
      - ./data:/app/data:Z           # SQLite database
      - ./geo:/app/geo:Z             # GeoIP database
    networks:
      - proxy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/v1/health/"]
      interval: 60s
      timeout: 5s
      retries: 3
      start_period: 10s
    labels:
      - traefik.enable=true
      - traefik.http.routers.shuul.rule=Host(`shuul.example.com`)
      - traefik.http.routers.shuul.entrypoints=https
      - traefik.http.routers.shuul.tls=true
      - traefik.http.services.shuul.loadbalancer.server.port=3000
      - traefik.docker.network=proxy

networks:
  proxy:
    external: true
```

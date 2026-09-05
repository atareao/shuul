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

## 2. OIDC Provider Setup

Shuul requires an OIDC provider for authentication. [PocketID](https://github.com/atareao/pocketid) is the recommended option.

### 2.1 Create a client in your OIDC provider

| Field | Value |
|---|---|
| Client ID | `shuul` (or your preference) |
| Client Secret | Generate a random secret |
| Redirect URI | `https://shuul.yourdomain.com/api/v1/auth/callback` |
| Grant Type | Authorization Code |
| Scopes | `openid`, `profile`, `email` |

### 2.2 Verify OIDC endpoints

Your provider should expose:
- `https://auth.yourdomain.com/.well-known/openid-configuration`

Shuul fetches metadata and JWKS automatically on startup.

---

## 3. MaxMind GeoIP Setup (Optional)

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

## 4. Directory Structure

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

## 5. Configuration

### 5.1 Environment Variables

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
```

### 5.2 Docker Compose

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

### 5.3 Production Checklist

- [ ] **SECRET** is a strong random string (`openssl rand -hex 32`)
- [ ] OIDC redirect URL uses HTTPS
- [ ] Traefik network is created and shared
- [ ] Data directory permissions are correct (container runs as `app` user, UID 10001)
- [ ] MaxMind database is downloaded and mounted
- [ ] `RUST_LOG` is set to `info` or `warn` (not `debug`) in production
- [ ] Health check is working
- [ ] Container restarts automatically

---

## 6. First Run

### 6.1 Start the container

```bash
docker compose up -d
```

### 6.2 Check the logs

```bash
docker compose logs -f
```

Expected output:
```
🚀 Server started successfully
```

### 6.3 Verify health

```bash
curl https://shuul.yourdomain.com/api/v1/health/
# → "Up and running"
```

### 6.4 Access the dashboard

Navigate to `https://shuul.yourdomain.com` in your browser. Click "Sign in with PocketID" and authenticate.

### 6.5 Apply initial rules

1. Go to **Templates** → **WAF Templates**
2. Search for templates matching your services (WordPress, Nextcloud, etc.)
3. Click **Apply** for each template
4. Repeat for **Jail Templates**
5. Go to **Rules** and verify they're active

---

## 7. Protecting a Service

Here's a complete example protecting a WordPress instance:

### 7.1 Add the middleware chain

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

### 7.2 Apply relevant templates in shuul

1. **WAF**: WordPress - wp-login, WordPress - xmlrpc, WordPress - wp-admin
2. **Jail**: Auth Brute Force (for wp-login)
3. **Custom rule**: Block countries with no business presence

### 7.3 Test with log_only mode

Set new rules to `log_only` first, check the logs, then switch to `enforce`.

---

## 8. Upgrading

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

## 9. Backup and Restore

### 9.1 Backup

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

### 9.2 Restore

```bash
# Stop shuul
docker compose down

# Restore database
cp /backups/shuul/shuul_20250101_120000.db /path/to/shuul/data/shuul.db

# Restart
docker compose up -d
```

---

## 10. Monitoring

### Health Check

```bash
curl -f http://localhost:3000/api/v1/health/
```

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

## 11. Troubleshooting

### 11.1 "401 Unauthorized" on dashboard

- Check that your OIDC provider is running
- Verify `OIDC_ISSUER_URL` is reachable from the shuul container
- Check logs for OIDC initialization: `grep OIDC docker compose logs shuul`

### 11.2 WAF not blocking requests

- Ensure the rule is `active`
- Set `mode` to `enforce` (not `log_only`)
- Check rule weight — lower weight rules run first
- Verify the request matches the rule's filter criteria

### 11.3 Rate limiting not working

- Verify the rule has a `rate_limit_profile_id` set
- Check the profile's `fail_codes` include the status code your backend returns
- Ensure the `shuul-reporter` plugin is in the middleware chain **after** the backend
- Check plugin logs on the Traefik side

### 11.4 Database issues

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

### 11.5 "Failed to load bans from DB"

Ban format may have changed between versions. The application falls back to an empty ban manager — existing bans are logged as a warning. No data loss occurs.

---

## 12. Security Considerations

- **Shuul does not rate-limit itself.** Traefik's ForwardAuth is synchronous — slow responses from shuul affect all protected services.
- The `SECRET` is used for JWT signing. Rotate it periodically.
- OIDC tokens expire after 60 minutes. Users are redirected to re-authenticate.
- SQLite is not designed for multi-instance deployments. Run a single shuul container.
- Traefik's `trustForwarders: true` is required for correct IP extraction behind reverse proxies.
- The shuul-reporter plugin sends async requests — network issues do not affect backend latency.

---

## 13. Reference: compose.yml Reference

```yaml
services:
  shuul:
    image: atareao/shuul:latest
    container_name: shuul
    restart: unless-stopped
    env_file: .env
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
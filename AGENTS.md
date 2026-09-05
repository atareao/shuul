# Reglas específicas del proyecto Shuul — AGENTS

## Stack tecnológico

| Capa | Tecnología |
|---|---|
| Backend | Rust + Axum 0.8 + Tokio |
| Base de datos | **SQLite** (via sqlx 0.8, runtime-tokio-rustls) |
| Frontend | React 19 (class components) + TypeScript + Vite 7 |
| UI | Ant Design 6 + @ant-design/charts |
| Contenedor | Docker (multi-stage build: rust:alpine3.23 → node:23-alpine → alpine:3.23) |
| Proxy inverso | Traefik v3.x (ForwardAuth + plugin middleware) |
| Plugin Traefik | [traefik-shuul-reporter](https://github.com/atareao/traefik-shuul-reporter) |
| GeoIP | MaxMind GeoLite2 City (mmdb) + moka LRU cache |
| Auth | OIDC (PocketID) + JWT (HS256, jsonwebtoken crate) |
| i18n | i18next (es, ca, en) |
| Tooling | just (task runner), vampus (version management) |

## Arquitectura: dos pipelines independientes

Shuul opera **dos pipelines** sobre un mismo conjunto de reglas:

| Pipeline | Endpoint | Rol | Comportamiento |
|---|---|---|---|
| **WAF** | `ANY /api/v1/shuul` | ForwardAuth — interceptar, matchear, allow/deny | Primera regla que matchea gana (break por weight ASC) |
| **Jail** | `POST /api/v1/report` | Rate limiter post-factum (fail2ban-style) | TODAS las reglas que matchean cuentan |

### Flujo completo

```
Request → Safe Paths? → ALLOW
        → Trusted IPs? → ALLOW
        → Trusted UAs? → ALLOW
        → Banned IP? → 403 FORBIDDEN
        → WAF rules (first match wins) → 200 OK / 403
        → Backend responde
        → Plugin captura status_code
        → Jail rules (ALL match) → rate limit → ban si excede
```

### WAF (`shuul.rs`)

- **No evalúa rate limits.** Solo matching + allow/deny.
- Primera regla que matchea (por weight ASC) gana. `break` tras encontrar una.
- `mode = "off"` → skip. `mode = "log_only"` → allow=true. `mode = "enforce"` → apply allow/deny.
- IP baneada → 403 FORBIDDEN (antes del matching loop).
- Safe paths, trusted IPs, trusted UAs → ALLOW inmediato (antes de todo).
- No hay concepto de `store` — ya no persiste requests individuales.

### Jail (`report.rs`)

- **Único pipeline que evalúa rate limits.**
- Itera TODAS las reglas (sin break). Cada regla con `rate_limit_profile_id` es un "jail" independiente.
- Para cada match: carga perfil desde DB, si `status_code ∈ fail_codes` → `record()` + ban si excede.
- Fire-and-forget: siempre devuelve 200 OK.
- Recibe `ReportPayload` desde el plugin de Traefik.

## CacheRule

`CacheRule` en `backend/src/models/rule.rs` envuelve una `Rule` con `Option<Regex>` precompilado para cada filtro. **No tiene** `CachedRateLimit` ni campo `rate_limit`.

### Filtros disponibles (14)

```rust
ip_address, protocol, fqdn, path, query,
city_name, country_name, country_code,
user_agent, method, referer, content_type,
accept_language, x_request_id
```

### Lógica de `matches()`

Todos los filtros se evalúan con AND. Si el regex de la regla es `None` → condición se cumple. Si el valor del request es `None` → condición se cumple. Ambos deben existir para que el regex se evalúe.

```rust
check_match(self.ip_address.as_ref(), request.ip_address.as_ref())
    && check_match(self.protocol.as_ref(), request.protocol.as_ref())
    && check_match(self.fqdn.as_ref(), request.fqdn.as_ref())
    && check_match(self.path.as_ref(), request.path.as_ref())
    && check_match(self.query.as_ref(), request.query.as_ref())
    && check_match(self.city_name.as_ref(), request.city_name.as_ref())
    && check_match(self.country_name.as_ref(), request.country_name.as_ref())
    && check_match(self.country_code.as_ref(), request.country_code.as_ref())
    && check_match(self.user_agent.as_ref(), request.user_agent.as_ref())
    && check_match(self.method.as_ref(), request.method.as_ref())
    && check_match(self.referer.as_ref(), request.referer.as_ref())
    && check_match(self.content_type.as_ref(), request.content_type.as_ref())
    && check_match(self.accept_language.as_ref(), request.accept_language.as_ref())
    && check_match(self.x_request_id.as_ref(), request.x_request_id.as_ref())
```

## Base de datos — SQLite

Shuul usa **SQLite** (NO PostgreSQL). Puntos clave:

- Single file, zero administration.
- Pool de 5 conexiones max (`SqlitePoolOptions::new().max_connections(5)`).
- Migraciones automáticas al arrancar (sqlx migrate).
- `DATABASE_URL` por defecto: `sqlite:///app/data/shuul.db?mode=rwc`
- `PRAGMA` features: mode rwc (read-write-create).
- NO hay tablas de requests individuales. Sólo: `rules`, `rate_limit_profiles`, `bans`, `settings`, `stats_cache`.

### Migraciones

```
backend/migrations/
├── 20260902000000_initial_schema.up.sql       # Schema completo inicial
├── 20260903000000_add_log_all_requests_setting.up.sql
├── 20260903000001_update_rate_limit_profiles.up.sql
└── 20260903000002_add_scanner_aggressive_profile.up.sql
```

### Tablas

| Tabla | Propósito |
|---|---|
| `rules` | Reglas WAF + Jail (14 filtros, rate_limit_profile_id FK) |
| `rate_limit_profiles` | Perfiles de rate limiting (max_retry, find_time, fail_codes, escalado) |
| `bans` | Baneos persistentes (IP, rule_id, duración, nivel de escalado) |
| `settings` | Configuración clave-valor (safe_paths, trusted_ips, etc.) |
| `stats_cache` | Snapshot JSON de estadísticas (persistido cada 30 min) |

## AppState — Estado compartido

```rust
pub struct AppState {
    pub pool: SqlitePool,
    pub secret: String,
    pub geoip: GeoIpService,
    pub rules: Mutex<Vec<CacheRule>>,
    pub stats: StatsCollector,
    pub static_dir: String,
    pub ban_manager: Mutex<BanManager>,
    pub rate_limiter: Mutex<HashMap<i32, RateLimiter>>,
    pub settings: Mutex<Settings>,
    pub oidc_metadata: tokio::sync::RwLock<Option<OidcMetadata>>,
    pub jwt_validator: tokio::sync::RwLock<Option<JwtValidator>>,
    pub oidc_states: tokio::sync::Mutex<HashMap<String, (String, Instant)>>,
    pub oidc_client_id: Option<String>,
    pub oidc_redirect_url: Option<String>,
}
```

## Concurrencia

Todos los `MutexGuard` se liberan antes de cualquier `.await`. El orden de adquisición de locks es siempre:

```
rules → rate_limiter → ban_manager
```

Nunca se adquiere un lock en orden inverso para evitar deadlocks. Este patrón se ve explícitamente en `shuul.rs` y `report.rs` — todo el trabajo síncrono (lock, read, match, release) se hace dentro de un bloque `{ }` cuyo ámbito termina antes de cualquier operación async.

## Background Tasks

| Tarea | Intervalo | Propósito |
|---|---|---|
| OIDC init | Cada 30s (hasta éxito) | Fetch metadata + JWKS del proveedor OIDC |
| Ban cleanup | Cada 60s | Eliminar bans expirados, limpiar rate limiters |
| Stats persist | Cada 1800s (30 min) | Guardar snapshot de StatsCollector en SQLite |

## StatsCollector

Colector de estadísticas en memoria con:

- `AtomicU64` para totales (allowed/blocked) — sin locks
- `Mutex<HashMap>` para top N (rules, countries, methods, paths, FQDNs)
- 3 series temporales (minute: 60 buckets, hour: 24 buckets, day: 31 buckets)
- Series por método HTTP (method_series)
- `record_blocked()` y `record_allowed()` desde WAF y Jail pipelines
- Snapshot a SQLite cada 30 min (JSON en tabla `stats_cache`)
- Carga del snapshot al arrancar

## API Surface

### Públicos (sin auth)

| Method | Path | Handler | Descripción |
|---|---|---|---|
| ANY | `/api/v1/shuul` | `shuul` | WAF pipeline |
| POST | `/api/v1/report` | `report_handler` | Jail pipeline (fire-and-forget) |
| GET | `/api/v1/health` | `check_health` | Health check |
| GET | `/api/v1/util/complete?ip=` | `complete_ip` | GeoIP lookup |
| GET | `/api/v1/auth/sso` | `sso_redirect` | OIDC redirect |
| GET | `/api/v1/auth/callback` | `callback_handler` | OIDC callback |
| GET | `/api/v1/auth/sso-status` | `sso_status` | Estado de SSO |
| GET | `/api/v1/templates` | `list_templates` | Listar templates |

### Protegidos (JWT Bearer)

| Method | Path | Descripción |
|---|---|---|
| GET/POST | `/api/v1/rules` | Listar / Crear reglas |
| PATCH/DELETE | `/api/v1/rules` | Actualizar / Eliminar regla |
| GET | `/api/v1/rules/export` | Exportar reglas como JSON |
| POST | `/api/v1/rules/import` | Importar reglas (upsert por name) |
| GET | `/api/v1/rules/info` | Conteo de reglas |
| GET/POST | `/api/v1/rate-limit-profiles` | Listar / Crear perfiles |
| PATCH/DELETE | `/api/v1/rate-limit-profiles` | Actualizar / Eliminar perfil |
| GET/POST | `/api/v1/bans` | Listar / Crear baneo |
| DELETE | `/api/v1/bans` | Eliminar baneo |
| GET | `/api/v1/bans/info` | Conteo de baneos |
| GET/PUT | `/api/v1/settings` | Leer / Actualizar settings |
| GET | `/api/v1/stats/*` | Estadísticas (info, evolution, top_*) |

## Frontend — React Class Components

### Páginas

| Ruta | Componente | Descripción |
|---|---|---|
| `/` | HomePage | Landing page pública |
| `/login` | LoginPage | SSO redirect |
| `/admin/` | DashboardPage | Resumen del sistema |
| `/admin/rules` | RulesPage | CRUD de reglas |
| `/admin/rate-limit-profiles` | RateLimitProfilesPage | CRUD de perfiles |
| `/admin/bans` | BansPage | Baneos activos |
| `/admin/templates` | TemplatesPage | Biblioteca de templates |
| `/admin/charts` | ChartsPage | Analíticas y gráficos |
| `/admin/settings` | SettingsPage | Configuración global |
| `/admin/logout` | LogoutPage | Cerrar sesión |

### Componentes clave

- **CustomTable** — Tabla CRUD genérica con paginación servidor, sorting, filtros, auto-refresh
- **CustomDialog** — Modal genérico que auto-genera formularios desde `FieldDefinition<T>`
- **RuleDialog** — Modal especializado con 4 tabs (General, Network, Location, Request)
- **RateLimitProfileDialog** — Modal especializado con 2 tabs (General, Penalty)
- **AuthContext** — Contexto de autenticación JWT (token, login, logout, auto-logout timer)
- **ModeContext** — Contexto de tema (dark/light, persistido en localStorage)

### Convenciones TypeScript

Ver sección completa en AGENTS.md más abajo.

## TypeScript

### `debounce` siempre debe tiparse con `.cancel()`

```typescript
import type { DebouncedFn } from '@/common/utils';

// ✅ BIEN
private debouncedSetFilter: DebouncedFn<(key: string, value: string) => void>;

// ❌ MAL (TypeScript error TS2339: Property 'cancel' does not exist)
private debouncedSetFilter: (key: string, value: string) => void;
```

### `loadData` con query params: usar `Map`, no embeker en URL

```typescript
// ❌ MAL: query params en el endpoint
loadData(`requests/evolution?unit=${unit}&last=${last}`)

// ✅ BIEN: query params como Map
loadData("requests/evolution", new Map([["unit", unit], ["last", last.toString()]]))
```

### Llamadas API secuenciales: usar `Promise.all()`

```typescript
// ❌ MAL: ~1.5s de carga
const a = await loadData(...);
const b = await loadData(...);
const c = await loadData(...);

// ✅ BIEN: ~300ms (lo que dure la más lenta)
const [a, b, c] = await Promise.all([
    loadData(...),
    loadData(...),
    loadData(...),
]);
```

### `componentDidUpdate`: early return para cambios irrelevantes

```typescript
componentDidUpdate = async (prevProps, prevState) => {
    if (prevState.loading !== this.state.loading || prevState.items !== this.state.items) {
        return;
    }
    // ... resto de la lógica
}
```

### `clientFilter` y `extraHeaderContent` en CustomTable

```typescript
<CustomTable<Item>
    ...
    extraHeaderContent={<Select ... />}
    clientFilter={(items) => items.filter(item => ...)}
/>
```

### `type: 'tag'` en FieldDefinition

```typescript
{
    key: 'type',
    label: 'Type',
    type: 'tag',
    options: [
        { value: 'waf', label: 'WAF', color: 'blue' },
        { value: 'jail', label: 'Jail', color: 'green' },
        { value: 'both', label: 'WAF+Jail', color: 'purple' },
    ],
}
```

### `getRuleType()` helper

```typescript
import { getRuleType } from "@/models/rule";
const ruleType = getRuleType(rule); // "waf" | "jail" | "both"
```

Lógica: si tiene `rate_limit_profile_id` → jail. Si tiene algún filtro → waf. Si ambos → both.

## Docker

### Multi-stage build

1. **backend-builder**: `rust:alpine3.23` — compila el backend Rust
2. **frontend-builder**: `node:23-alpine` — build del frontend con pnpm
3. **runtime**: `alpine:3.23` — copia binario + static + migrations, expone puerto 3000

### compose.yml

- Imagen: `atareao/shuul`
- Volúmenes: `data` (SQLite), `geo` (MaxMind, external)
- Red: `proxy` (external, para Traefik)
- Healthcheck: `curl -f http://localhost:3000/api/v1/health/` cada 60s
- Labels Traefik: router rule, entrypoint https, loadbalancer port 3000

### Variables de entorno producción

| Variable | Requerida | Descripción |
|---|---|---|
| `SECRET` | ✅ | JWT signing secret |
| `OIDC_ISSUER_URL` | ✅ | URL del proveedor OIDC |
| `OIDC_CLIENT_ID` | ✅ | Client ID |
| `OIDC_CLIENT_SECRET` | ✅ | Client secret |
| `OIDC_REDIRECT_URL` | ❌ | Callback URL |
| `DATABASE_URL` | ❌ | SQLite path (default: sqlite:///app/data/shuul.db?mode=rwc) |
| `PORT` | ❌ | Puerto (default: 3000) |
| `MAXMIND_DB_PATH` | ❌ | Ruta a GeoIP database |
| `RUST_LOG` | ❌ | Nivel de log |

## Templates

81 plantillas de reglas y 9 perfiles de rate limit preconfigurados.

### Categorías WAF (35 templates)

WordPress, Drupal, Laravel, GraphQL, Adminer, Sensitive Files, Known Bots, Geo blocking, SSTI, SQLi, XSS, Path Traversal, Log4j, Swagger, Symfony, Docker socket, etc.

### Categorías Jail (46 templates)

Auth Brute Force, Admin Guard, Path Scanning, API Abuse, Scraping, Global Shield, Scanner Aggressive - con paths específicos para cada servicio (wp-login, grafana, nextcloud, jenkins, kubernetes, portainer, etc.)

### Perfiles Rate Limit (9 perfiles)

| ID | Nombre | max_retry | window | ban_time | fail_codes |
|---|---|---|---|---|---|
| 1 | Auth Brute Force | 5 | 300s | 900s | 401 |
| 2 | Admin Guard | 5 | 300s | 3600s | 401,403 |
| 3 | Path Scanning | 20 | 60s | 300s | 403,404 |
| 4 | API Abuse | 100 | 60s | 300s | 401,403,429 |
| 5 | Scraping | 60 | 60s | 300s | 403,429,500 |
| 6 | Health & Webhooks | 100 | 60s | 60s | 500,502,503 |
| 7 | Recidive | 3 | 172800s | 604800s | 403,429 |
| 8 | Global Shield | 300 | 60s | 300s | 403,404,429,500,502,503 |
| 9 | Scanner Aggressive | 50 | 10s | 1800s | 403,404,405,500 |

## Plugin Traefik: traefik-shuul-reporter

Plugin externo en Go que captura el status code del backend y lo reporta a shuul vía `POST /api/v1/report`.

### Middleware chain

```yaml
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
  my-app:
    middlewares:
      - shuul-auth      # 1º: WAF (allow/deny)
      - shuul-reporter  # 2º: Report (rate limit)
```

## Configuración de Settings

| Clave | Tipo | Descripción |
|---|---|---|
| `safe_paths` | Vec<String> | Patrones regex de paths que bypassan reglas |
| `trusted_ips` | Vec<IpNet> | CIDR que bypassan reglas |
| `trusted_user_agents` | Vec<String> | Patrones regex de UA que bypassan reglas |
| `default_rule_mode` | String | Modo por defecto para nuevas reglas |
| `log_retention_days` | i32 | Días de retención de logs (1-365) |
| `log_all_requests` | String | Nivel de log: all, pass, audit |

## Documentación

| Archivo | Audiencia | Contenido |
|---|---|---|
| `README.md` | General | Visión general, features, quick start, API reference |
| `SELF-HOSTING.md` | Administradores | Despliegue completo, Traefik, OIDC, backup, troubleshooting |
| `USER-GUIDE.md` | Usuarios finales | Dashboard, reglas, perfiles, bans, templates, charts, FAQ |
| `AGENTS.md` | AI agents | Arquitectura técnica, convenciones de código, reglas del proyecto |
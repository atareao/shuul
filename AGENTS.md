# Reglas específicas del proyecto Shuul

## Arquitectura: dos pipelines independientes

Shuul opera **dos pipelines** sobre un mismo conjunto de reglas:

| Pipeline | Endpoint | Rol | Comportamiento |
|---|---|---|---|
| **WAF** | `POST /api/v1/shuul` | ForwardAuth — interceptar, matchear, allow/deny | Primera regla que matchea gana (break) |
| **Jail** | `POST /api/v1/report` | Rate limiter post-factum (fail2ban-style) | TODAS las reglas que matchean cuentan |

### WAF (`shuul.rs`)

- **No evalúa rate limits.** Solo matching + allow/store.
- Primera regla que matchea (por weight ASC) gana. `break` tras encontrar una.
- `mode = "off"` → skip. `mode = "log_only"` → allow=true. `mode = "enforce"` → apply allow/store.
- IP baneada → 403 FORBIDDEN (antes del matching loop).
- Safe paths, trusted IPs, trusted UAs → ALLOW inmediato (antes de todo).

### Jail (`report.rs`)

- **Único pipeline que evalúa rate limits.**
- Itera TODAS las reglas (sin break). Cada regla con `rate_limit_profile_id` es un "jail" independiente.
- Para cada match: carga perfil, si `status_code ∈ fail_codes` → `record()` + ban si excede.
- Fire-and-forget: siempre devuelve 200 OK.

## CacheRule

`CacheRule` en `backend/src/models/rule.rs` **no tiene** `CachedRateLimit` ni `rate_limit` field.
El JOIN a `rate_limit_profiles` se eliminó. `read_all_active()` hace `SELECT * FROM rules WHERE active = TRUE`.

### Filtros disponibles en CacheRule

Todos son `Option<Regex>` precompilados desde los strings de la regla:

```
ip_address, protocol, fqdn, path, query,
city_name, country_name, country_code,
user_agent, method, referer, content_type,
accept_language, x_request_id
```

### `matches()` incluye los 14 filtros

```rust
check_match(self.ip_address.as_ref(), request.ip_address.as_ref())
    && check_match(self.protocol.as_ref(), request.protocol.as_ref())
    && ...
    && check_match(self.x_request_id.as_ref(), request.x_request_id.as_ref())
```

## Concurrencia

Todos los `MutexGuard` se liberan antes de cualquier `.await`. El orden de adquisición de locks es siempre:

```
rules → rate_limiter → ban_manager
```

Nunca se adquiere un lock en orden inverso para evitar deadlocks.

## TypeScript

### `debounce` siempre debe tiparse con `.cancel()`

La función `debounce` en `frontend/src/common/utils.ts` devuelve una función con un método `.cancel()`. Si no se tipa correctamente, TypeScript se queja al llamar a `.cancel()` en `componentWillUnmount`.

**Solución:** Usar la interfaz `DebouncedFn<T>` exportada desde `utils.ts`:

```typescript
import type { DebouncedFn } from '@/common/utils';

// ✅ BIEN
private debouncedSetFilter: DebouncedFn<(key: string, value: string) => void>;

// ❌ MAL (TypeScript error TS2339: Property 'cancel' does not exist)
private debouncedSetFilter: (key: string, value: string) => void;
```

### `loadData` con query params: usar `Map`, no embeker en URL

`loadData(endpoint, paramsMap)` construye la URL con `BASE_URL` y serializa el `Map` como query params con `URLSearchParams`. **NUNCA** embebas query params en el string del endpoint:

```typescript
// ❌ MAL: query params en el endpoint
loadData(`requests/evolution?unit=${unit}&last=${last}`)

// ✅ BIEN: query params como Map
loadData("requests/evolution", new Map([["unit", unit], ["last", last.toString()]]))
```

### Llamadas API secuenciales: usar `Promise.all()`

Cuando un componente necesita cargar datos de múltiples endpoints en `componentDidMount`, **NUNCA** uses `await` secuenciales. Usa `Promise.all()` para paralelizar:

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

Cuando `loading` o `items` cambian en un `componentDidUpdate`, no se necesita hacer nada más (es el resultado de un fetch). Añadir un early return al inicio:

```typescript
componentDidUpdate = async (prevProps, prevState) => {
    // Early return: ignorar cambios solo en loading o items
    if (prevState.loading !== this.state.loading || prevState.items !== this.state.items) {
        return;
    }
    // ... resto de la lógica
}
```

### `clientFilter` y `extraHeaderContent` en CustomTable

Cuando se necesita filtrar datos en cliente (ej: por tipo de regla), usar las props:

```typescript
<CustomTable<Item>
    ...
    extraHeaderContent={<Select ... />}
    clientFilter={(items) => items.filter(item => ...)}
/>
```

`clientFilter` recibe los items del estado y devuelve los filtrados. `extraHeaderContent` se renderiza debajo del botón de crear en el header.

### `type: 'tag'` en FieldDefinition

Para columnas que muestran badges de colores, usar `type: 'tag'` con `options`:

```typescript
{
    key: 'type',
    label: 'Type',
    type: 'tag',
    options: [
        { value: 'waf', label: 'WAF', color: 'blue' },
        { value: 'jail', label: 'Jail', color: 'green' },
    ],
}
```

### `getRuleType()` helper

En `frontend/src/models/rule.ts`:

```typescript
import { getRuleType } from "@/models/rule";

const ruleType = getRuleType(rule); // "waf" | "jail" | "both"
```

Lógica: si tiene `rate_limit_profile_id` → jail. Si tiene algún filtro → waf. Si ambos → both.
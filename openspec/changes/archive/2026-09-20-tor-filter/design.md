# Design: Tor Network IP Filter

## TorService

### Estructura

```rust
pub struct TorService {
    exit_nodes: RwLock<HashSet<IpAddr>>,
    client: reqwest::Client,
}
```

- `RwLock<HashSet<IpAddr>>`: permite lecturas concurrentes sin bloqueo, escritura exclusiva durante refresh.
- `reqwest::Client`: reutilizable, con timeout de 10s.

### API pública

```rust
impl TorService {
    /// Crea un TorService con conjunto vacío (lazy init).
    pub fn new() -> Self;

    /// Descarga la lista de Tor exit nodes desde check.torproject.org.
    /// Si falla, mantiene el conjunto anterior (stale).
    pub async fn refresh(&self);

    /// Comprueba si una IP es un Tor exit node.
    /// Returns None si el conjunto aún no se ha cargado (lazy).
    /// Returns Some(true/false) si el conjunto está cargado.
    pub fn is_exit_node(&self, ip: &str) -> Option<bool>;
}
```

### Formato de la lista

`https://check.torproject.org/torbulkexitlist` devuelve texto plano, una IP por línea:

```
185.220.101.1
185.220.101.2
...
```

Se parsea con `ip.parse::<IpAddr>()`, ignorando líneas inválidas.

### Integración en AppState

```rust
pub struct AppState {
    // ... existing fields ...
    pub tor_service: TorService,
}
```

### Background task

```rust
// En main.rs, tras crear app_state:
let tor_state = Arc::clone(&app_state);
tokio::spawn(async move {
    // Primer refresh a los 30s (lazy)
    tokio::time::sleep(Duration::from_secs(30)).await;
    tor_state.tor_service.refresh().await;

    // Refresh cada 30 min
    let mut interval = tokio::time::interval(Duration::from_secs(1800));
    loop {
        interval.tick().await;
        tor_state.tor_service.refresh().await;
    }
});
```

## Modelos

### Rule (back-end)

```rust
pub struct Rule {
    // ... existing fields ...
    pub is_tor: Option<bool>,  // Nuevo
}
```

### CacheRule

```rust
pub struct CacheRule {
    pub rule: Rule,
    // ... existing regex fields ...
    pub is_tor: Option<bool>,  // Nuevo — sin Regex, es booleano
}
```

### NewRequest

```rust
pub struct NewRequest {
    // ... existing fields ...
    pub is_tor: Option<bool>,  // Nuevo
}
```

### NewRule / UpdateRule

```rust
pub struct NewRule {
    // ... existing fields ...
    pub is_tor: Option<bool>,
}

pub struct UpdateRule {
    // ... existing fields ...
    pub is_tor: Option<bool>,
}
```

## from_request()

```rust
pub fn from_request(
    headers: &http::HeaderMap,
    geoip: Option<&GeoIpService>,
    tor_service: Option<&TorService>,  // Nuevo parámetro
) -> Self {
    // ... existing logic ...

    // Tor lookup
    let is_tor = tor_service.and_then(|ts| {
        let ip = headers.get("x-forwarded-for")
            .map(|s| s.to_str())
            .and_then(Result::ok)
            .unwrap_or("");
        if ip.is_empty() { None }
        else { ts.is_exit_node(ip) }
    });

    Self {
        // ... existing fields ...
        is_tor,
    }
}
```

## matches()

```rust
pub fn matches(&self, request: &NewRequest) -> bool {
    let check_match = |rule_regex: Option<&Regex>, request_value: Option<&String>| -> bool {
        // ... existing logic ...
    };

    let check_tor = |rule_tor: Option<bool>, request_tor: Option<bool>| -> bool {
        match rule_tor {
            None => true,              // No filter → neutral
            Some(true) => request_tor == Some(true),  // Must be Tor
            _ => true,                 // Safety
        }
    };

    check_match(self.ip_address.as_ref(), request.ip_address.as_ref())
        && check_match(self.protocol.as_ref(), request.protocol.as_ref())
        // ... all existing checks ...
        && check_match(self.x_request_id.as_ref(), request.x_request_id.as_ref())
        && check_tor(self.is_tor, request.is_tor)  // Nuevo
}
```

## Migración SQL

```sql
ALTER TABLE rules ADD COLUMN is_tor INTEGER NOT NULL DEFAULT 0;
```

## Frontend

### Rule interface

```typescript
export default interface Rule {
  // ... existing fields ...
  is_tor?: boolean;
}
```

### RuleDialog — Network tab

Añadir al final del tab Network:

```tsx
<Flex align="center" gap="small">
  <Text style={{ width: 120, flexShrink: 0 }}>{t("Tor Exit Node")}</Text>
  <Switch
    checked={Boolean(formValues.is_tor)}
    onChange={(checked) => updateField("is_tor", checked)}
    disabled={disabled}
  />
</Flex>
```

### DEFAULT_VALUES

```typescript
const DEFAULT_VALUES: Record<string, any> = {
  // ... existing ...
  is_tor: false,
};
```

### initializeFromItem

```typescript
is_tor: item.is_tor !== undefined ? Boolean(item.is_tor) : false,
```

### formatForApi

```typescript
is_tor: values.is_tor,
```

## Orden de locks

Sin cambios. TorService usa `RwLock` interno que se consulta antes de adquirir cualquier lock de reglas. La llamada a `is_exit_node()` ocurre durante `from_request()`, que se ejecuta antes de acceder a `app_state.rules`.
# Fail-Open: cuando shuul no responde

Si shuul se cae, se reinicia, o no responde, Traefik por defecto deniega
todas las peticiones (ForwardAuth falla → 403). Este documento describe
dos estrategias para que el tráfico **siga pasando** cuando shuul no está
disponible, a costa de no filtrar nada durante esa ventana.

## Tabla comparativa

| Aspecto | Sidecar Rust | Plugin Yaegi |
|---|---|---|
| Código | 50 LOC Rust | ~60 LOC Go (limitado por Yaegi) |
| Imagen | scratch (~5 MB) | No aplica (código fuente interpretado) |
| Latencia extra | ~1ms (salto HTTP localhost) | ~0ms (in-process) |
| Afecta a Traefik | No | Sí — reiniciar para cargar cambios |
| Mantenimiento | Mínimo (binario estático) | Moderado (entrypoint + checksum) |
| Dependencia externa | Ninguna | API de plugins de Traefik |
| Complejidad del deploy | Baja | Media-alta |
| Voto | **Recomendada** | Alternativa |

---

## Opción A: Sidecar (Rust) — recomendada

### Arquitectura

```
Traefik ──► sidecar:3001 ──proxy──► shuul:3000/api/v1/shuul
                │
                └── timeout/error ──► 200 OK (fail-open)
```

El sidecar es un binario Rust mínimo que:

1. Escucha en `0.0.0.0:3001`
2. Reenvía toda request a `http://shuul:3000/api/v1/shuul`
3. Si shuul responde → devuelve su respuesta (status, headers, body) tal cual
4. Si shuul no responde en 2s (timeout, connection refused, DNS failure) → devuelve `200 OK`

Traefik se configura apuntando al sidecar:

```yaml
forwardAuth:
  address: "http://sidecar:3001/api/v1/shuul"
```

### Código

**`sidecar/Cargo.toml`:**

```toml
[package]
name = "shuul-sidecar"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
reqwest = { version = "0.12", features = ["stream"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**`sidecar/src/main.rs`:**

```rust
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn};

const UPSTREAM: &str = "http://shuul:3000/api/v1/shuul";
const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let client = Client::builder()
        .timeout(TIMEOUT)
        .build()
        .expect("reqwest client");

    let app = Router::new().route("/{*path}", any(move |h| handler(client.clone(), h)));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("bind");

    info!("shuul-sidecar listening on 0.0.0.0:3001 → {}", UPSTREAM);

    axum::serve(listener, app).await.expect("serve");
}

async fn handler(client: Client, headers: HeaderMap) -> Response {
    let method = reqwest::Method::from_bytes(
        headers
            .get(":method")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("GET")
            .as_bytes(),
    )
    .unwrap_or(reqwest::Method::GET);

    let mut req_builder = client.request(method, UPSTREAM);

    for (key, val) in headers.iter() {
        if !key.as_str().starts_with(':') {
            req_builder = req_builder.header(key.as_str(), val);
        }
    }

    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = resp.headers().clone();
            let body = Body::from_stream(resp.bytes_stream());

            info!(status = status.as_u16(), "forwarded to upstream");

            let mut response = Response::new(body);
            *response.status_mut() = status;
            *response.headers_mut() = resp_headers;
            response
        }
        Err(_) => {
            warn!("upstream failed, fail-opening");
            (StatusCode::OK, "Ok").into_response()
        }
    }
}
```

### Dockerfile

**`sidecar/Dockerfile`:**

```dockerfile
FROM docker.io/library/rust:alpine AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch
COPY src ./src
RUN cargo build --release && strip target/release/sidecar

FROM scratch
COPY --from=builder /build/target/release/sidecar /
EXPOSE 3001
CMD ["/sidecar"]
```

### docker-compose

```yaml
services:
  shuul:
    build: .
    restart: always

  sidecar:
    build:
      context: ./sidecar
    ports: ["3001:3001"]
    restart: always
```

### Pros y contras

| Pros | Contras |
|---|---|
| Binario estático, sin dependencias | Latencia extra (~1ms por salto HTTP) |
| Nunca reiniciar Traefik | Un contenedor más (aunque ínfimo) |
| Independiente de la API de plugins de Traefik | |
| Fácil de debuggear (logs con tracing) | |
| Misma tecnología que shuul (Rust) | |

---

## Opción B: Plugin Yaegi en Traefik (alternativa)

### Arquitectura

```
Traefik ──► [plugin shuul-forwardauth] ──HTTP──► shuul:3000/api/v1/shuul
                 │
                 └── timeout/error ──► 200 OK (fail-open)
```

El plugin es código Go interpretado por Yaegi que vive dentro de Traefik.
Se distribuye como código fuente en un volumen compartido.

### Cómo se distribuye

1. Los plugins (`shuul-reporter` + `shuul-forwardauth`) van dentro de la imagen de shuul
2. Un entrypoint copia los plugins a un volumen nombrado compartido con Traefik
3. Si los plugins cambian (checksum del `.traefik.yml`), el entrypoint los actualiza
4. Al actualizar plugins, hay que reiniciar Traefik

### Entrypoint

```bash
#!/bin/sh
SRC=/app/traefik-plugins-src
DST=/app/traefik-plugins

for plugin in shuul-reporter shuul-forwardauth; do
    dst_manifest="$DST/$plugin/.traefik.yml"
    src_manifest="$SRC/$plugin/.traefik.yml"

    if [ ! -f "$dst_manifest" ] || ! cmp -s "$src_manifest" "$dst_manifest"; then
        echo "Instalando/actualizando plugin: $plugin"
        rm -rf "$DST/$plugin"
        cp -r "$SRC/$plugin" "$DST/$plugin"
    fi
done

exec ./backend
```

### docker-compose

```yaml
services:
  traefik:
    image: traefik:v3
    volumes:
      - shuul-plugins:/plugins
    command:
      - "--experimental.localPlugins.shuul-reporter.moduleName=github.com/atareao/traefik-shuul-reporter"
      - "--experimental.localPlugins.shuul-forwardauth.moduleName=github.com/atareao/traefik-shuul-forwardauth"

  shuul:
    build: .
    volumes:
      - shuul-plugins:/app/traefik-plugins
    restart: always

volumes:
  shuul-plugins:
```

### Pros y contras

| Pros | Contras |
|---|---|
| Sin latencia extra (in-process) | Reiniciar Traefik si cambian los plugins |
| Todo en Traefik, un proceso menos | API de plugins puede cambiar entre versiones de Traefik |
| | Entrypoint + checksum añade complejidad |
| | Yaegi tiene limitaciones vs Go nativo |
| | Mayor superficie de fallo (volumen, copias, permisos) |

---

## Decisión

**Cuando se implemente, la opción A (sidecar Rust)** es la recomendada
por su simplicidad, bajo mantenimiento, y cero impacto en Traefik.

La opción B (plugin Yaegi) se tendría en cuenta solo si en el futuro:

- El sidecar supusiera un problema de recursos (no debería, es ~5MB y 0.1% CPU)
- Se necesitara eliminar cualquier latencia extra (casos límite de alto throughput)
- Traefik ofreciera una API de plugins más madura y estable

Ambas opciones son compatibles con el `traefik-shuul-reporter` existente,
que puede seguir funcionando como bind mount o como parte del volumen
compartido sin cambios.
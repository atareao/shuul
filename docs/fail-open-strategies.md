# Fail-Open: cuando shuul no responde

Si shuul se cae, se reinicia, o no responde, Traefik por defecto deniega
todas las peticiones (ForwardAuth falla → 403). Este documento describe
dos estrategias para que el tráfico **siga pasando** cuando shuul no está
disponible, a costa de no filtrar nada durante esa ventana.

## Estado actual (implementado): reader de journald in-process

> **Nota.** La solución finalmente implementada para alimentar el pipeline Jail no es
> ninguno de los dos mecanismos descritos en este documento, sino un **reader de
> journald in-process** dentro del propio backend (`backend/src/journald_reader.rs`),
> que corre como background task. No hay sidecar separado ni plugin Yaegi: vive dentro
> de shuul.

El reader obtiene los access logs de Traefik desde **`systemd-journal-gatewayd`** por
HTTP en streaming:

```
GET {gateway_url}/entries?follow&CONTAINER_NAME=traefik
Accept: application/json
```

La respuesta es NDJSON (un objeto JSON por línea). El reader encaja con la filosofía de
la "opción A": Rust, sin Yaegi, sin latencia extra para Traefik y cero impacto en
Traefik, pero **sin proceso adicional**.

### Configuración

| Variable | Default | Descripción |
|---|---|---|
| `SHUUL_JOURNALD_URL` | `http://localhost:19531` | URL de `systemd-journal-gatewayd` |
| `SHUUL_JOURNALD_CONTAINER` | `traefik` | Nombre del contenedor cuyos logs se consumen |

Detalles de despliegue en `SELF-HOSTING.md`, sección 2.

### Fail-open del reader

- Si `gatewayd` no está disponible, registra `journald gateway not available` y
  **reintenta cada 30 s**. El proceso no se cae y el **pipeline WAF sigue intacto**;
  solo el Jail deja de recibir observaciones hasta que reconecte.
- **Entradas descartables**: se ignoran las que no son del contenedor configurado, las
  que no traen `MESSAGE`, las cuyo `MESSAGE` no es JSON válido (p. ej. líneas de log
  normales de Traefik) y las que tienen `OriginStatus == 0` (la request nunca llegó al
  backend: la respuesta vino del routing o de un middleware).
- **No se pierden entradas al reconectar**: guarda el último `__CURSOR` y reconecta con
  `Range: entries=<cursor>:1`.

### Compatibilidad con el plugin (legacy)

El plugin Go `traefik-shuul-reporter` y el endpoint `POST /api/v1/report` **siguen
existiendo** por compatibilidad. Ambos caminos (el reader de journald y el endpoint
HTTP) llaman a la misma función `run_jail_pipeline`.

---

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

## Opción A: Sidecar (Rust) — recomendada (propuesta de diseño)

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

## Opción B: Plugin Yaegi en Traefik (alternativa — legacy)

### Arquitectura

```
Traefik ──► [plugin shuul-forwardauth] ──HTTP──► shuul:3000/api/v1/shuul
                 │
                 └── timeout/error ──► 200 OK (fail-open)
```

El plugin es código Go interpretado por Yaegi que vive dentro de Traefik.
Se distribuye como código fuente en un volumen compartido.

### Cómo se distribuye (legacy)

> **Legacy.** Mecanismo vigente en su día para distribuir los plugins; sigue soportado
> pero ya **no recomendado**. El entrypoint y el `docker-compose` de abajo se conservan
> como documentación histórica.

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

Lo finalmente implementado es el **reader de journald in-process**
(`backend/src/journald_reader.rs`), equivalente en espíritu a la opción A
(Rust, sin Yaegi, sin latencia extra y cero impacto en Traefik) pero **sin
proceso adicional**: vive dentro del propio backend como background task. No
se implementó un sidecar separado ni un plugin Yaegi.

El plugin Yaegi (opción B) queda como **legacy**: la distribución de plugins
descrita arriba sigue soportada pero ya no es el mecanismo recomendado, y el
plugin Go `traefik-shuul-reporter` y el endpoint `POST /api/v1/report` siguen
existiendo por compatibilidad. Ambos caminos —el reader de journald y el
endpoint HTTP— convergen en la misma función `run_jail_pipeline`.

La configuración del reader está en `SELF-HOSTING.md`, sección 2.
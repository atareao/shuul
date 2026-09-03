# ═══════════════════════════════════════════════════════════════
# Stage 1: Backend (Rust)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/rust:alpine3.23 AS backend-builder

RUN apk add --no-cache --update \
    build-base \
    musl-dev \
    pkgconfig

WORKDIR /build

# Cache dependencies (avoid recompiling every time)
RUN cargo init --bin --name backend . && \
    echo "pub fn dummy() {}" > src/lib.rs

COPY backend/Cargo.toml backend/Cargo.lock ./
RUN cargo build --release && \
    rm -rf src

COPY backend/src ./src
RUN touch src/main.rs src/lib.rs && \
    cargo build --release && \
    strip target/release/backend

# ═══════════════════════════════════════════════════════════════
# Stage 2: Frontend (Node)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/node:23-alpine AS frontend-builder

RUN npm install -g pnpm@latest

WORKDIR /build
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
RUN pnpm install --ignore-scripts && pnpm rebuild esbuild

COPY frontend/ ./
ENV VITE_BASE_URL=""
RUN CI=true pnpm build

# ═══════════════════════════════════════════════════════════════
# Stage 3: Runtime
# ═══════════════════════════════════════════════════════════════
FROM alpine:3.23

ENV RUST_LOG=info \
    USER=app \
    UID=1000

RUN apk add --update --no-cache \
    curl \
    sqlite \
    ca-certificates && \
    rm -rf /var/cache/apk && \
    rm -rf /var/lib/app/lists && \
    mkdir -p /app/db /app/data

COPY --from=backend-builder /build/target/release/backend /app
COPY --from=frontend-builder /build/dist /app/static
COPY backend/migrations /app/migrations/

# Create the user
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/${USER}" \
    --shell "/sbin/nologin" \
    --uid "${UID}" \
    "${USER}" && \
    chown -R app:app /app

WORKDIR /app
USER app
EXPOSE 3000

CMD ["./backend"]

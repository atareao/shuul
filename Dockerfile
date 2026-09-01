# ═══════════════════════════════════════════════════════════════
# Stage 1: Backend (Rust)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/rust:alpine3.23 AS backend-builder

RUN apk add --no-cache --update \
    build-base \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

WORKDIR /build

# Cache dependencies (avoid recompiling every time)
RUN cargo init --bin --name backend . && \
    echo "pub fn dummy() {}" > src/lib.rs

COPY backend/Cargo.toml backend/Cargo.lock ./
RUN cargo build --release && \
    rm -rf src

ENV OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_STATIC=1

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

RUN apk add --no-cache \
    ca-certificates \
    && adduser -D -h /app -u 1000 app

WORKDIR /app
COPY --from=backend-builder /build/target/release/backend .
COPY --from=frontend-builder /build/dist ./static
COPY backend/migrations ./migrations/

RUN chown -R app:app /app

USER app
EXPOSE 3000
ENV RUST_LOG=info

CMD ["./backend"]
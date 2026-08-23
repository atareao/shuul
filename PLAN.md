# Cleanup: Eliminar JwtValidator y código OIDC no usado

> **Para agentic workers:** Implementar task por task, verificando `cargo check` después de cada una.

**Goal:** Eliminar `JwtValidator`, `jwt_validator` del `AppState`, y la background task de OIDC lazy init, ya que el middleware ahora valida JWTs con HS256 + `app_state.secret`.

**Architecture:** El middleware ya no usa `JwtValidator`. El `oidc_metadata` sigue siendo necesario para SSO redirect/callback en `auth.rs`, pero `jwt_validator` y su inicialización son código muerto.

**Tech Stack:** Rust, Axum, jsonwebtoken

---

### Task 1: Eliminar `JwtValidator` de `models/oidc.rs`

**Files:**
- Modify: `backend/src/models/oidc.rs`

- [ ] **Step 1: Eliminar `JwtValidator` struct y su impl**

Eliminar todo el bloque `pub struct JwtValidator` (líneas 22-26) y su `impl JwtValidator` (líneas 38-104), incluyendo `new()`, `fetch_jwks()` y `validate()`.

- [ ] **Step 2: Verificar compilación**

Run: `cargo check`
Expected: PASS (puede quedar warning por `oidc_metadata` no usado en middleware, pero no error)

---

### Task 2: Eliminar `jwt_validator` de `AppState`

**Files:**
- Modify: `backend/src/models/mod.rs`

- [ ] **Step 1: Eliminar `jwt_validator` del struct `AppState`**

Eliminar la línea:
```rust
pub jwt_validator: tokio::sync::RwLock<Option<JwtValidator>>,
```

- [ ] **Step 2: Verificar compilación**

Run: `cargo check`
Expected: PASS

---

### Task 3: Eliminar background task de OIDC lazy init en `main.rs`

**Files:**
- Modify: `backend/src/main.rs`

- [ ] **Step 1: Eliminar la background task que fetchea JWKS**

Eliminar el bloque `tokio::spawn(async move { ... })` que reintenta cada 30s (alrededor de línea 190-220).

- [ ] **Step 2: Eliminar `jwt_validator: RwLock::new(None)` de la construcción de `AppState`**

- [ ] **Step 3: Verificar compilación**

Run: `cargo check`
Expected: PASS

---

### Task 4: Simplificar `oidc_metadata` si procede

**Files:**
- Modify: `backend/src/models/mod.rs`

- [ ] **Step 1: Evaluar si `oidc_metadata` puede volver a `Option` en vez de `RwLock<Option>`**

El middleware ya no lo lee, solo `auth.rs` lo usa bajo `.read().await`. Si sigue siendo `RwLock` no hay problema funcional, pero se puede simplificar.

- [ ] **Step 2: Verificar compilación**

Run: `cargo check`
Expected: PASS
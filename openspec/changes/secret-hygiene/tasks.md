# Tasks: secret-hygiene

## 1. RED
- [ ] 1.1 `backend/tests/`: guarda que falla si alguna invocación de log del backend interpola la
      variable del secreto (recorriendo `src/main.rs`).
- [ ] 1.2 `backend/tests/`: guarda que falla si el `RUST_LOG` por defecto del `compose.yml`
      contiene `debug`.
- [ ] 1.3 `cargo test`: las dos guardas nuevas **fallan** y los tests existentes siguen en verde.

## 2. GREEN
- [ ] 2.1 `backend/src/main.rs`: fuera el `debug!("Secret: {}", secret)`.
- [ ] 2.2 `backend/src/main.rs`: aviso si el secreto mide menos de 32 caracteres.
- [ ] 2.3 `compose.yml`: `RUST_LOG` por defecto a `backend=info,maxminddb=warn`.
- [ ] 2.4 `cargo test` en verde.

## 3. REFACTOR
- [ ] 3.1 `cargo clippy -- -D warnings` sin avisos.
- [ ] 3.2 `cargo fmt --check` limpio.
- [ ] 3.3 `SELF-HOSTING.md`: la rotación del secreto con su consecuencia (invalida los JWT vivos) y
      qué hacer si un secreto acabó en los logs.

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Rotar el `SECRET` a `openssl rand -hex 32` en el `compose.yaml` del despliegue y recrear
      el contenedor.
- [ ] 4.2 Confirmar en el arranque que no aparece ninguna línea `Secret: `.
- [ ] 4.3 Confirmar que el `RUST_LOG` del despliegue ya no es `debug`.
- [ ] 4.4 Volver a entrar al panel: la rotación cierra las sesiones vivas, así que toca
      reautenticarse por OIDC.

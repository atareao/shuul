# Tasks: secret-hygiene

## 1. RED
- [x] 1.1 `backend/tests/`: guarda que falla si alguna invocación de log del backend interpola la
      variable del secreto (recorriendo `src/main.rs`).
      **Hecho:** `backend/tests/secret_hygiene.rs` recorre `backend/src` y marca toda invocación
      de log que interpole un identificador con la palabra `secret` (valor, truncamiento y
      longitud), con controles positivo y negativo. `LOG_MACROS` incluye también los volcados
      directos `dbg!`, `print!` y `eprint!` —clippy no los vigila: `clippy::dbg_macro` es del
      grupo `restriction`, que este crate no activa—, y hay controles de conteo único para
      `println!` y `eprintln!`.
- [x] 1.2 `backend/tests/`: guarda que falla si el `RUST_LOG` por defecto del `compose.yml`
      contiene `debug`.
      **Hecho:** segunda guarda en `secret_hygiene.rs`, sobre la línea `RUST_LOG:` de `compose.yml`.
- [x] 1.3 `cargo test`: las dos guardas nuevas **fallan** y los tests existentes siguen en verde.
      **Hecho:** `cargo test --no-fail-fast` con 3 rojos (`ninguna_invocacion_de_log_interpola_el_secreto`,
      `la_depuracion_no_esta_activa_por_defecto_en_el_compose`, `tests::un_secreto_corto_dispara_el_aviso`)
      y 360 heredados en verde.
- [x] 1.4 `backend/src/main.rs`: test unitario de la función que decide el aviso
      (`#[cfg(test)] mod tests`), cubriendo los dos escenarios del requisito «Un secreto débil se
      denuncia al arrancar»: un secreto de 10 caracteres dispara el aviso, uno de 64 no.
      **Hecho:** `#[cfg(test)] mod tests` en `main.rs` con los dos escenarios, más la frontera
      del mínimo (31 avisa, 32 no) y un `assert_eq!(MIN_SECRET_LEN, 32)` que ata el literal del
      aviso a la constante para que no se separen a escondidas.

## 2. GREEN
- [x] 2.1 `backend/src/main.rs`: fuera el `debug!("Secret: {}", secret)`.
      **Hecho:** línea eliminada; quedan intactos la lectura de `SECRET`, el punto de llamada del
      aviso y los demás usos de `secret`.
- [x] 2.2 `backend/src/main.rs`: aviso si el secreto mide menos de 32 caracteres.
      **Hecho:** cubierto por el cuerpo de `weak_secret_warning` junto con 2.5; verificado por
      `tests::un_secreto_corto_dispara_el_aviso`.
- [x] 2.3 `compose.yml`: `RUST_LOG` por defecto a `backend=info,maxminddb=warn`.
      **Hecho:** la línea pasa de `backend=debug,maxminddb=warn` a `backend=info,maxminddb=warn`.
- [x] 2.4 `cargo test` en verde.
      **Hecho:** `cargo test --no-fail-fast` → 363 en verde, 0 rojos (exit 0).
- [x] 2.5 `backend/src/main.rs`: implementar el cuerpo de `weak_secret_warning` (hoy provisional:
      devuelve siempre `None`), de modo que el test del secreto corto pase y el del largo siga verde.
      **Hecho:** `const fn` que devuelve el literal (sin interpolar nada, mínimo `32` nombrado en el
      texto y arreglo con `openssl rand -hex 32`) si el secreto mide menos de `MIN_SECRET_LEN`, y
      `None` si no.

## 3. REFACTOR
- [x] 3.1 `cargo clippy -- -D warnings` sin avisos.
      **Hecho:** exit 0, sin avisos nuevos.
- [x] 3.2 `cargo fmt --check` limpio.
      **Hecho:** exit 0, sin diferencias.
- [x] 3.3 `SELF-HOSTING.md`: la rotación del secreto con su consecuencia (invalida los JWT vivos) y
      qué hacer si un secreto acabó en los logs.
      **Hecho:** §13 amplía el punto del `SECRET` con las dos consecuencias: la rotación cierra las
      sesiones abiertas (reautenticación por OIDC) y, si el secreto acabó en un log, rotarlo de
      inmediato, auditar el acceso a esos logs y dar por sospechosas las sesiones previas; se
      aclara que una versión anterior con `RUST_LOG=backend=debug` sí lo escribió.

## 4. Verificación manual en producción (la hace el usuario)
- [ ] 4.1 Rotar el `SECRET` a `openssl rand -hex 32` en el `compose.yaml` del despliegue y recrear
      el contenedor.
- [ ] 4.2 Confirmar en el arranque que no aparece ninguna línea `Secret: `.
- [ ] 4.3 Confirmar que el `RUST_LOG` del despliegue ya no es `debug`.
- [ ] 4.4 Volver a entrar al panel: la rotación cierra las sesiones vivas, así que toca
      reautenticarse por OIDC.

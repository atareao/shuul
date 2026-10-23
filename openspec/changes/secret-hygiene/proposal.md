# Proposal: secret-hygiene

## Why

**El secreto con el que se firman los JWT se escribe en el log.** En `backend/src/main.rs:88`:

```rust
let secret = var("SECRET").expect("SECRET environment variable is mandatory");
debug!("Secret: {}", secret);
```

Es la **única** línea del backend que registra una credencial. Un barrido de los macros de log
(`debug!`, `info!`, `warn!`, `error!`, `trace!`) y de las salidas directas (`println!`, `eprintln!`)
por todo `backend/src` buscando `secret`, `password`, `token`, `credential` y `api_key` devuelve esa
línea y ninguna más. `OIDC_CLIENT_SECRET` se usa en `http/auth.rs` para el intercambio de tokens y no se
registra.

**Y el repositorio lo entrega con el log en `debug`.** En `compose.yml:9`:

```yaml
RUST_LOG: backend=debug,maxminddb=warn
```

Eso contradice la propia documentación del proyecto: el checklist de producción
(`SELF-HOSTING.md:1047`) exige «`RUST_LOG` is set to `info` or `warn` (**not `debug`**) in
production» y el `.env` de ejemplo (`SELF-HOSTING.md:978`) dice `RUST_LOG=info`. Quien despliegue
copiando el `compose.yml` del repositorio —que es exactamente lo que hace el `compose.yaml` de la
instancia de referencia— imprime su secreto de firma en cada arranque.

**El efecto es real, no teórico.** En la instancia de referencia el stack vivo corre con
`RUST_LOG=backend=debug,maxminddb=warn`, copiado del `compose.yml`. El objetivo `backend` de ese
filtro cubre `main.rs` —como demuestran las líneas `backend::http::report` y `backend::http::shuul`
que sí salen—, así que el `debug!` del secreto entra en el filtro y se emite en cada arranque. La
ventana retenida del journal de ese host ya rotó más allá del último arranque: no aparece ninguna
de sus líneas de arranque (`Port:`, `Maxmin DB Path` —así, sin la `d`, tal y como está escrita en
`main.rs:86`—, `Created databae pool` y `Migrations path` están todas a cero) y la primera entrada
es de diecisiete segundos después. La copia de hoy **no es recuperable, por suerte y no por
diseño**: el journal de un host con este volumen de tráfico rota rápido.

## What Changes

1. **El secreto no se registra nunca.** Fuera el `debug!("Secret: {}", secret)`. En su lugar, nada:
   un secreto no se registra a ningún nivel, ni completo, ni truncado, ni su longitud.
2. **La configuración por defecto deja de activar el log de depuración.** El `compose.yml` pasa a
   `RUST_LOG: backend=info,maxminddb=warn`, alineado con su propio checklist y con su `.env` de
   ejemplo. Subirlo temporalmente a `backend=debug` para diagnosticar sigue siendo posible: el
   §2.15 ya documenta cómo hacerlo y cómo volver atrás.
3. **Un secreto débil se denuncia al arrancar.** Hoy la aplicación aborta si `SECRET` falta, pero si
   está y es corta arranca en silencio. Pasa a emitir un aviso que nombra el requisito —32
   caracteres, el mínimo que ya exigen `README.md:147` y `SELF-HOSTING.md:1042`— y el arreglo. Es un
   aviso, no un aborto: si shuul no arranca, el ForwardAuth de Traefik no responde y se caen todos
   los servicios protegidos, así que una variable mal puesta no puede tumbar la infraestructura.
4. **La rotación se documenta con su consecuencia.** `SELF-HOSTING.md` §13 dice «rótalo
   periódicamente» pero no avisa de que al rotar se invalidan los JWT vivos: quien rote se
   encontrará con todas las sesiones del panel cerradas y tendrá que volver a entrar por OIDC. La
   guía gana además una nota de incidente —qué hacer cuando un secreto ha acabado en los logs:
   rotarlo, revisar quién tuvo acceso al log y dar por sospechosas las sesiones emitidas antes de
   la rotación—, que es la situación exacta de la instancia de referencia.

## Non-goals

- **No se rota ningún secreto desde el código.** El `SECRET` de cada despliegue lo rota su
  operador: es configuración, no aplicación.
- **No se enmascara el `DATABASE_URL`** que se registra en `main.rs:81`. En este proyecto es una
  ruta de SQLite, sin credenciales. Queda anotado por si algún día se admite otro motor.
- No se toca el mecanismo de firma, ni la validación de los JWT, ni el flujo OIDC.

## Impact

- `backend/src/main.rs` — fuera el registro del secreto; aviso si es débil
- `compose.yml` — `RUST_LOG` por defecto a `backend=info`
- `backend/tests/` — dos guardas de regresión: el secreto no vuelve a registrarse y el `compose.yml`
  no vuelve a activar la depuración por defecto
- `SELF-HOSTING.md` — la rotación y su consecuencia
- Sin migraciones, sin cambios en la base de datos, sin cambios en la interfaz

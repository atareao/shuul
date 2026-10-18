# Git Flow

Este proyecto sigue **Git Flow** con versionado semántico y changelog automáticos.

> **Nota:** la rama de integración se llama **`develop`** (no `development`).

## Ramas

| Rama | Propósito | Base |
|---|---|---|
| `main` | Producción. Cada merge aquí dispara la release automática. | — |
| `develop` | Integración de features en curso. | `main` |
| `feature/*` | Nuevas funcionalidades. | `develop` |
| `hotfix/*` | Correcciones urgentes a producción. | `main` |

## Flujo diario

### Features

```bash
git checkout develop && git pull origin develop
git checkout -b feature/mi-feature
# ... cambios ...
git commit -m "✨ feat(scope): descripción"
git push -u origin feature/mi-feature
gh pr create --base develop --head feature/mi-feature
```

Tras el merge, la rama se puede borrar:

```bash
git push origin --delete feature/mi-feature
```

### Hotfixes

```bash
git checkout main && git pull origin main
git checkout -b hotfix/arreglo-critico
git commit -m "🚑️ hotfix(scope): descripción"
git push -u origin hotfix/arreglo-critico
gh pr create --base main --head hotfix/arreglo-critico
```

Después del merge a `main`, sincronizar `develop`:

```bash
git checkout develop && git pull origin develop
git merge main && git push origin develop
```

### Releases

```bash
# 1. PR de develop -> main, titulado "release: vX.Y.Z"
gh pr create --base develop --head main --title "release: vX.Y.Z"

# 2. Al mergear, el workflow release-prepare.yml:
#    a) Compara .vampus.yml con el último tag
#    b) Bumpea la versión si hace falta (vampus upgrade)
#    c) Regenera CHANGELOG.md (git-cliff)
#    d) Commitea "chore: release vX.Y.Z" y crea el tag vX.Y.Z
#    e) Sincroniza develop con main

# 3. El tag v* dispara release.yml:
#    a) Construye y publica la imagen en GitHub Container Registry (ghcr.io)
#    b) Crea la GitHub Release con las notas del changelog
```

`release-prepare.yml` es **idempotente**: si la versión ya se bumpeó a mano en el PR, sólo regenera el changelog, commitea y taguea; si los ficheros de versión siguen en el último tag, bumpea primero.

En local, el equivalente es `just upgrade` (vampus + `cargo update` + tag + `just build push`).

## Conventional Commits con gitmoji

| Mensaje | Bump |
|---|---|
| `✨ feat: ...` | minor (`0.Y.0`) |
| `🐛 fix: ...` | patch (`0.0.Z`) |
| `♻️ refactor: ...` | patch (`0.0.Z`) |
| `📝 docs: ...` | patch (`0.0.Z`) |
| `💥 feat!: ...` o `BREAKING CHANGE` | major (`X.0.0`) |

### Gitmoji de referencia

| Tipo | Emoji |
|---|---|
| feat | ✨ |
| fix | 🐛 |
| hotfix | 🚑️ |
| docs | 📝 |
| refactor | ♻️ |
| perf | ⚡ |
| style | 💄 |
| test | ✅ |
| chore | 🔧 |
| ci | 👷 |
| revert | ⏪️ |
| deps | ⬆️ |
| breaking | 💥 |

## Versionado con vampus

`vampus` es la fuente de verdad de la versión (`.vampus.yml: current_version`) y
propaga el valor a los ficheros gestionados vía `replaces`:

- `backend/Cargo.toml`
- `frontend/.env`
- `frontend/.env.development`

> ⚠️ **Los patrones de `replaces` se comparan contra la versión *actual*.** Si un
> fichero gestionado no está sincronizado con `.vampus.yml`, `vampus upgrade`
> **aborta sin escribir nada** (comportamiento seguro, pero el bump no ocurre).
> Si eso pasa, sincroniza el fichero con el valor de `current_version` y vuelve a
> ejecutar el bump.

## CI Workflows

- **`ci.yml`** — en PRs a `main` y `develop`: `cargo fmt --check`, `clippy`,
  `cargo test` (backend) y `lint`, `build`, `test` (frontend).
- **`release-prepare.yml`** — en push a `main`: bump de versión, changelog, tag y
  sincronización de `develop`.
- **`release.yml`** — en tags `v*`: construye y publica la imagen de contenedor y
  crea la GitHub Release.

### Sobre `cargo test` en CI

Los tests del backend necesitan una base de datos **MaxMind GeoLite2** en una de
las rutas que prueban los helpers (`backend/geo/`, `geo/` o `/tmp/test.mmdb`).
Ese fichero está en `.gitignore` y no se puede redistribuir.

El CI lo descarga con la clave de licencia de MaxMind cuando el secreto está
disponible; si no lo está, **el paso de tests se salta y emite un warning** (los
checks de formato y clippy sí se ejecutan siempre).

## Secretos de GitHub necesarios

| Secreto | Propósito | ¿Obligatorio? |
|---|---|---|
| `GH_PAT` | Personal Access Token con scope `contents: write`, para que `release-prepare.yml` pueda pushear el commit de release, el tag y sincronizar `develop` | Sí para releases automáticas |
| `MAXMIND_LICENSE_KEY` | Descargar la base GeoLite2 en CI y poder ejecutar `cargo test` | Recomendado |
| `GITHUB_TOKEN` *(automático)* | Publicar en GitHub Container Registry (`ghcr.io/atareao/shuul`). Lo inyecta GitHub: **no hay que configurarlo** | — |

Se configuran en **Settings → Secrets and variables → Actions**.

> **Nota sobre GHCR:** la imagen se publica en `ghcr.io/atareao/shuul` con el
> `GITHUB_TOKEN`, así que **no requiere ningún secreto**. Pero los paquetes de
> GHCR son **privados por defecto**: si el host que despliega no está autenticado,
> hay que hacer público el paquete o hacer `podman login ghcr.io` una vez en él
> (token con scope `read:packages`). Ver `SELF-HOSTING.md` §6.2.

## Resumen visual

```
main        ──hotfix──●──────────────●──────────────────────●
                       \              /                      /
develop ──────────────●──●──●──────●──●──●──●───────────────●
                        \ /          \   /
feature     ──────────●  feature ────●
                      feature/foo     feature/bar

● = merge a develop (PR)
● = merge a main (release automática)
```

## Historia de releases (reparación de tags)

Hasta `v0.11.0` los tags se creaban a mano. Después se mergearon PRs de release
(`v0.12.0`, `v0.13.0`) pero **los tags nunca se crearon** (no había automatización
ni changelog), así que el changelog los englobaba dentro de `0.14.0`.

El **2026-10-18** se crearon retroactivamente los tags que faltaban:

| Tag | Commit | Fecha del release |
|---|---|---|
| `v0.12.0` | `d3b49cc` | 2026-09-21 |
| `v0.13.0` | `77c40c8` | 2026-09-22 |

> ⚠️ **Commit deliberadamente sin tag:** `97c3a7f` está titulado
> `release: v0.12.0`, pero es **posterior** a `v0.13.0` (verificado por ancestría).
> Es un título duplicado por error; etiquetarlo como `v0.12.0` habría roto la
> monotonía de la secuencia y generado rangos invertidos en git-cliff. Sus
> cambios se listan en la sección `0.14.0` del changelog.

A partir de `v0.14.0` los tags los crea `release-prepare.yml` automáticamente.

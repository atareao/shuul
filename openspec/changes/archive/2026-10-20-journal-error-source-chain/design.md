# Design

## Context

Ver `proposal.md` — Why. `JournalError` (backend/src/journald_reader.rs:69) es
el tipo de error de todo el módulo `journald_reader`: lo devuelven
`parse_journal_entry`, `parse_traefik_access_log`, `extract_report_payload`,
`tcp_endpoint` y `JournaldReader::run`. Las cuatro variantes guardan un `String`
con el contexto ya formateado y nada más.

Dos hechos verificados que condicionan el diseño:

1. **thiserror 1.0.61 soporta `#[source]` sobre un campo `Option`.** El
   generador de `thiserror-impl` (`expand.rs`) detecta que el tipo del campo es
   `Option` y genera `.as_ref()?` + `unoptional_type(...)`; `source()` devuelve
   `None` cuando el campo es `None`. El proyecto usa
   `thiserror = "1.0.61"` (backend/Cargo.toml:22). No hace falta subir la
   dependencia.
2. **`JournalError` tiene que seguir siendo `Send + Sync + 'static`.** El reader
   corre dentro de un `tokio::spawn` (backend/src/main.rs:379) y el error cruza
   el `.await`. El `String` de hoy es `Send + Sync`; un
   `Box<dyn std::error::Error + Send + Sync>` lo mantiene. `'static` se preserva
   porque la caja no presta nada.

## Goals / Non-Goals

**Goals:**

- Que `std::error::Error::source()` devuelva la causa real en los 9 puntos donde
  existe una.
- Que la causa se pueda inspeccionar **por tipo** (`downcast_ref`, `kind()`,
  `line()`/`column()`, `is_incomplete_message()`), no sólo leer formateada.
- Que los fallos sin causa sigan siendo representables sin fuente sintáctica.
- Que `Display` y los mensajes de log **no cambien** con la opción recomendada.
- Cero cambios de API pública y cero cambios en la variante devuelta por cada
  sitio.

**Non-Goals:**

- No cambiar la enumeración de variantes ni añadir variantes nuevas.
- No introducir un reporte que imprima la cadena (sería un cambio aparte).
- No tocar `AppError` ni el resto de tipos de error del proyecto.
- No migrar el error a `anyhow`/`miette`: el reader es `no_std`-agnóstico hoy y
  su error se `match`ea por variante en tests y en el bucle de reconexión.

## Decisions

### D1. `#[source] Option<Box<dyn Error + Send + Sync>>`

```rust
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("Socket error: {0}")]
    Socket(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),

    #[error("Journal entry parse error: {0}")]
    Parse(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),

    #[error("Traefik access log parse error: {0}")]
    TraefikParse(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),

    #[error("Connection error: {0}")]
    Connection(String, #[source] Option<Box<dyn std::error::Error + Send + Sync>>),
}
```

El `String` se conserva como primer campo: sigue siendo el contexto que puebla
`Display` y el que afirman los tests actuales. La fuente va aparte. El `Box` es
necesario porque los tipos de origen son heterogéneos (`io::Error`,
`hyper::Error`, `http::Error`, `InvalidUri`, `serde_json::Error`) y el tipo del
error no puede ser genérico: aparece en `Result` de firmas públicas y se
`match`ea por variante.

Alternativa descartada: `#[from]` con una variante por cada tipo de origen.
Multiplicaría las variantes (cuatro tipos de causa × cuatro contextos), rompería
todos los `match` existentes y perdería la flexibilidad de adjuntar la causa a
la variante que corresponde semánticamente.

### D2. Constructores de ayuda por variante

Para que los 23 sitios sin causa no tengan que escribir `JournalError::Connection(ctx, None)`
y los 9 con causa `JournalError::Connection(ctx, Some(Box::new(e)))`:

```rust
impl JournalError {
    pub fn socket(ctx: impl Into<String>) -> Self;
    pub fn socket_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn parse(ctx: impl Into<String>) -> Self;
    pub fn parse_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn traefik_parse(ctx: impl Into<String>) -> Self;
    pub fn traefik_parse_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn connection(ctx: impl Into<String>) -> Self;
    pub fn connection_from(ctx: impl Into<String>, e: impl std::error::Error + Send + Sync + 'static) -> Self;
}
```

Los constructores públicos también mantienen legible el diff: cada sitio cambia
`JournalError::Connection(format!(…))` por `JournalError::connection(format!(…))`
(sin causa) o `JournalError::connection_from(format!(…), e)` (con causa). El
`e` de origen se sigue formateando dentro de `ctx`, así que `Display` no cambia.

### D3. El `Display` incrusta el error **y además** lo lleva como `#[source]`

**Esta es la decisión abierta, y la elige el dueño del repo.** Hoy el contexto
incrusta el texto del error: `format!("invalid JSON: {e}")`. Al añadir
`#[source]` hay que decidir qué pasa con `Display`:

- **Opción 1 (recomendada): el contexto se queda como está y el error se añade
  *además* como `#[source]`.**
  - Cero cambios en los mensajes. Los `debug!`/`warn!` que hoy los imprimen
    (`:483` `"journald reader: skipping unsuitable journal line: {e}"`, `:638`
    `"jail pipeline error"`, `:643` `"MESSAGE is not a Traefik access log"`) no
    pierden ni un carácter, y los tests que hoy afirman sobre el texto siguen
    pasando sin tocarlos.
  - El operador sigue leyendo el log como hasta ahora — que es justo la
    herramienta que `SELF-HOSTING.md` §2.11 usa para distinguir «socket ausente»
    de «gateway no disponible».
  - Contrapartida: un reporte que recorra e imprima la cadena mostraría el texto
    dos veces (contexto + causa). Hoy no existe ese reporte; el coste del
    duplicado se paga sólo si algún día se activa.
  - Radio de acción mínimo.

- **Opción 2: el contexto deja de incrustar el error y los sitios de log pasan a
  imprimir la cadena.**
  - Renderizado más limpio (`invalid JSON` + fuente anidada una sola vez).
  - Obliga a tocar los tres sitios de log (`:483`, `:638`, `:643`) y a añadir un
    `fn chain(&JournalError)` o equivalente, y a reescribir todos los tests que
    afirman sobre el mensaje.
  - Más limpio, más superficie.

**Recomendación: Opción 1.** El objetivo es recuperar la causa por tipo, no
rediseñar el formato del mensaje; la Opción 1 lo consigue con el menor radio de
acción y sin arriesgar la herramienta de diagnóstico que el operador ya usa. La
decisión final es del dueño del repo.

### D4. Los 23 sitios sin causa usan los constructores sin fuente

447 `"gateway returned HTTP {status}"`, 566 timeout, 696/701/706/714
esquema/host ausentes, 767 `"empty journal entry"`, 774 `"journal entry is not a
JSON object"`, 787 `"no usable fields in journal entry"`, los tests de
865/871/877/883 y los `matches!`/`Err(...)` de 942/1301/1442/2486/2502 no tienen
causa que perder. Pasan a `JournalError::<variante>(ctx)` con
`#[source] = None`, para que quede explícito que la ausencia de fuente es
intencionada y no un olvido.

## Risks / Trade-offs

- **`Box<dyn Error>` no es `Clone`** → no se clona el error en ningún sitio hoy.
  Si hiciera falta, se clonaría el `String`. Sin impacto.
- **El `Box` añade una indirección** → sólo en el camino de error, que ya asigna
  un `String`. Despreciable.
- **Un reporte que imprima la cadena duplica el texto con la Opción 1** →
  aceptado; no existe tal reporte hoy y la decisión es reversible (Opción 2).
- **Downcast por tipo exige conocer el tipo** → es exactamente el objetivo:
  `source().and_then(|e| e.downcast_ref::<io::Error>()).map(|e| e.kind())`. El
  test de cada escenario lo usa.
- **Los tests que construyen las variantes a mano (865/871/877/883) dejan de
  compilar** → se migran a los constructores en la misma tarea de contrato; es
  el RED inicial esperado.

## Migration Plan

1. Desplegar la imagen con el cambio.
2. Recrear el contenedor.
3. Comprobar en los logs que los mensajes `journald reader: …` no cambian y que
   el diagnóstico de conexión sigue distinguiéndose.
4. Rollback: volver a la imagen anterior. No hay cambios de datos ni de esquema.

## Open Questions

- **Opción 1 vs Opción 2 del `Display` (D3).** Recomendación: Opción 1. Decisión
  del dueño del repo antes de implementar.

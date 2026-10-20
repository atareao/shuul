# journald-reader Specification — Delta

## ADDED Requirements

### Requirement: JournalError preserves the source chain

`JournalError` MUST carry the underlying error, when there is one, in a field
marked `#[source]`, so that callers can walk the chain through
`std::error::Error::source()` and inspect the cause by type. Failures that are
context only MUST remain representable without a source, and `JournalError`
MUST stay `Send + Sync + 'static`.

Las cuatro variantes (`Socket`, `Parse`, `TraefikParse`, `Connection`) SHALL
llevar un segundo campo `#[source] Option<Box<dyn std::error::Error + Send + Sync>>`.
`source()` SHALL devolver la causa cuando el campo no es `None` y `None` cuando
lo es. SHALL existir una pareja de constructores de ayuda por variante —una sin
fuente y otra con fuente— para que los sitios de contexto puro no ganen ruido.

La firma pública no cambia: cada sitio SHALL seguir devolviendo la misma
variante que devolvía antes. Con la Opción 1 de `design.md` (§D3), el texto del
primer campo NO SHALL cambiar, de modo que los `Display`, los `debug!`/`warn!`
que los imprimen y los tests que afirman sobre ese texto se mantienen.

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

#### Scenario: Parse failure exposes the serde_json error

- **GIVEN** una línea NDJSON que no es JSON válido
- **WHEN** `parse_journal_entry()` falla
- **THEN** devuelve `Err(JournalError::Parse)`
- **AND** `err.source()` es el `serde_json::Error` de la causa
- **AND** se puede hacer `downcast_ref::<serde_json::Error>()` sobre `source()`
  y leer su `line()` y `column()`
- **AND** el `Display` sigue nombrando el contexto (`invalid journal JSON: …`)

#### Scenario: Traefik access log failure exposes the serde_json error

- **GIVEN** un `MESSAGE` de Traefik cuyo texto no es JSON válido
- **WHEN** `parse_traefik_access_log()` falla
- **THEN** devuelve `Err(JournalError::TraefikParse)`
- **AND** `err.source()` es el `serde_json::Error` de la causa
- **AND** se puede leer su `line()` y `column()`
- **AND** el `Display` sigue nombrando el contexto (`invalid JSON: …`)

#### Scenario: Missing Unix socket exposes an io::Error with kind NotFound

- **GIVEN** `JournaldTransport::UnixSocket` apuntando a un path que no existe
- **WHEN** el reader intenta conectar
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `std::io::Error` de la causa
- **AND** `err.source().downcast_ref::<std::io::Error>().kind() == NotFound`

#### Scenario: Closed TCP port exposes an io::Error with kind ConnectionRefused

- **GIVEN** `JournaldTransport::Tcp` apuntando a un puerto en el que nadie escucha
- **WHEN** el reader intenta conectar
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `std::io::Error` de la causa
- **AND** `err.source().downcast_ref::<std::io::Error>().kind() == ConnectionRefused`

#### Scenario: Invalid gateway URL exposes the InvalidUri

- **GIVEN** un `base_url` que no parsea como URI
- **WHEN** se construye el endpoint TCP
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es el `http::uri::InvalidUri` de la causa
- **AND** el `Display` sigue nombrando el valor rechazado y la pista

#### Scenario: A failure without a cause keeps source None

- **GIVEN** un `base_url` al que le falta el esquema (contexto puro, sin causa)
- **WHEN** se construye el endpoint TCP
- **THEN** devuelve `Err(JournalError::Connection)`
- **AND** `err.source()` es `None`
- **AND** el `Display` es exactamente el de hoy

#### Scenario: The error stays usable across the async boundary

- **GIVEN** el reader corre dentro de un `tokio::spawn`
- **WHEN** un `JournalError` cruza el `.await` de la tarea de fondo
- **THEN** `JournalError` sigue siendo `Send + Sync + 'static`
- **AND** la aserción estática correspondiente compila

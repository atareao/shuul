//! Tests de integración (fase RED) del change `waf-transformations`, parte B2b.
//!
//! Specs:
//! - `openspec/changes/waf-transformations/specs/rules/spec.md`
//!   → «La especificación de transformaciones se valida en la escritura»
//!   (escenarios «Una transformación desconocida se rechaza al crear», «Una
//!   transformación desconocida se rechaza al importar sin abortar el resto»,
//!   «Una transformación desconocida se rechaza al editar», «Una especificación
//!   válida se almacena tal cual», «El valor por defecto es la pipeline vacía»,
//!   «Un nombre con otra capitalización se rechaza» y «El marcador none limpia
//!   la pipeline»).
//! - `openspec/changes/waf-transformations/specs/rules/spec.md`
//!   → «Una regla con una especificación irresoluble no entra en la caché»
//!   (escenarios «Una fila con un token irresoluble queda fuera de la caché» y
//!   «Las demás reglas siguen cargándose»).
//!
//! Cubre las tareas 1.14, 1.15 y 1.16 del change.
//!
//! ## Segunda ronda de RED
//!
//! Los cinco tests de la sección «Segunda ronda de RED» cubren escenarios que
//! quedaron sin test al contrastar el código con los deltas. Cuatro de ellos
//! —edición rechazada, especificación válida almacenada, valor por defecto y
//! capitalización— entran ahora como **guardia de regresión** de un
//! comportamiento que la ronda anterior ya implementó **antes** de que existiera
//! su test: es una desviación del orden estricto test-first y se declara como
//! tal, no se disimula. El quinto —el marcador `none`— sí falla en esta ronda:
//! la canonización en la escritura todavía no está implementada.
//!
//! El harness es el del repositorio, copiado de `tests/jail_weight_coherence_test.rs`:
//! pool `SQLite` en memoria con el esquema real (las migraciones de `migrations/`
//! aplicadas con `sqlx::migrate::Migrator`), `AppState` construido a mano y los
//! handlers reales conducidos con `tower::ServiceExt::oneshot`.
//!
//! ## Estado esperado en RED
//!
//! `TransformationPipeline::parse` es **inerte**: hoy ignora la especificación y
//! devuelve siempre una pipeline vacía, así que **nunca** falla. Por eso:
//!
//! - 1.14 falla porque la creación responde `201`, no `400`;
//! - 1.15 falla porque `rejected` viene vacío e `imported` vale `2`;
//! - 1.16 falla porque la fila irresoluble **sí** entra en la caché y no se emite
//!   ningún `error!`.
//!
//! Los tres fallos son **por aserción**, nunca por compilación.
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::rule_router;
use backend::models::{
    AppState, BanManager, CacheRule, GeoIpService, RateLimiter, Settings, StatsCollector,
    TorService,
};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Pool `SQLite` en memoria con el esquema de producción: todas las migraciones de
/// `backend/migrations/` aplicadas, como al arrancar el servidor.
async fn create_migrated_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("url sqlite válida")
        .create_if_missing(true);
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria");

    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("cargar las migraciones");
    migrator.run(&pool).await.expect("aplicar las migraciones");

    pool
}

/// Construye el `AppState` mínimo que necesitan los handlers de reglas.
fn build_app_state(pool: SqlitePool) -> Arc<AppState> {
    Arc::new(AppState {
        pool,
        secret: "test-secret".to_string(),
        geoip: GeoIpService::new(
            maxminddb::Reader::open_readfile("geo/GeoLite2-City.mmdb").expect("geo mmdb"),
        ),
        tor_service: TorService::new(),
        rules: Mutex::new(Vec::new()),
        stats: StatsCollector::new(),
        static_dir: ".".to_string(),
        ban_manager: Mutex::new(BanManager::new(3600, false, vec![1], 86400, 30)),
        rate_limiter: Mutex::new(HashMap::<i32, RateLimiter>::new()),
        pending_bans: Mutex::new(Vec::new()),
        settings: Mutex::new(Settings::default()),
        oidc_metadata: tokio::sync::RwLock::new(None),
        jwt_validator: tokio::sync::RwLock::new(None),
        oidc_states: tokio::sync::Mutex::new(HashMap::new()),
        oidc_client_id: None,
        oidc_redirect_url: None,
    })
}

/// Conduce el handler real de reglas y devuelve `(status, cuerpo JSON)`.
async fn call_rules_api(
    state: &Arc<AppState>,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let router = rule_router().with_state(Arc::clone(state));
    let payload = body.map_or_else(String::new, |value| value.to_string());
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("petición"),
        )
        .await
        .expect("respuesta del handler");

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("leer el cuerpo");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// Inserta una regla **activa** por SQL crudo, incluida su especificación de
/// transformaciones, y devuelve su id.
///
/// El `insert_rule` de `tests/jail_weight_coherence_test.rs` no escribe la
/// columna `transformations`; para 1.16 hace falta una fila cuya especificación
/// irresoluble **nunca** pasó por la API —es el legado que la caché tiene que
/// saber descartar—, así que este helper la escribe explícitamente.
async fn insert_rule_with_transformations(
    pool: &SqlitePool,
    name: &str,
    pipeline: &str,
    transformations: &str,
) -> i32 {
    sqlx::query_scalar(
        "INSERT INTO rules
            (name, description, weight, mode, pipeline, transformations, allow, path, active,
             created_at, updated_at)
         VALUES (?, '', 0, 'enforce', ?, ?, 1, '^/admin', 1, datetime('now'), datetime('now'))
         RETURNING id",
    )
    .bind(name)
    .bind(pipeline)
    .bind(transformations)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("insertar la regla {name}: {error}"))
}

/// Cuenta las filas almacenadas con ese nombre.
async fn stored_rows(pool: &SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("contar las filas de {name}: {error}"))
}

// ---------------------------------------------------------------------------
// 1.14 — La creación rechaza una transformación desconocida
// ---------------------------------------------------------------------------

/// Escenario: «Una transformación desconocida se rechaza al crear».
///
/// GIVEN un `POST /api/v1/rules` con `transformations: "urlDecodeUni,noExiste"`
/// WHEN el handler procesa la petición
/// THEN SHALL responder `400 Bad Request`
/// AND el mensaje SHALL nombrar `noExiste`
/// AND NO SHALL almacenarse ninguna fila
///
/// Se conduce el handler real de `rule_router()` (`POST /`): la validación tiene
/// que ocurrir **antes** de escribir, así que el rechazo se comprueba también
/// contando las filas almacenadas, no solo por el código de estado. El cuerpo del
/// error es el `AppError::InvalidInput` serializado a JSON
/// (`{"status":400,"message":"…noExiste…"}`), de modo que el token viaja al
/// cliente y el mensaje es diagnosticable.
#[tokio::test]
async fn la_creacion_rechaza_una_transformacion_desconocida() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "transformacion-no-admitida";

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": "urlDecodeUni,noExiste",
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "una especificación de transformaciones con un token desconocido debe rechazarse con 400; \
         respuesta: {response}"
    );
    assert!(
        response.to_string().contains("noExiste"),
        "el mensaje del rechazo SHALL nombrar el token que no se reconoció; respuesta: {response}"
    );
    assert_eq!(
        stored_rows(&pool, name).await,
        0,
        "el rechazo SHALL ocurrir antes de escribir: no puede quedar ninguna fila con una \
         especificación irresoluble"
    );
}

// ---------------------------------------------------------------------------
// 1.15 — La importación rechaza la entrada, no la importación entera
// ---------------------------------------------------------------------------

/// Escenario: «Una transformación desconocida se rechaza al importar sin abortar
/// el resto».
///
/// GIVEN un `POST /api/v1/rules/import` con dos reglas, una con un token
/// desconocido
/// WHEN el handler procesa la petición
/// THEN la regla inválida SHALL aparecer en `rejected` con el motivo
/// AND la otra SHALL importarse
///
/// La importación es **por entradas**, igual que con el pipeline: una entrada
/// inválida no puede abortar en silencio la importación entera ni colarse como si
/// nada. Por eso la respuesta es `200` —no un error— y dice cuántas se importaron
/// y cuáles se rechazaron, con el motivo que nombra el token.
#[tokio::test]
async fn la_importacion_rechaza_una_transformacion_desconocida_sin_abortar_el_resto() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({
            "rules": [
                {
                    "name": "import-transform-buena",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "waf",
                    "transformations": "urlDecodeUni,none,lowercase",
                    "allow": true,
                    "path": "^/admin",
                    "active": true
                },
                {
                    "name": "import-transform-mala",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "waf",
                    "transformations": "urlDecodeUni,noExiste",
                    "allow": true,
                    "path": "^/admin",
                    "active": true
                }
            ]
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "la importación no debe abortar por una entrada con un token desconocido; \
         respuesta: {response}"
    );
    assert_eq!(
        response["imported"].as_u64(),
        Some(1),
        "solo la entrada con una especificación válida cuenta como importada; \
         respuesta: {response}"
    );
    let rejected = response["rejected"].as_array().unwrap_or_else(|| {
        panic!("la respuesta debe identificar las entradas rechazadas: {response}")
    });
    assert_eq!(
        rejected.len(),
        1,
        "la respuesta debe identificar exactamente la entrada rechazada; respuesta: {response}"
    );
    assert_eq!(
        rejected[0]["name"].as_str(),
        Some("import-transform-mala"),
        "la entrada rechazada SHALL identificarse por su nombre; respuesta: {response}"
    );
    assert!(
        rejected[0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("noExiste")),
        "el motivo del rechazo SHALL nombrar el token que no se reconoció; respuesta: {response}"
    );
    assert_eq!(
        stored_rows(&pool, "import-transform-mala").await,
        0,
        "la entrada con un token desconocido no puede quedar almacenada"
    );
    assert_eq!(
        stored_rows(&pool, "import-transform-buena").await,
        1,
        "la entrada válida de la misma importación sí debe almacenarse"
    );
    assert_eq!(
        stored_transformations(&pool, "import-transform-buena").await,
        "lowercase",
        "la importación SHALL canonizar la especificación (el marcador `none` limpia la \
         pipeline) y SHALL escribirla en la columna `transformations`"
    );
}

// ---------------------------------------------------------------------------
// 1.16 — Una especificación irresoluble no entra en la caché
// ---------------------------------------------------------------------------

/// Un `Write` que acumula en un buffer compartido lo que el subscriber emite.
#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Instala en el **hilo actual** un subscriber a nivel ERROR que graba cada evento
/// y devuelve su `DefaultGuard` junto al buffer donde se acumula.
///
/// Es la misma técnica que `src/models/rule.rs` usa para el `debug!` del matcher:
/// un `tracing_subscriber::fmt()` con un `Writer` propio. `#[tokio::test]` corre
/// el futuro en el runtime *current-thread*, así que el guard —que es un valor
/// *thread-local*— sigue activo en cada `.await` del mismo hilo y captura lo que
/// `CacheRule::read_all_active` emita mientras se carga la caché. El guard se
/// devuelve para poder soltarlo al terminar la captura y no contaminar al resto
/// de tests.
fn capture_errors() -> (tracing::subscriber::DefaultGuard, Arc<Mutex<Vec<u8>>>) {
    let sink = Arc::new(Mutex::new(Vec::new()));
    let writer = CapturedLog(Arc::clone(&sink));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .with_ansi(false)
        .without_time()
        .with_writer(move || CapturedLog(Arc::clone(&writer.0)))
        .finish();

    let guard = tracing::subscriber::set_default(subscriber);
    (guard, sink)
}

/// El texto acumulado en el buffer de captura.
fn captured_text(sink: &Arc<Mutex<Vec<u8>>>) -> String {
    let bytes = sink
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    String::from_utf8_lossy(&bytes).to_string()
}

/// Escenario: «Una fila con un token irresoluble queda fuera de la caché» y «Las
/// demás reglas siguen cargándose».
///
/// GIVEN una base con una fila insertada por SQL crudo con
/// `transformations = "urlDecodeUni,noExiste"` y otra válida
/// WHEN se cargan las reglas activas con `CacheRule::read_all_active`
/// THEN la fila irresoluble NO SHALL estar en la caché
/// AND SHALL emitirse un `error!` que nombre la regla y `noExiste`
/// AND la fila válida SHALL estar en la caché
///
/// Las dos filas se insertan **por SQL crudo** (sin pasar por la API) a
/// propósito: es el legado que la caché tiene que saber descartar. Cargar la fila
/// irresoluble con las transformaciones que sí se reconocen —o con ninguna— la
/// haría decidir algo distinto de lo que su autor escribió, que es el fallo
/// silencioso que el requisito evita; por eso se exige el `error!` que la nombre.
/// `read_all_active` tampoco puede devolver `Err`: una fila ilegible se excluye,
/// no aborta la carga entera.
#[tokio::test]
async fn una_especificacion_irresoluble_no_entra_en_la_cache() {
    let pool = create_migrated_pool().await;
    let bad = "transformacion-irresoluble";
    let good = "transformacion-buena";

    insert_rule_with_transformations(&pool, bad, "waf", "urlDecodeUni,noExiste").await;
    insert_rule_with_transformations(&pool, good, "waf", "urlDecodeUni").await;

    let (guard, sink) = capture_errors();
    let loaded = CacheRule::read_all_active(&pool).await;
    drop(guard);

    let loaded = loaded.unwrap_or_else(|error| {
        panic!("una fila con una especificación irresoluble SHALL excluirse de la caché, no abortar la carga: {error}")
    });
    let names: Vec<&str> = loaded.iter().map(|rule| rule.rule.name.as_str()).collect();

    assert!(
        names.contains(&good),
        "la regla con una especificación válida SHALL seguir cargándose; cargadas: {names:?}"
    );
    assert!(
        !names.contains(&bad),
        "una regla cuya especificación no se puede leer NO SHALL entrar en la caché (cargarla con \
         las transformaciones que sí se reconocen, o con ninguna, la haría decidir algo distinto \
         de lo que su autor escribió); cargadas: {names:?}"
    );

    let logs = captured_text(&sink);
    assert!(
        logs.contains(bad),
        "SHALL emitirse un `error!` que nombre la regla irresoluble ({bad}); capturado: {logs:?}"
    );
    assert!(
        logs.contains("noExiste"),
        "el `error!` SHALL nombrar el token que no se pudo resolver (`noExiste`); \
         capturado: {logs:?}"
    );
}

// ---------------------------------------------------------------------------
// Segunda ronda de RED — escenarios del delta `rules` sin test
// ---------------------------------------------------------------------------

/// Lee la especificación de transformaciones almacenada de la regla con ese
/// nombre, **por SQL crudo**.
///
/// Se lee la columna, no la respuesta de la API: lo que se comprueba es lo que
/// quedó **almacenado**, que es lo que el requisito describe («SHALL
/// almacenarse»).
async fn stored_transformations(pool: &SqlitePool, name: &str) -> String {
    sqlx::query_scalar("SELECT transformations FROM rules WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("leer las transformaciones de {name}: {error}"))
}

/// Escenario: «Una transformación desconocida se rechaza al editar».
///
/// GIVEN un `PATCH /api/v1/rules` que introduce un token desconocido
/// WHEN el handler procesa la petición
/// THEN SHALL responder `400 Bad Request`
/// AND la regla SHALL conservar su especificación anterior
///
/// La regla se crea primero con una especificación válida y se lee la columna
/// **después** del rechazo: la validación tiene que ocurrir antes de escribir,
/// así que el `400` no basta como aserto —la fila podría haberse degradado—.
#[tokio::test]
async fn la_edicion_rechaza_una_transformacion_desconocida() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "edicion-transformacion-no-admitida";
    let previous = "urlDecodeUni,compressWhitespace";

    let (status, created) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": previous,
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "la regla de partida debe crearse con una especificación válida; respuesta: {created}"
    );
    let id = created["data"]["id"]
        .as_i64()
        .expect("la regla creada debe traer su id");

    let (status, response) = call_rules_api(
        &state,
        Method::PATCH,
        "/",
        Some(json!({
            "id": id,
            "name": name,
            "description": "esta descripción no debe guardarse",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": "urlDecodeUni,noExiste",
            "allow": true,
            "is_tor": false,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "la edición con un token desconocido debe rechazarse con 400; respuesta: {response}"
    );
    assert!(
        response.to_string().contains("noExiste"),
        "el mensaje del rechazo SHALL nombrar el token que no se reconoció; respuesta: {response}"
    );
    assert_eq!(
        stored_transformations(&pool, name).await,
        previous,
        "la regla SHALL conservar su especificación anterior: el rechazo tiene que ocurrir \
         antes de escribir"
    );
}

/// Escenario: «Una especificación válida se almacena tal cual».
///
/// GIVEN un `POST /api/v1/rules` con `transformations:
/// "urlDecodeUni,compressWhitespace"`
/// WHEN el handler procesa la petición
/// THEN SHALL responder `201 Created`
/// AND la regla leída SHALL declarar la misma especificación
#[tokio::test]
async fn una_especificacion_valida_se_almacena_tal_cual() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "especificacion-valida-almacenada";
    let spec = "urlDecodeUni,compressWhitespace";

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": spec,
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::CREATED,
        "una especificación válida debe crear la regla con 201; respuesta: {response}"
    );
    assert_eq!(
        stored_transformations(&pool, name).await,
        spec,
        "la regla SHALL declarar exactamente la especificación que se envió"
    );
}

/// Escenario: «El valor por defecto es la pipeline vacía».
///
/// GIVEN un `POST /api/v1/rules` sin el campo `transformations`
/// WHEN el handler procesa la petición
/// THEN la regla SHALL almacenarse con la cadena vacía
///
/// La cadena vacía es la pipeline vacía: una regla que no declara
/// transformaciones no cambia de comportamiento.
#[tokio::test]
async fn sin_el_campo_la_regla_se_almacena_con_la_cadena_vacia() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "sin-campo-transformaciones";

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::CREATED,
        "una petición sin el campo debe crear la regla con 201; respuesta: {response}"
    );
    assert_eq!(
        stored_transformations(&pool, name).await,
        "",
        "sin el campo, la regla SHALL almacenarse con la cadena vacía (la pipeline vacía)"
    );
}

/// Escenario: «Un nombre con otra capitalización se rechaza».
///
/// GIVEN un `POST /api/v1/rules` con `transformations: "urldecodeuni"`
/// WHEN el handler procesa la petición
/// THEN SHALL responder `400 Bad Request`
/// AND el mensaje SHALL listar los nombres válidos
///
/// Los nombres canónicos son sensibles a mayúsculas: `urldecodeuni` no es
/// `urlDecodeUni`. El aserto comprueba que el mensaje **lista los válidos**, no
/// solo que rechace.
#[tokio::test]
async fn un_nombre_con_otra_capitalizacion_se_rechaza_y_lista_los_validos() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "capitalizacion-no-admitida";

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": "urldecodeuni",
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "un nombre con otra capitalización debe rechazarse con 400; respuesta: {response}"
    );
    assert!(
        response.to_string().contains("urlDecodeUni"),
        "el mensaje SHALL listar los nombres válidos, entre ellos `urlDecodeUni`; \
         respuesta: {response}"
    );
    assert_eq!(
        stored_rows(&pool, name).await,
        0,
        "el rechazo SHALL ocurrir antes de escribir: no puede quedar ninguna fila"
    );
}

/// Escenario: «El marcador none limpia la pipeline».
///
/// GIVEN un `POST /api/v1/rules` con `transformations:
/// "urlDecodeUni,none,lowercase"`
/// WHEN el handler procesa la petición
/// THEN la regla SHALL almacenar una pipeline equivalente a `lowercase`
///
/// La especificación **canónica** resultante es `lowercase`: el `none` limpia lo
/// construido hasta ese punto. La canonización en la escritura todavía no está
/// implementada —se almacena la cadena tal cual se envió—, así que este es el
/// test que falla en esta ronda de RED.
#[tokio::test]
async fn el_marcador_none_limpia_la_pipeline() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());
    let name = "marcador-none";

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": name,
            "description": "",
            "weight": 100,
            "mode": "enforce",
            "pipeline": "waf",
            "transformations": "urlDecodeUni,none,lowercase",
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::CREATED,
        "el marcador `none` se acepta, así que la creación responde 201; respuesta: {response}"
    );
    assert_eq!(
        stored_transformations(&pool, name).await,
        "lowercase",
        "el `none` limpia la pipeline construida hasta ese punto: la especificación canónica \
         almacenada SHALL ser `lowercase`"
    );
}

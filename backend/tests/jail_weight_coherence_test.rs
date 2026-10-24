//! Tests de integración (fase RED) del change `jail-weight-coherence`.
//!
//! Specs:
//! - `openspec/changes/jail-weight-coherence/specs/rules/spec.md`
//!   → «El peso de una regla de Jail es siempre 0» y «La ordenación de la lista
//!   de reglas es estable».
//! - `openspec/changes/jail-weight-coherence/specs/templates/spec.md`
//!   → «Weight alignment with pipeline» (los templates de JAIL declaran peso 0).
//!
//! El harness es el del repositorio: pool `SQLite` en memoria con el esquema real
//! (las migraciones de `migrations/` aplicadas con `sqlx::migrate::Migrator`,
//! igual que `src/models/rate_limiter.rs`), `AppState` construido a mano y los
//! handlers reales conducidos con `tower::ServiceExt::oneshot`, como en
//! `tests/templates_listing_test.rs` y `tests/report_per_rule_ban_test.rs`.
//!
//! ## Qué simula (y qué no) cada test
//!
//! - 1.1 y 1.2 van contra el **handler real** de `rule_router()` (creación,
//!   lectura por id, edición e **importación**). `rule_router()` no lleva el
//!   middleware de JWT —`require_auth` se aplica al hacer `nest` en `main.rs`—,
//!   así que se puede conducir sin token y sigue siendo el contrato HTTP real.
//! - 1.3 aplica las migraciones sobre una base **vacía** (como al arrancar) y
//!   después inserta por SQL crudo filas «legacy» y vuelve a ejecutar el SQL de
//!   la migración de normalización (que es idempotente). En la fase RED el
//!   fichero de la migración aún no existe: el test falla precisamente por eso.
//! - 1.4 y 1.5 llaman a la función de listado real (`Rule::read_paged`) y crean
//!   un índice **entre** dos consultas. Es honesto decirlo: con el plan que
//!   `SQLite` elige hoy para el esquema migrado (recorrido de tabla + *temp
//!   B-tree* para el `ORDER BY`) los empates salen en orden de rowid, así que
//!   dos consultas idénticas devolverían el mismo orden *por casualidad* y la
//!   ausencia de desempate sería invisible. El índice nuevo es exactamente la
//!   perturbación que sufre el planificador cuando una migración futura añade un
//!   índice o cuando `ANALYZE` refresca las estadísticas: sin desempate
//!   explícito las dos respuestas se ordenan distinto (rowid frente a índice) y
//!   la paginación repite y pierde filas; con él, las dos coinciden.
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::rule_router;
use backend::models::{
    AppState, BanManager, GeoIpService, RateLimiter, ReadRuleParams, Rule, Settings,
    StatsCollector, TorService,
};
use backend::templates::all_rule_templates;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
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

/// Inserta una regla por SQL crudo (sin pasar por la API) y devuelve su id.
async fn insert_rule(pool: &SqlitePool, name: &str, pipeline: &str, weight: i32) -> i32 {
    sqlx::query_scalar(
        "INSERT INTO rules
            (name, description, weight, mode, pipeline, allow, path, active, created_at, updated_at)
         VALUES (?, '', ?, 'enforce', ?, 1, '^/admin', 1, datetime('now'), datetime('now'))
         RETURNING id",
    )
    .bind(name)
    .bind(weight)
    .bind(pipeline)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("insertar la regla {name}: {error}"))
}

/// El peso tal y como está **almacenado** en la base de datos.
async fn stored_weight(pool: &SqlitePool, name: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT weight FROM rules WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|error| panic!("leer el peso de {name}: {error}"))
}

/// El pipeline tal y como está **almacenado** en la base de datos.
async fn stored_pipeline(pool: &SqlitePool, name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT pipeline FROM rules WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|error| panic!("leer el pipeline de {name}: {error}"))
}

/// Filas modificadas por la última sentencia de escritura de la conexión
/// (`sqlite3_changes()`), que es lo que mide «la migración no cambió nada».
///
/// Es observable porque el pool del harness tiene una **única** conexión
/// (`max_connections(1)`): si tuviera más, `changes()` podría leerse en otra y
/// devolver un valor ajeno a la sentencia que se quiere medir.
async fn rows_changed(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT changes()")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("leer changes(): {error}"))
}

// ---------------------------------------------------------------------------
// 1.1 — La API no confía en el cliente
// ---------------------------------------------------------------------------

/// Escenario: «La API no confía en el cliente» (vía de creación).
///
/// GIVEN una petición de creación con pipeline `jail` y un peso distinto de 0
/// WHEN se crea la regla
/// THEN queda almacenada con peso 0
/// AND el peso que devuelve la API para esa regla es 0
///
/// Se cubren las **tres** vías de escritura del cliente que el proposal nombra:
/// creación (este test), importación (el siguiente) y edición (1.2). La
/// importación tiene su propio `INSERT ... ON CONFLICT` en `src/http/rule.rs` y
/// sería un camino nuevo para reintroducir el peso si la coerción solo viviese
/// en el handler de creación. Una regla del WAF se crea con el mismo peso para
/// comprobar que el WAF conserva el número: es el único pipeline donde decide.
#[tokio::test]
async fn la_api_no_confia_en_el_cliente_al_crear_una_regla_de_jail() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, created) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": "jail-create-weight-300",
            "description": "",
            "weight": 300,
            "mode": "enforce",
            "pipeline": "jail",
            "allow": true,
            "path": "^/admin",
            "active": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "la creación de la regla de Jail debe responder 201; respuesta: {created}"
    );
    assert_eq!(
        created["data"]["weight"].as_i64(),
        Some(0),
        "el peso que devuelve la API para una regla de Jail creada con peso 300 debe ser 0; \
         respuesta: {created}"
    );
    let created_id = created["data"]["id"]
        .as_i64()
        .unwrap_or_else(|| panic!("la respuesta debe traer el id de la regla creada: {created}"));

    let (status, read_back) =
        call_rules_api(&state, Method::GET, &format!("/?id={created_id}"), None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "leer por id la regla recién creada; respuesta: {read_back}"
    );
    assert_eq!(
        read_back["data"]["weight"].as_i64(),
        Some(0),
        "la API debe devolver peso 0 al releer una regla de Jail; respuesta: {read_back}"
    );
    assert_eq!(
        stored_weight(&pool, "jail-create-weight-300").await,
        Some(0),
        "la regla de Jail creada con peso 300 debe quedar almacenada con peso 0"
    );

    // --- control: una regla del WAF conserva el peso que trae la petición ---
    let (status, waf) = call_rules_api(
        &state,
        Method::POST,
        "/",
        Some(json!({
            "name": "waf-keeps-weight-300",
            "description": "",
            "weight": 300,
            "mode": "enforce",
            "pipeline": "waf",
            "allow": false,
            "country_code": "RU",
            "active": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "la creación de la regla del WAF debe responder 201; respuesta: {waf}"
    );
    assert_eq!(
        waf["data"]["weight"].as_i64(),
        Some(300),
        "el peso de una regla del WAF SHALL conservarse: es el único pipeline donde el número \
         decide (first match wins por weight ASC); respuesta: {waf}"
    );
    assert_eq!(
        stored_weight(&pool, "waf-keeps-weight-300").await,
        Some(300),
        "la regla del WAF debe quedar almacenada con el peso que traía la petición"
    );
}

/// Escenario: «La API no confía en el cliente» (vía de importación).
///
/// GIVEN una importación con pipeline `jail` y un peso distinto de 0
/// WHEN se importa la regla
/// THEN queda almacenada con peso 0
///
/// La importación es una vía de escritura con **código propio**
/// (`INSERT ... ON CONFLICT (name) DO UPDATE` en `src/http/rule.rs`): si la
/// coerción del peso solo viviese en el handler de creación, este sería el
/// camino por el que el peso volvería a la base de datos.
#[tokio::test]
async fn la_api_no_confia_en_el_cliente_al_importar_una_regla_de_jail() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, imported) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({
            "rules": [{
                "name": "jail-import-weight-300",
                "description": "",
                "weight": 300,
                "mode": "enforce",
                "pipeline": "jail",
                "allow": true,
                "path": "^/admin",
                "active": true
            }]
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la importación de una regla de Jail debe aceptarse; respuesta: {imported}"
    );
    assert_eq!(
        stored_weight(&pool, "jail-import-weight-300").await,
        Some(0),
        "la importación es otra vía de escritura del cliente (tiene su propio \
         `INSERT ... ON CONFLICT (name) DO UPDATE`): una regla de Jail importada con peso 300 \
         debe quedar almacenada con peso 0"
    );
}

// ---------------------------------------------------------------------------
// 1.2 — La edición tampoco reintroduce un peso
// ---------------------------------------------------------------------------

/// Escenario: «La edición tampoco reintroduce un peso».
///
/// GIVEN una regla de Jail existente con peso 0
/// WHEN se actualiza enviando un peso distinto de 0
/// THEN la regla sigue almacenada con peso 0
#[tokio::test]
async fn la_edicion_no_reintroduce_un_peso_en_una_regla_de_jail() {
    let pool = create_migrated_pool().await;
    let rule_id = insert_rule(&pool, "jail-edited-weight-300", "jail", 0).await;
    let state = build_app_state(pool.clone());

    let (status, updated) = call_rules_api(
        &state,
        Method::PATCH,
        "/",
        Some(json!({
            "id": rule_id,
            "name": "jail-edited-weight-300",
            "description": "",
            "weight": 300,
            "mode": "enforce",
            "pipeline": "jail",
            "allow": true,
            // `rules.is_tor` is `NOT NULL` and `Rule::update` binds the `Option`
            // as-is, so the payload of an update has to carry it (a `null` here
            // fails with a NOT NULL constraint, unrelated to this change).
            "is_tor": false,
            "ip_address": null,
            "protocol": null,
            "fqdn": null,
            "path": "^/admin",
            "query": null,
            "city_name": null,
            "country_name": null,
            "country_code": null,
            "user_agent": null,
            "method": null,
            "referer": null,
            "content_type": null,
            "accept_language": null,
            "x_request_id": null,
            "rate_limit_profile_id": null,
            "active": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la edición de la regla de Jail debe responder 200; respuesta: {updated}"
    );
    assert_eq!(
        updated["data"]["weight"].as_i64(),
        Some(0),
        "la API no debe devolver un peso distinto de 0 al editar una regla de Jail; \
         respuesta: {updated}"
    );
    assert_eq!(
        stored_weight(&pool, "jail-edited-weight-300").await,
        Some(0),
        "una regla de Jail editada enviando peso 300 debe seguir almacenada con peso 0"
    );
}

// ---------------------------------------------------------------------------
// 1.3 — La normalización alcanza a lo ya desplegado
// ---------------------------------------------------------------------------

/// Ruta y contenido del SQL de la migración que normaliza el peso de las reglas
/// de Jail.
///
/// Se lee **en tiempo de ejecución** desde `migrations/`, nunca con
/// `include_str!`: en la fase RED ese fichero todavía no existe y con
/// `include_str!` el fallo sería de compilación y arrastraría a todo el binario
/// de test en vez de a este caso, ensuciando la comprobación de que lo existente
/// sigue en verde.
fn read_normalization_migration_sql() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let suffix = "_normalize_jail_rule_weight.up.sql";

    let mut found: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("no se puede leer {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.ends_with(suffix))
        })
        .collect();
    found.sort();

    assert_eq!(
        found.len(),
        1,
        "se esperaba exactamente una migración `*{suffix}` en {} (encontradas: {found:?}); \
         su ausencia en la fase RED es el motivo por el que este test falla",
        dir.display()
    );

    fs::read_to_string(&found[0])
        .unwrap_or_else(|error| panic!("no se puede leer {}: {error}", found[0].display()))
}

/// Escenario: «La normalización alcanza a lo ya desplegado».
///
/// GIVEN una base de datos con reglas de Jail de pesos variados
/// WHEN se aplica la migración de normalización
/// THEN todas las reglas de Jail quedan con peso 0
/// AND las reglas del WAF conservan su peso
///
/// Honestidad sobre lo que simula: la migración ya se aplicó sobre una base
/// **vacía** al arrancar (`create_migrated_pool`), así que aquí se insertan
/// filas «legacy» —los pesos que el catálogo repartía a las plantillas de Jail:
/// 10, 50, 100, 9997, 9998 y 9999— y se vuelve a ejecutar el SQL de la
/// normalización, que es idempotente. Es el equivalente en test de una base ya
/// desplegada que recibe la migración nueva.
#[tokio::test]
async fn la_normalizacion_alcanza_a_lo_ya_desplegado() {
    let pool = create_migrated_pool().await;

    let jail_legacy = [
        ("legacy-jail-10", 10),
        ("legacy-jail-50", 50),
        ("legacy-jail-100", 100),
        ("legacy-jail-9997", 9997),
        ("legacy-jail-9998", 9998),
        ("legacy-jail-9999", 9999),
    ];
    let waf_legacy = [
        ("legacy-waf-10", 10),
        ("legacy-waf-50", 50),
        ("legacy-waf-100", 100),
        ("legacy-waf-200", 200),
        ("legacy-waf-300", 300),
    ];

    for (name, weight) in jail_legacy {
        insert_rule(&pool, name, "jail", weight).await;
    }
    for (name, weight) in waf_legacy {
        insert_rule(&pool, name, "waf", weight).await;
    }

    let sql = read_normalization_migration_sql();
    sqlx::raw_sql(&sql)
        .execute(&pool)
        .await
        .expect("ejecutar el SQL de la migración de normalización");

    assert_eq!(
        rows_changed(&pool).await,
        i64::try_from(jail_legacy.len()).expect("el recuento cabe en i64"),
        "la primera aplicación debe normalizar exactamente las {} reglas de Jail del escenario \
         (ni una del WAF de más): el filtro tiene que ceñirse al pipeline de Jail",
        jail_legacy.len()
    );

    // Idempotencia: el comentario de la migración promete que reaplicarla no
    // escribe nada. Aquí se aserta, en vez de dejarlo en una promesa.
    sqlx::raw_sql(&sql)
        .execute(&pool)
        .await
        .expect("reaplicar el SQL de la migración de normalización");

    assert_eq!(
        rows_changed(&pool).await,
        0,
        "la segunda aplicación de la migración SHALL NOT cambiar ninguna fila: el filtro \
         `weight <> 0` deja fuera las que ya están bien, así que reaplicarla es un no-op"
    );

    for (name, weight) in jail_legacy {
        assert_eq!(
            stored_weight(&pool, name).await,
            Some(0),
            "la migración debe normalizar a 0 la regla de Jail {name} (tenía peso {weight})"
        );
    }
    for (name, weight) in waf_legacy {
        assert_eq!(
            stored_weight(&pool, name).await,
            Some(i64::from(weight)),
            "la migración SHALL NOT tocar la regla del WAF {name}: su peso decide el orden de \
             evaluación (first match wins por weight ASC)"
        );
    }
}

// ---------------------------------------------------------------------------
// 1.4 y 1.5 — La ordenación de la lista de reglas es estable
// ---------------------------------------------------------------------------

/// Inserta `count` reglas del WAF con el mismo peso y devuelve sus nombres.
///
/// Los nombres van en orden **inverso** al de inserción a propósito: así el orden
/// del índice `(weight, name)` que crea [`perturb_query_plan`] es distinguible
/// del orden de rowid. Si ambos coincidiesen, el test no podría observar la
/// diferencia entre «hay desempate» y «el plan devuelve lo mismo por suerte».
///
/// El pipeline elegido es `waf` porque es donde el peso sigue siendo libre y
/// donde los empates son lo normal (el propio catálogo reparte varias plantillas
/// con el mismo peso); tras este change las reglas de Jail valen siempre 0 y un
/// empate suyo no diría nada del `ORDER BY`.
async fn insert_tied_rules(
    pool: &SqlitePool,
    prefix: &str,
    count: usize,
    weight: i32,
) -> Vec<String> {
    let mut names = Vec::with_capacity(count);
    for index in 1..=count {
        let name = format!("{prefix}-{:02}", count + 1 - index);
        insert_rule(pool, &name, "waf", weight).await;
        names.push(name);
    }
    names
}

/// Crea un índice que el planificador **puede** usar para satisfacer el
/// `ORDER BY weight`, lo que cambia el orden de los empates.
///
/// Es la perturbación que el mundo real provoca sin querer: cualquier migración
/// futura que añada un índice, o un `ANALYZE` que refresque las estadísticas,
/// cambia el plan de la lista de reglas. Sin un desempate explícito ese cambio
/// reordena las filas empatadas entre dos consultas iguales.
async fn perturb_query_plan(pool: &SqlitePool) {
    sqlx::query("CREATE INDEX idx_rules_weight_name ON rules (weight, name)")
        .execute(pool)
        .await
        .expect("crear el índice que perturba el plan");
}

/// Parámetros de listado: orden por `weight ASC` y una página concreta.
fn weight_page(page: u32, limit: u32) -> ReadRuleParams {
    ReadRuleParams {
        id: None,
        name: None,
        description: None,
        mode: None,
        pipeline: None,
        weight: None,
        allow: None,
        ip_address: None,
        protocol: None,
        fqdn: None,
        path: None,
        query: None,
        city_name: None,
        country_name: None,
        country_code: None,
        rate_limit_profile_name: None,
        active: None,
        page: Some(page),
        limit: Some(limit),
        sort_by: Some("weight".to_string()),
        asc: Some(true),
    }
}

/// Escenario: «Los empates no bailan entre consultas».
///
/// GIVEN varias reglas con el mismo peso
/// WHEN se listan ordenadas por peso dos veces consecutivas
/// THEN las dos respuestas presentan las reglas en el mismo orden
#[tokio::test]
async fn los_empates_no_bailan_entre_consultas() {
    const TIES: usize = 25;

    let pool = create_migrated_pool().await;
    insert_tied_rules(&pool, "tie", TIES, 100).await;

    let first = Rule::read_paged(&pool, &weight_page(1, 50))
        .await
        .expect("primera consulta de la lista");
    perturb_query_plan(&pool).await;
    let second = Rule::read_paged(&pool, &weight_page(1, 50))
        .await
        .expect("segunda consulta de la lista");

    assert_eq!(
        first.len(),
        TIES,
        "la primera consulta debe traer las {TIES} reglas empatadas; trajo {}",
        first.len()
    );
    assert_eq!(
        second.len(),
        TIES,
        "la segunda consulta debe traer las {TIES} reglas empatadas; trajo {}",
        second.len()
    );
    assert!(
        first.iter().all(|rule| rule.weight == 100),
        "todas las reglas del escenario comparten el peso 100"
    );

    let first_ids: Vec<i32> = first.iter().map(|rule| rule.id).collect();
    let second_ids: Vec<i32> = second.iter().map(|rule| rule.id).collect();
    assert_eq!(
        first_ids, second_ids,
        "dos consultas consecutivas con el mismo criterio (weight ASC) deben devolver el mismo \
         orden aunque haya pesos repetidos: el criterio SHALL desempatar de forma determinista"
    );
}

/// Escenario: «La paginación de empates no repite ni pierde filas».
///
/// GIVEN más reglas con el mismo peso que el tamaño de página
/// WHEN se recorren todas las páginas ordenando por ese peso
/// THEN la unión de las páginas contiene cada regla exactamente una vez
#[tokio::test]
async fn la_paginacion_de_empates_no_repite_ni_pierde_filas() {
    const TIES: usize = 25;
    const PAGE_SIZE: u32 = 10;

    let pool = create_migrated_pool().await;
    insert_tied_rules(&pool, "page-tie", TIES, 100).await;

    let mut seen: Vec<i32> = Vec::new();
    for page in 1..=3u32 {
        if page == 2 {
            // La segunda página es otra petición más tarde: el plan de la
            // consulta puede haber cambiado (índice nuevo de una migración,
            // estadísticas refrescadas...). Es justo el momento en el que un
            // orden sin desempate repetiría filas.
            perturb_query_plan(&pool).await;
        }
        let rows = Rule::read_paged(&pool, &weight_page(page, PAGE_SIZE))
            .await
            .unwrap_or_else(|error| panic!("listar la página {page}: {error}"));
        assert!(
            !rows.is_empty(),
            "la página {page} no puede venir vacía: hay {TIES} reglas empatadas y el tamaño de \
             página es {PAGE_SIZE}"
        );
        assert!(
            rows.iter().all(|rule| rule.weight == 100),
            "todas las reglas del escenario comparten el peso 100"
        );
        seen.extend(rows.iter().map(|rule| rule.id));
    }

    assert_eq!(
        seen.len(),
        TIES,
        "recorrer las páginas debe devolver cada una de las {TIES} reglas (la unión no puede \
         perder filas); se obtuvieron {} filas: {seen:?}",
        seen.len()
    );

    let unique: HashSet<i32> = seen.iter().copied().collect();
    assert_eq!(
        unique.len(),
        seen.len(),
        "la unión de las páginas no puede repetir filas empatadas; se obtuvieron {} filas y {} \
         distintas: {seen:?}",
        seen.len(),
        unique.len()
    );

    let all: HashSet<i32> = sqlx::query_scalar("SELECT id FROM rules")
        .fetch_all(&pool)
        .await
        .expect("leer los ids de todas las reglas")
        .into_iter()
        .collect();
    assert_eq!(
        unique, all,
        "la unión de las páginas debe contener exactamente las reglas almacenadas"
    );
}

// ---------------------------------------------------------------------------
// 1.6 — El catálogo no disfraza el peso de Jail
// ---------------------------------------------------------------------------

/// Escenario (delta de `templates`): «JAIL catch-all have lowest weights» y
/// «JAIL service protection have medium weights», reescritos a peso 0.
///
/// GIVEN el catálogo de plantillas de reglas
/// WHEN se examinan sus pipelines
/// THEN las 18 plantillas de Jail declaran peso 0 (el pipeline de Jail no ordena)
/// AND ninguna de las 24 del WAF tiene peso 0 (es donde el número decide)
#[test]
fn el_catalogo_no_disfraza_el_peso_de_jail() {
    let templates = all_rule_templates();

    let jail: Vec<_> = templates
        .iter()
        .filter(|template| template.pipeline == "jail")
        .collect();
    let waf: Vec<_> = templates
        .iter()
        .filter(|template| template.pipeline == "waf")
        .collect();

    assert_eq!(
        jail.len(),
        18,
        "el catálogo debe seguir teniendo 18 plantillas de Jail"
    );
    assert_eq!(
        waf.len(),
        24,
        "el catálogo debe seguir teniendo 24 plantillas del WAF"
    );

    for template in &jail {
        assert_eq!(
            template.weight, 0,
            "la plantilla de Jail «{}» debe declarar peso 0: el pipeline de Jail itera todas las \
             reglas que casan, sin break, así que cualquier otro peso es una decisión sin efecto \
             que la interfaz, además, oculta (declara {})",
            template.name, template.weight
        );
    }

    for template in &waf {
        assert_ne!(
            template.weight, 0,
            "la plantilla del WAF «{}» debe conservar su peso: es el pipeline donde el orden \
             decide (first match wins por weight ASC)",
            template.name
        );
    }
}

// ---------------------------------------------------------------------------
// 6.1 — Solo un pipeline admitido llega a la base de datos
// ---------------------------------------------------------------------------

/// Valores que **no** son un pipeline admitido: la misma palabra con otra
/// capitalización, con un espacio de más, una etiqueta que no existe y la
/// cadena vacía.
///
/// El resto del sistema compara el pipeline de forma **literal y sensible a
/// mayúsculas** —`shuul.rs` salta las reglas `jail`, `report.rs` salta las
/// `waf`—, así que ninguno de estos valores participa de la invariante de su
/// pipeline: una regla `JAIL` no es una regla de Jail (no se le aplicaría el
/// peso 0) y una `waf ` tampoco es del WAF.
const PIPELINES_NO_ADMITIDOS: [&str; 6] = ["JAIL", "Jail", "WAF", "jail ", "both", ""];

/// Cuenta las filas almacenadas con ese nombre.
async fn stored_rows(pool: &SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("contar las filas de {name}: {error}"))
}

/// Escenario: «La creación lo rechaza».
///
/// GIVEN una petición de creación con un pipeline que no es exactamente `waf`
/// ni `jail`
/// WHEN se crea la regla
/// THEN la API la rechaza
/// AND no queda ninguna fila con ese nombre
#[tokio::test]
async fn la_creacion_rechaza_un_pipeline_no_admitido() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    for (index, pipeline) in PIPELINES_NO_ADMITIDOS.iter().enumerate() {
        let name = format!("pipeline-no-admitido-{index}");
        let (status, response) = call_rules_api(
            &state,
            Method::POST,
            "/",
            Some(json!({
                "name": name,
                "description": "",
                "weight": 100,
                "mode": "enforce",
                "pipeline": pipeline,
                "allow": true,
                "path": "^/admin",
                "active": true
            })),
        )
        .await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "un pipeline no admitido ({pipeline:?}) debe rechazarse con 400; respuesta: {response}"
        );
        assert_eq!(
            stored_rows(&pool, &name).await,
            0,
            "el rechazo SHALL ocurrir antes de escribir: no puede quedar ninguna fila con el \
             pipeline {pipeline:?}"
        );
    }
}

/// Escenario: «La edición lo rechaza».
///
/// GIVEN una regla de Jail existente
/// WHEN se actualiza con un pipeline que no es exactamente `waf` ni `jail`
/// THEN la API la rechaza
/// AND la regla conserva sus valores
#[tokio::test]
async fn la_edicion_rechaza_un_pipeline_no_admitido() {
    let pool = create_migrated_pool().await;
    let rule_id = insert_rule(&pool, "jail-edit-sin-cambios", "jail", 0).await;
    let state = build_app_state(pool.clone());

    let (status, response) = call_rules_api(
        &state,
        Method::PATCH,
        "/",
        Some(json!({
            "id": rule_id,
            "name": "jail-edit-renombrada",
            "description": "esta descripción no debe guardarse",
            "weight": 300,
            "mode": "enforce",
            "pipeline": "JAIL",
            "allow": true,
            "is_tor": false,
            "ip_address": null,
            "protocol": null,
            "fqdn": null,
            "path": "^/admin",
            "query": null,
            "city_name": null,
            "country_name": null,
            "country_code": null,
            "user_agent": null,
            "method": null,
            "referer": null,
            "content_type": null,
            "accept_language": null,
            "x_request_id": null,
            "rate_limit_profile_id": null,
            "active": true
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "un pipeline no admitido debe rechazarse en la edición con 400; respuesta: {response}"
    );

    let stored = sqlx::query_as::<_, (String, String, String, i32)>(
        "SELECT name, description, pipeline, weight FROM rules WHERE id = ?",
    )
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .unwrap_or_else(|error| panic!("leer la regla {rule_id} tras el rechazo: {error}"));

    assert_eq!(
        stored,
        (
            "jail-edit-sin-cambios".to_string(),
            String::new(),
            "jail".to_string(),
            0
        ),
        "el rechazo SHALL dejar la regla tal y como estaba: nombre, descripción, pipeline y peso \
         no pueden cambiar"
    );
}

/// Escenario: «La importación no lo cuela».
///
/// GIVEN una importación con una regla de pipeline admitido y otra con un
/// pipeline no admitido
/// WHEN se importan
/// THEN la entrada rechazada no se almacena
/// AND la respuesta la identifica
///
/// La importación es **por entradas**: una entrada inválida no puede abortar en
/// silencio la importación entera ni colarse como si nada, así que la respuesta
/// dice cuántas se importaron y cuáles se rechazaron.
#[tokio::test]
async fn la_importacion_no_cuela_un_pipeline_no_admitido() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({
            "rules": [
                {
                    "name": "import-admitida-waf",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "waf",
                    "allow": true,
                    "country_code": "RU",
                    "active": true
                },
                {
                    "name": "import-rechazada-JAIL",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "JAIL",
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
        "la importación no debe abortar por una entrada inválida; respuesta: {response}"
    );
    assert_eq!(
        response["imported"].as_u64(),
        Some(1),
        "solo la entrada admitida cuenta como importada; respuesta: {response}"
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
        Some("import-rechazada-JAIL"),
        "la entrada rechazada SHALL identificarse por su nombre; respuesta: {response}"
    );
    assert_eq!(
        stored_rows(&pool, "import-rechazada-JAIL").await,
        0,
        "la entrada con un pipeline no admitido no puede quedar almacenada"
    );
    assert_eq!(
        stored_rows(&pool, "import-admitida-waf").await,
        1,
        "la entrada admitida de la misma importación sí debe almacenarse"
    );
}

// ---------------------------------------------------------------------------
// 6.1 bis — La migración alcanza al legado con otra capitalización
// ---------------------------------------------------------------------------

/// Escenario: «La migración alcanza al legado con otra capitalización».
///
/// GIVEN una base con reglas de Jail almacenadas con otra capitalización
/// WHEN se aplica la migración de normalización
/// THEN todas quedan con peso 0
/// AND el pipeline **almacenado** de esas filas queda canónico (`jail`)
/// AND las del WAF conservan su peso
///
/// El legado no tiene por qué traer `jail` en minúsculas: antes de que la API
/// validase el pipeline, cualquier cadena cabía en la columna. La comparación de
/// la migración tiene que ser insensible a mayúsculas **solo** para `jail`; las
/// filas del WAF no se tocan.
///
/// No basta con normalizar el peso: `shuul.rs` salta las reglas de Jail
/// comparando el literal exacto `jail`, así que una fila con `JAIL` o `Jail`
/// seguiría **decidiendo en el WAF** aunque su peso ya fuese 0. La migración tiene
/// que dejar canónica también la etiqueta.
#[tokio::test]
async fn la_migracion_alcanza_al_legado_con_otra_capitalizacion() {
    let pool = create_migrated_pool().await;

    let legacy_jail = [
        ("legacy-upper-jail", "JAIL", 200),
        ("legacy-mixed-jail", "Jail", 50),
    ];
    let legacy_waf = [
        ("legacy-upper-waf", "WAF", 200),
        ("legacy-lower-waf", "waf", 300),
    ];

    for (name, pipeline, weight) in legacy_jail {
        insert_rule(&pool, name, pipeline, weight).await;
    }
    for (name, pipeline, weight) in legacy_waf {
        insert_rule(&pool, name, pipeline, weight).await;
    }

    let sql = read_normalization_migration_sql();
    sqlx::raw_sql(&sql)
        .execute(&pool)
        .await
        .expect("ejecutar el SQL de la migración de normalización");

    for (name, pipeline, weight) in legacy_jail {
        assert_eq!(
            stored_weight(&pool, name).await,
            Some(0),
            "la migración debe alcanzar la regla {name}, almacenada con el pipeline {pipeline:?} \
             (peso {weight}): la comparación SHALL ser insensible a mayúsculas"
        );
        assert_eq!(
            stored_pipeline(&pool, name).await.as_deref(),
            Some("jail"),
            "la migración SHALL dejar canónica la etiqueta de {name} (era {pipeline:?}): \
             `shuul.rs` salta las reglas de Jail comparando el literal `jail`, así que una fila \
             con otra capitalización seguiría decidiendo en el WAF aunque su peso ya sea 0"
        );
    }
    for (name, pipeline, weight) in legacy_waf {
        assert_eq!(
            stored_weight(&pool, name).await,
            Some(i64::from(weight)),
            "la migración SHALL NOT tocar la regla {name}, almacenada con el pipeline \
             {pipeline:?}: su peso decide el orden de evaluación del WAF"
        );
    }
}

// ---------------------------------------------------------------------------
// 6.2 — La reimportación de un nombre existente tampoco reintroduce el peso
// ---------------------------------------------------------------------------

/// Escenario: «La reimportación pasa por `ON CONFLICT DO UPDATE`».
///
/// GIVEN una regla de Jail importada
/// WHEN se vuelve a importar el **mismo nombre** con un peso distinto de 0
/// THEN la rama `ON CONFLICT (name) DO UPDATE` corre
/// AND el peso sigue siendo 0
///
/// El test 1.1 solo ejercita el `INSERT` de la importación. La rama de
/// actualización es otra sentencia —`ON CONFLICT (name) DO UPDATE SET weight =
/// EXCLUDED.weight`— y por tanto otra oportunidad de reintroducir el peso: si la
/// coerción viviese solo en el valor del `INSERT`, el `EXCLUDED.weight` la
/// saltaría. Se comprueba además que la descripción sí se actualiza, para no dar
/// por hecho que la rama de actualización corrió.
#[tokio::test]
async fn la_importacion_normaliza_el_peso_al_reimportar_un_nombre_existente() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = |weight: i32, description: &str| {
        json!({
            "rules": [{
                "name": "import-upsert-jail",
                "description": description,
                "weight": weight,
                "mode": "enforce",
                "pipeline": "jail",
                "allow": true,
                "path": "^/admin",
                "active": true
            }]
        })
    };

    let (status, first) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(payload(777, "primera importación")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la primera importación (rama `INSERT`) debe aceptarse; respuesta: {first}"
    );
    assert_eq!(
        stored_weight(&pool, "import-upsert-jail").await,
        Some(0),
        "el `INSERT` de la importación debe almacenar la regla de Jail con peso 0"
    );

    let (status, second) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(payload(999, "reimportada")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la reimportación (rama `ON CONFLICT DO UPDATE`) debe aceptarse; respuesta: {second}"
    );

    let stored =
        sqlx::query_as::<_, (String, i32)>("SELECT description, weight FROM rules WHERE name = ?")
            .bind("import-upsert-jail")
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|error| panic!("leer la regla reimportada: {error}"));

    assert_eq!(
        stored.0, "reimportada",
        "la rama `ON CONFLICT (name) DO UPDATE` SHALL haber corrido: la descripción es la de la \
         segunda importación"
    );
    assert_eq!(
        stored.1, 0,
        "el peso SHALL seguir siendo 0 tras reimportar el mismo nombre con peso 999: la coerción \
         no puede quedarse solo en el valor del `INSERT`, porque el `DO UPDATE` escribe \
         `EXCLUDED.weight`"
    );
}

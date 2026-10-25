//! Tests de integración (fase RED) del change `waf-rule-conditions` (F2b-1).
//!
//! Specs: `openspec/changes/waf-rule-conditions/specs/rules/spec.md`
//! → «Las condiciones de una regla se persisten en `rule_conditions`» y
//! «El matcheo se alimenta de las condiciones persistidas».
//!
//! El harness es el del repositorio: pool `SQLite` en memoria con el esquema real
//! (las migraciones de `migrations/` aplicadas con `sqlx::migrate::Migrator`,
//! igual que `tests/jail_weight_coherence_test.rs`), `AppState` construido a mano
//! y los handlers reales de reglas conducidos con `tower::ServiceExt::oneshot`.
//!
//! En la fase RED la tabla `rule_conditions` todavía **no** existe: la migración
//! se crea en GREEN, así que cada consulta a la tabla falla con
//! «no such table: `rule_conditions`», que es el modo natural en que estos tests
//! se ponen en rojo. El test de backfill falla además porque la migración de
//! `rule_conditions` aún no está en `migrations/`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::rule_router;
use backend::models::condition::{Collection, Operator, project};
use backend::models::transformation::TransformationPipeline;
use backend::models::{
    AppState, BanManager, CacheRule, GeoIpService, NewRule, RateLimiter, Rule, Settings,
    StatsCollector, TorService, UpdateRule,
};
use backend::templates::{RuleTemplate, all_rule_templates};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::migrate::{Migration, Migrator};
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
    let migrator = Migrator::new(migrations_path)
        .await
        .expect("cargar las migraciones");
    migrator.run(&pool).await.expect("aplicar las migraciones");

    pool
}

/// Pool `SQLite` en memoria con todas las migraciones **menos** la de
/// `rule_conditions`, que se devuelve aparte para aplicarla después del backfill.
///
/// Sirve para simular una base ya desplegada antes de esta sub-entrega. En la
/// fase RED la migración de `rule_conditions` no existe: se aplican todas las
/// actuales y se devuelve `None`, de modo que el test del backfill falla al no
/// encontrar la migración que debe ejercitar.
async fn migrated_pool_without_conditions() -> (SqlitePool, Option<String>) {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("url sqlite válida")
        .create_if_missing(true);
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria");

    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = Migrator::new(migrations_path)
        .await
        .expect("cargar las migraciones");
    let mut migrations: Vec<Migration> = migrator.migrations.clone().into_owned();
    migrations.sort_by_key(|migration| migration.version);

    // Localizada por su contenido, nunca por un número mágico: en RED no existe
    // y `conditions_sql` queda `None`.
    let mut conditions_sql: Option<String> = None;
    for migration in &migrations {
        let is_conditions = migration.description.contains("rule_conditions")
            || migration.sql.contains("rule_conditions");
        if is_conditions {
            conditions_sql = Some(migration.sql.as_ref().to_string());
            continue;
        }
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&pool)
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "aplicar la migración {} ({}): {error}",
                    migration.version, migration.description
                )
            });
    }

    (pool, conditions_sql)
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

// ---------------------------------------------------------------------------
// Constructores
// ---------------------------------------------------------------------------

/// Una [`NewRule`] sin ningún campo restringido, con los valores por defecto que
/// la API resuelve en `Rule::create`.
fn blank_new_rule(name: &str) -> NewRule {
    NewRule {
        name: name.to_string(),
        description: Some(String::new()),
        weight: Some(100),
        mode: Some("enforce".to_string()),
        pipeline: Some("waf".to_string()),
        transformations: Some(String::new()),
        allow: Some(false),
        is_tor: None,
        ip_address: None,
        protocol: None,
        fqdn: None,
        path: None,
        query: None,
        city_name: None,
        country_name: None,
        country_code: None,
        user_agent: None,
        method: None,
        referer: None,
        content_type: None,
        accept_language: None,
        x_request_id: None,
        rate_limit_profile_id: None,
        active: Some(true),
    }
}

/// Una [`UpdateRule`] con los mismos 14 campos que se le pasen; el resto, neutro.
fn blank_update_rule(id: i32, name: &str) -> UpdateRule {
    UpdateRule {
        id,
        name: name.to_string(),
        description: String::new(),
        weight: 100,
        mode: "enforce".to_string(),
        pipeline: "waf".to_string(),
        transformations: String::new(),
        allow: false,
        // La columna `rules.is_tor` es `NOT NULL`, así que el `PATCH` tiene que
        // llevar un valor (un `null` falla con una restricción ajena a este
        // change).
        is_tor: Some(false),
        ip_address: None,
        protocol: None,
        fqdn: None,
        path: None,
        query: None,
        city_name: None,
        country_name: None,
        country_code: None,
        user_agent: None,
        method: None,
        referer: None,
        content_type: None,
        accept_language: None,
        x_request_id: None,
        rate_limit_profile_id: None,
        active: true,
    }
}

/// La [`NewRule`] equivalente a una plantilla del catálogo (mismo mapeo que
/// `test_project_is_faithful_over_the_whole_template_catalog`).
fn new_rule_from_template(template: &RuleTemplate) -> NewRule {
    let mut rule = blank_new_rule(&template.name);
    rule.description = Some(template.description.clone());
    rule.weight = Some(template.weight);
    rule.pipeline = Some(template.pipeline.clone());
    rule.transformations = Some(template.transformations.clone());
    rule.allow = Some(template.allow);
    rule.is_tor = template.is_tor;
    rule.ip_address.clone_from(&template.ip_address);
    rule.protocol.clone_from(&template.protocol);
    rule.fqdn.clone_from(&template.fqdn);
    rule.path.clone_from(&template.path);
    rule.query.clone_from(&template.query);
    rule.city_name.clone_from(&template.city_name);
    rule.country_name.clone_from(&template.country_name);
    rule.country_code.clone_from(&template.country_code);
    rule.user_agent.clone_from(&template.user_agent);
    rule.method.clone_from(&template.method);
    rule.referer.clone_from(&template.referer);
    rule.content_type.clone_from(&template.content_type);
    rule.accept_language.clone_from(&template.accept_language);
    rule.x_request_id.clone_from(&template.x_request_id);
    rule.rate_limit_profile_id = template.rate_limit_profile_id;
    rule
}

/// El patrón fuente de un operador, para compararlo sin exponer el `Regex`.
fn operator_pattern(operator: &Operator) -> &str {
    match operator {
        Operator::Rx(regex) => regex.as_str(),
    }
}

/// El conjunto observable de una regla: `(collection, pattern)` de cada
/// condición, en orden.
fn observable(conditions: &[backend::models::condition::Condition]) -> Vec<(String, String)> {
    conditions
        .iter()
        .map(|condition| {
            (
                condition.collection.field_name().to_string(),
                operator_pattern(&condition.operator).to_string(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Lectura cruda de `rule_conditions`
// ---------------------------------------------------------------------------

/// La fila cruda de `rule_conditions` tal y como la devuelve `query_as`
/// (`position, collection, key, transformations, operator, pattern, negate`).
type RawConditionRow = (i32, String, Option<String>, String, String, String, i32);

/// Una fila completa de `rule_conditions` tal y como está almacenada.
#[derive(Debug, PartialEq, Eq)]
struct StoredCondition {
    position: i32,
    collection: String,
    key: Option<String>,
    transformations: String,
    operator: String,
    pattern: String,
    negate: i32,
}

/// Lee las condiciones almacenadas de una regla, ordenadas por `position`.
///
/// Consulta la tabla por SQL crudo: es el contrato directo de la persistencia.
/// En RED falla porque la tabla todavía no existe.
async fn stored_conditions(pool: &SqlitePool, rule_id: i32) -> Vec<StoredCondition> {
    let rows: Vec<RawConditionRow> = sqlx::query_as(
        "SELECT position, collection, key, transformations, operator, pattern, negate
         FROM rule_conditions WHERE rule_id = ? ORDER BY position ASC",
    )
    .bind(rule_id)
    .fetch_all(pool)
    .await
    .unwrap_or_else(|error| panic!("leer `rule_conditions` de la regla {rule_id}: {error}"));

    rows.into_iter()
        .map(
            |(position, collection, key, transformations, operator, pattern, negate)| {
                StoredCondition {
                    position,
                    collection,
                    key,
                    transformations,
                    operator,
                    pattern,
                    negate,
                }
            },
        )
        .collect()
}

/// Cuenta las filas de `rule_conditions` de una regla.
async fn stored_conditions_count(pool: &SqlitePool, rule_id: i32) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM rule_conditions WHERE rule_id = ?")
        .bind(rule_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("contar `rule_conditions` de la regla {rule_id}: {error}"))
}

// ---------------------------------------------------------------------------
// 1.1 — Un campo restringido genera su fila
// ---------------------------------------------------------------------------

/// Escenario: «Un campo restringido genera su fila».
///
/// GIVEN una regla con `path` y `user_agent` restringidos
/// WHEN se crea por la API
/// THEN SHALL existir una fila por cada campo, con `collection` igual al nombre
///      del campo y `pattern` igual al patrón de la regla
/// AND el orden SHALL ser el de `Collection::ALL` (`path` antes que `user_agent`)
/// AND `operator` SHALL ser `@rx`, `key` nulo y `negate` 0.
#[tokio::test]
async fn test_creating_a_rule_persists_one_condition_per_restricted_field() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("rc-two-fields");
    rule.path = Some("^/admin$".to_string());
    rule.user_agent = Some("curl".to_string());
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con dos campos restringidos");

    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        2,
        "una regla con `path` y `user_agent` debe persistir dos condiciones; \
         almacenadas: {rows:?}"
    );

    assert_eq!(
        rows[0],
        StoredCondition {
            position: 0,
            collection: "path".to_string(),
            key: None,
            transformations: String::new(),
            operator: "@rx".to_string(),
            pattern: "^/admin$".to_string(),
            negate: 0,
        },
        "la primera condición debe ser `path` (0-based, en el orden de `Collection::ALL`)"
    );
    assert_eq!(
        rows[1],
        StoredCondition {
            position: 1,
            collection: "user_agent".to_string(),
            key: None,
            transformations: String::new(),
            operator: "@rx".to_string(),
            pattern: "curl".to_string(),
            negate: 0,
        },
        "la segunda condición debe ser `user_agent`, con `position` contigua a `path`"
    );
}

// ---------------------------------------------------------------------------
// 1.2 — Un campo en blanco no genera fila
// ---------------------------------------------------------------------------

/// Escenario: «Un campo en blanco no genera fila».
///
/// GIVEN una regla con `path` restringido y `query` en blanco
/// THEN SHALL existir solo la fila de `path`.
#[tokio::test]
async fn test_blank_field_does_not_persist_a_condition() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("rc-blank-field");
    rule.path = Some("^/admin$".to_string());
    rule.query = Some(String::new());
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con `path` y `query` en blanco");

    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        1,
        "un campo en blanco no debe generar fila: solo cabe la de `path`; \
         almacenadas: {rows:?}"
    );
    assert_eq!(
        rows[0].collection, "path",
        "la única fila debe ser la de `path`"
    );
    assert_eq!(rows[0].pattern, "^/admin$");
}

// ---------------------------------------------------------------------------
// 1.3 — El juego de condiciones se reemplaza al editar
// ---------------------------------------------------------------------------

/// Escenario: «El juego de condiciones se reemplaza al editar».
///
/// GIVEN una regla con `path` persistido
/// WHEN se edita para restringir `query`
/// THEN la tabla SHALL contener exactamente la condición de la nueva versión
/// AND NO SHALL quedar ninguna fila de la anterior.
#[tokio::test]
async fn test_editing_a_rule_replaces_its_conditions() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("rc-replaced");
    rule.path = Some("^/admin$".to_string());
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con `path`");

    let before = stored_conditions(&pool, created.id).await;
    assert_eq!(
        before.len(),
        1,
        "la creación debe persistir la condición de `path`"
    );

    let mut update = blank_update_rule(created.id, "rc-replaced");
    update.query = Some("union".to_string());
    Rule::update(&pool, update)
        .await
        .expect("editar la regla para restringir `query`");

    let after = stored_conditions(&pool, created.id).await;
    assert_eq!(
        after.len(),
        1,
        "editar debe dejar exactamente el conjunto de la nueva versión; \
         almacenadas: {after:?}"
    );
    assert_eq!(
        after[0].collection, "query",
        "la única fila debe ser la de la nueva versión (`query`)"
    );
    assert_eq!(after[0].pattern, "union");
    assert!(
        after.iter().all(|row| row.collection != "path"),
        "no puede quedar ninguna fila de la versión anterior (`path`); \
         almacenadas: {after:?}"
    );
}

// ---------------------------------------------------------------------------
// 1.4 — Las condiciones se borran con su regla
// ---------------------------------------------------------------------------

/// Escenario: «Las condiciones se borran con su regla».
///
/// GIVEN una regla con condiciones persistidas
/// WHEN se elimina la regla
/// THEN sus filas de `rule_conditions` SHALL desaparecer (cascada).
#[tokio::test]
async fn test_deleting_a_rule_cascades_its_conditions() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("rc-cascade");
    rule.path = Some("^/admin$".to_string());
    rule.user_agent = Some("curl".to_string());
    let created = Rule::create(&pool, rule).await.expect("crear la regla");

    assert_eq!(
        stored_conditions_count(&pool, created.id).await,
        2,
        "la regla debe tener sus dos condiciones antes de borrarla"
    );

    Rule::delete(&pool, created.id)
        .await
        .expect("eliminar la regla");

    assert_eq!(
        stored_conditions_count(&pool, created.id).await,
        0,
        "`ON DELETE CASCADE` debe llevarse las condiciones al eliminar la regla"
    );
}

// ---------------------------------------------------------------------------
// 1.5 — La importación persiste las condiciones
// ---------------------------------------------------------------------------

/// Escenario (requisito «Escribir una regla SHALL escribir las 14 columnas y
/// proyectarlas a `rule_conditions`», vía de importación).
///
/// GIVEN una importación con una regla que restringe `path` y otra que restringe
/// `query`
/// WHEN se importan por el handler real
/// THEN cada regla SHALL tener persistida su condición.
#[tokio::test]
async fn test_importing_rules_persists_their_conditions() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, response) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({
            "rules": [
                {
                    "name": "rc-import-path",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "waf",
                    "allow": false,
                    "path": "^/admin$"
                },
                {
                    "name": "rc-import-query",
                    "description": "",
                    "weight": 100,
                    "mode": "enforce",
                    "pipeline": "waf",
                    "allow": false,
                    "query": "union"
                }
            ]
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la importación debe aceptarse; respuesta: {response}"
    );
    assert_eq!(
        response["imported"].as_u64(),
        Some(2),
        "las dos reglas deben importarse; respuesta: {response}"
    );

    let path_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("rc-import-path")
        .fetch_one(&pool)
        .await
        .expect("id de rc-import-path");
    let query_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("rc-import-query")
        .fetch_one(&pool)
        .await
        .expect("id de rc-import-query");

    let path_rows = stored_conditions(&pool, path_id).await;
    assert_eq!(
        path_rows.len(),
        1,
        "la regla importada con `path` debe persistir su condición; almacenadas: {path_rows:?}"
    );
    assert_eq!(path_rows[0].collection, "path");
    assert_eq!(path_rows[0].pattern, "^/admin$");

    let query_rows = stored_conditions(&pool, query_id).await;
    assert_eq!(
        query_rows.len(),
        1,
        "la regla importada con `query` debe persistir su condición; almacenadas: {query_rows:?}"
    );
    assert_eq!(query_rows[0].collection, "query");
    assert_eq!(query_rows[0].pattern, "union");
}

// ---------------------------------------------------------------------------
// 1.7 — El ciclo export/import es estable
// ---------------------------------------------------------------------------

/// Escenario: «El ciclo export/import es estable».
///
/// GIVEN un conjunto de reglas con condiciones persistidas
/// WHEN se exportan por el handler y se importa el JSON resultante
/// THEN sus condiciones persistidas SHALL coincidir con las de antes.
#[tokio::test]
async fn test_export_import_round_trip_keeps_conditions() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let mut first = blank_new_rule("rc-roundtrip-1");
    first.path = Some("^/admin$".to_string());
    first.user_agent = Some("curl".to_string());
    let created_first = Rule::create(&pool, first).await.expect("crear la regla 1");

    let mut second = blank_new_rule("rc-roundtrip-2");
    second.query = Some("union".to_string());
    let created_second = Rule::create(&pool, second).await.expect("crear la regla 2");

    let before_first = stored_conditions(&pool, created_first.id).await;
    let before_second = stored_conditions(&pool, created_second.id).await;

    let (status, exported) = call_rules_api(&state, Method::GET, "/export", None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la exportación debe responder 200; respuesta: {exported}"
    );
    let exported_rules = exported["data"]
        .as_array()
        .unwrap_or_else(|| panic!("la exportación debe traer el array de reglas: {exported}"));

    let (status, imported) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({ "rules": exported_rules })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la importación del JSON exportado debe aceptarse; respuesta: {imported}"
    );

    let after_first = stored_conditions(&pool, created_first.id).await;
    let after_second = stored_conditions(&pool, created_second.id).await;
    assert_eq!(
        after_first, before_first,
        "el ciclo export/import no puede cambiar las condiciones de la regla 1"
    );
    assert_eq!(
        after_second, before_second,
        "el ciclo export/import no puede cambiar las condiciones de la regla 2"
    );
}

// ---------------------------------------------------------------------------
// 1.5 (delta) — La migración rellena las reglas existentes
// ---------------------------------------------------------------------------

/// Escenario: «La migración rellena las reglas existentes».
///
/// GIVEN una base con reglas almacenadas antes de esta sub-entrega
/// WHEN se aplica la migración de `rule_conditions`
/// THEN cada regla SHALL recibir sus filas, una por campo restringido, con
///      `position` contigua 0-based
/// AND una regla sin campos restringidos SHALL no recibir ninguna fila.
#[tokio::test]
async fn test_migration_backfills_existing_rules() {
    let (pool, conditions_sql) = migrated_pool_without_conditions().await;

    let conditions_sql = conditions_sql.unwrap_or_else(|| {
        panic!(
            "no existe ninguna migración que mencione `rule_conditions` en `migrations/`: \
             es el motivo por el que este test falla en la fase RED"
        )
    });

    // Filas «legacy»: escritas por SQL crudo, sin pasar por la API (que en RED
    // todavía no proyecta a la tabla).
    sqlx::query(
        "INSERT INTO rules
            (name, description, weight, mode, pipeline, allow,
             path, user_agent, active, created_at, updated_at)
         VALUES (?, '', 100, 'enforce', 'waf', 0, ?, ?, 1, datetime('now'), datetime('now'))",
    )
    .bind("legacy-two-fields")
    .bind("^/admin$")
    .bind("curl")
    .execute(&pool)
    .await
    .expect("insertar la regla legacy con dos campos");

    sqlx::query(
        "INSERT INTO rules
            (name, description, weight, mode, pipeline, allow,
             active, created_at, updated_at)
         VALUES (?, '', 100, 'enforce', 'waf', 1, 1, datetime('now'), datetime('now'))",
    )
    .bind("legacy-no-fields")
    .execute(&pool)
    .await
    .expect("insertar la regla legacy sin campos");

    sqlx::raw_sql(&conditions_sql)
        .execute(&pool)
        .await
        .expect("aplicar la migración de `rule_conditions`");

    let two_fields_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("legacy-two-fields")
        .fetch_one(&pool)
        .await
        .expect("id de legacy-two-fields");
    let no_fields_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("legacy-no-fields")
        .fetch_one(&pool)
        .await
        .expect("id de legacy-no-fields");

    let rows = stored_conditions(&pool, two_fields_id).await;
    assert_eq!(
        rows.len(),
        2,
        "el backfill debe rellenar una fila por campo restringido; almacenadas: {rows:?}"
    );
    assert_eq!(
        (rows[0].position, rows[0].collection.as_str()),
        (0, "path"),
        "`path` va primero, con `position` 0"
    );
    assert_eq!(
        (rows[1].position, rows[1].collection.as_str()),
        (1, "user_agent"),
        "`user_agent` sigue a `path` con `position` contigua"
    );

    assert_eq!(
        stored_conditions_count(&pool, no_fields_id).await,
        0,
        "una regla sin campos restringidos no debe recibir ninguna fila"
    );
}

// ---------------------------------------------------------------------------
// 1.6 — Equivalencia sobre las plantillas
// ---------------------------------------------------------------------------

/// Escenario: «Equivalencia sobre las plantillas».
///
/// GIVEN cada plantilla del catálogo persistida y recargada
/// THEN las condiciones leídas de la tabla SHALL ser las mismas que proyecta la
///      regla desde los 14 campos (misma `collection`, mismo patrón, mismo orden)
/// AND el conjunto que construye `read_all_active` —alimentado por la tabla—
///      SHALL ser el mismo.
#[tokio::test]
async fn test_projection_from_table_is_equivalent_over_the_catalog() {
    let pool = create_migrated_pool().await;
    let templates = all_rule_templates();
    assert_eq!(
        templates.len(),
        42,
        "el catálogo son 24 plantillas WAF más 18 de Jail"
    );

    for template in &templates {
        let created = Rule::create(&pool, new_rule_from_template(template))
            .await
            .unwrap_or_else(|error| panic!("«{}»: crear la regla: {error}", template.name));

        // Esperado: la proyección desde los 14 campos de la regla persistida.
        let pipeline = TransformationPipeline::parse(&created.transformations).unwrap_or_default();
        let expected = observable(&project(&created, &pipeline));

        // La tabla: lo que se ha persistido al escribir la regla.
        let stored: Vec<(String, String)> = sqlx::query_as(
            "SELECT collection, pattern FROM rule_conditions WHERE rule_id = ? ORDER BY position ASC",
        )
        .bind(created.id)
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|error| panic!("«{}»: leer `rule_conditions`: {error}", template.name));

        assert_eq!(
            stored, expected,
            "«{}»: las condiciones persistidas deben ser las mismas que proyectan los 14 campos",
            template.name
        );

        // La caché: `read_all_active` construye el conjunto leyendo la tabla.
        let cache = CacheRule::read_all_active(&pool)
            .await
            .expect("leer las reglas activas");
        let cached = cache
            .iter()
            .find(|cache_rule| cache_rule.rule.id == created.id)
            .unwrap_or_else(|| {
                panic!(
                    "«{}»: la regla creada debe estar en la caché",
                    template.name
                )
            });
        assert_eq!(
            observable(&cached.conditions),
            expected,
            "«{}»: `read_all_active` debe construir el conjunto de condiciones desde la tabla, \
             equivalente al de los 14 campos",
            template.name
        );

        // Se limpia la regla usada para no acumular estado entre plantillas (el
        // nombre de una plantilla es `UNIQUE`).
        Rule::delete(&pool, created.id)
            .await
            .unwrap_or_else(|error| panic!("«{}»: limpiar la regla: {error}", template.name));
    }

    // Control explícito del vocabulario: el nombre `path` de la tabla es el de
    // `Collection::Path`.
    assert_eq!(Collection::Path.field_name(), "path");
}

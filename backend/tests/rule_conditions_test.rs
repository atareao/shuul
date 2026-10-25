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
use backend::models::condition::{Collection, Condition, Operator, project};
use backend::models::transformation::TransformationPipeline;
use backend::models::{
    AppState, BanManager, CacheRule, ConditionInput, GeoIpService, NewRequest, NewRule,
    RateLimiter, Rule, Settings, StatsCollector, TorService, UpdateRule,
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

/// Pool `SQLite` en memoria con todas las migraciones **excepto** las dos de
/// `rule_conditions`, que se devuelven aparte: la que **crea** la tabla (F2b-1) y
/// la que **proyecta** `is_tor` (F2b-3).
///
/// Sirve para simular una base ya desplegada antes de F2b-1 (el test de su
/// backfill) y antes de F2b-3 (el test del backfill de `is_tor`). Las dos
/// migraciones se localizan por su **contenido** —y nunca por un número mágico—:
/// la de `rule_conditions` sin `is_tor` es la creación de la tabla, la que
/// menciona ambas es el backfill de `is_tor`. En RED la de `is_tor` no existe y
/// su hueco queda `None`.
async fn migrated_pool_without_conditions_migrations()
-> (SqlitePool, Option<String>, Option<String>) {
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

    let mut create_sql: Option<String> = None;
    let mut is_tor_sql: Option<String> = None;
    for migration in &migrations {
        let sql = migration.sql.as_ref();
        let mentions_rule_conditions =
            migration.description.contains("rule_conditions") || sql.contains("rule_conditions");
        let mentions_is_tor = migration.description.contains("is_tor") || sql.contains("is_tor");
        if mentions_rule_conditions && mentions_is_tor {
            is_tor_sql = Some(sql.to_string());
            continue;
        }
        if mentions_rule_conditions {
            create_sql = Some(sql.to_string());
            continue;
        }
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "aplicar la migración {} ({}): {error}",
                    migration.version, migration.description
                )
            });
    }

    (pool, create_sql, is_tor_sql)
}

/// Pool `SQLite` en memoria con todas las migraciones **menos** la que crea
/// `rule_conditions` (F2b-1), que se devuelve aparte para aplicarla después del
/// backfill. Es el harness del test de backfill de F2b-1.
async fn migrated_pool_without_conditions() -> (SqlitePool, Option<String>) {
    let (pool, create_sql, _is_tor_sql) = migrated_pool_without_conditions_migrations().await;
    (pool, create_sql)
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
        conditions: None,
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
        conditions: None,
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
        Operator::Truthy => "@truthy",
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

/// La representación **serializable** de una condición: `(collection, operator,
/// pattern)`.
///
/// A diferencia de [`observable`], incluye el nombre del operador y el patrón tal
/// y como se persiste (`""` para el operador booleano), de modo que la
/// equivalencia cubre también `is_tor`/`@truthy`, que no lleva patrón.
fn serialized(conditions: &[Condition]) -> Vec<(String, String, String)> {
    conditions
        .iter()
        .map(|condition| {
            let pattern = match &condition.operator {
                Operator::Rx(regex) => regex.as_str(),
                Operator::Truthy => "",
            };
            (
                condition.collection.field_name().to_string(),
                condition.operator.name().to_string(),
                pattern.to_string(),
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

        // Esperado: la proyección desde los 14 campos y la columna `is_tor` de la
        // regla persistida.
        let pipeline = TransformationPipeline::parse(&created.transformations).unwrap_or_default();
        let expected = serialized(&project(&created, &pipeline));

        // La tabla: lo que se ha persistido al escribir la regla. Se compara el
        // operador además del patrón, porque la proyección incluye el operador
        // booleano `@truthy` de `is_tor`, cuyo patrón es vacío y no lo distingue.
        let stored: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT collection, operator, pattern FROM rule_conditions WHERE rule_id = ? ORDER BY position ASC",
        )
        .bind(created.id)
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|error| panic!("«{}»: leer `rule_conditions`: {error}", template.name));

        assert_eq!(
            stored, expected,
            "«{}»: las condiciones persistidas deben ser las mismas que proyectan los 14 campos y `is_tor`",
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
            serialized(&cached.conditions),
            expected,
            "«{}»: `read_all_active` debe construir el conjunto de condiciones desde la tabla, \
             equivalente al de los 14 campos y `is_tor`",
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

// ---------------------------------------------------------------------------
// waf-collections (F2b-2a) — persistencia con clave y autoría avanzada
// ---------------------------------------------------------------------------
//
// Contrato verificado:
// - Las condiciones con clave se persisten en `rule_conditions` con su `key`.
// - Las condiciones sin clave se persisten con `key` nula.
// - Las condiciones inválidas se rechazan o se descartan al leer: colección
//   desconocida, patrón no compilable, transformaciones no legibles y clave
//   ausente en una colección que la exige.
// - La importación rechaza por entradas las condiciones inválidas sin abortar
//   el resto de la importación.
//
// La persistencia se verifica aquí —y no en `#[cfg(test)]` de `rule.rs`— porque
// el harness de base de datos migrada en memoria ya vive en este fichero de
// integración; moverla evita duplicar helpers.

/// Construye una condición serializable para los cuerpos JSON de la API.
fn condition_input_json(collection: &str, key: Option<&str>, pattern: &str) -> Value {
    json!({
        "collection": collection,
        "key": key,
        "transformations": "",
        "operator": null,
        "pattern": pattern,
    })
}

/// Escenario 1: «Una condición con clave se conserva en el ciclo
/// escritura-lectura» (`specs/rules/spec.md`).
///
/// GIVEN una regla con una condición de `request_cookies` y clave `session`
/// WHEN se crea y se relee por la ruta de producción (`read_all_active`)
/// THEN la condición leída SHALL tener clave `session`.
#[tokio::test]
async fn test_condition_with_key_survives_write_read() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("wc-key-survives");
    rule.conditions = Some(vec![ConditionInput {
        collection: "request_cookies".to_string(),
        key: Some("session".to_string()),
        transformations: String::new(),
        operator: None,
        pattern: "^abc$".to_string(),
    }]);

    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con condición con clave");

    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == created.id)
        .expect("la regla creada debe estar en la caché");

    assert_eq!(
        cached.conditions.len(),
        1,
        "la condición con clave debe persistirse y releerse; obtenidas: {:?}",
        cached.conditions
    );
    assert_eq!(cached.conditions[0].collection, Collection::RequestCookies);
    assert_eq!(
        cached.conditions[0].key.as_deref(),
        Some("session"),
        "la clave debe sobrevivir al ciclo escritura-lectura"
    );
}

/// Escenario 2: «Una condición sin clave la conserva nula».
///
/// GIVEN una regla con una condición de `path`
/// WHEN se crea y se relee
/// THEN su clave SHALL ser nula.
#[tokio::test]
async fn test_condition_without_key_stays_null() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("wc-key-null");
    rule.conditions = Some(vec![ConditionInput {
        collection: "path".to_string(),
        key: None,
        transformations: String::new(),
        operator: None,
        pattern: "^/admin$".to_string(),
    }]);

    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con condición sin clave");

    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == created.id)
        .expect("la regla creada debe estar en la caché");

    assert_eq!(
        cached.conditions.len(),
        1,
        "la condición sin clave debe persistirse y releerse; obtenidas: {:?}",
        cached.conditions
    );
    assert_eq!(cached.conditions[0].collection, Collection::Path);
    assert_eq!(cached.conditions[0].key, None, "la clave debe quedar nula");
}

/// Escenario 3: «El modo simple se conserva» (proyección de los 14 campos).
///
/// Guardián: en RED el modo simple ya funciona. Fija que una regla creada sin
/// `conditions` siga proyectando sus 14 campos y se relea desde la tabla.
#[tokio::test]
async fn test_simple_mode_conditions_are_projected() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("wc-simple-projection");
    rule.path = Some("^/admin$".to_string());
    // `conditions` sigue en `None`: modo simple.

    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla en modo simple");

    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == created.id)
        .expect("la regla creada debe estar en la caché");

    assert_eq!(
        cached.conditions.len(),
        1,
        "el modo simple debe proyectar la condición de `path`; obtenidas: {:?}",
        cached.conditions
    );
    assert_eq!(cached.conditions[0].collection, Collection::Path);
    assert_eq!(cached.conditions[0].key, None);
}

/// Escenario 4: «Una regla avanzada persiste su lista tal cual».
///
/// GIVEN una petición de creación con dos condiciones
/// WHEN se almacena
/// THEN el conjunto persistido SHALL contener esas dos condiciones, en orden,
///      con sus claves.
#[tokio::test]
async fn test_create_advanced_rule_persists_conditions_in_order() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-advanced-order",
        "description": "",
        "weight": 100,
        "mode": "enforce",
        "pipeline": "waf",
        "allow": false,
        "active": true,
        "conditions": [
            condition_input_json("request_cookies", Some("PHPSESSID"), "^abc$"),
            condition_input_json("path", None, "^/admin$"),
        ],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "la creación debe aceptarse; respuesta: {response}"
    );

    let rule_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-advanced-order")
        .fetch_one(&pool)
        .await
        .expect("id de wc-advanced-order");

    let rows = stored_conditions(&pool, rule_id).await;
    assert_eq!(
        rows.len(),
        2,
        "deben persistirse las dos condiciones de la lista; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].collection, "request_cookies");
    assert_eq!(rows[0].key.as_deref(), Some("PHPSESSID"));
    assert_eq!(rows[1].collection, "path");
    assert_eq!(rows[1].key, None);
}

/// Escenario 5: «Una colección con clave sin clave se rechaza».
#[tokio::test]
async fn test_advanced_rule_rejects_missing_key_for_keyed_collection() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-missing-key",
        "conditions": [condition_input_json("request_cookies", None, "^abc$")],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "una colección con clave sin clave debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("wc-missing-key")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

/// Escenario 6: «Una clave en una colección que no la admite se rechaza».
#[tokio::test]
async fn test_advanced_rule_rejects_key_for_non_keyed_collection() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-extra-key",
        "conditions": [condition_input_json("path", Some("sobra"), "^/admin$")],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "una clave en una colección que no la admite debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("wc-extra-key")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

/// Escenario 7: «Una colección desconocida se rechaza».
#[tokio::test]
async fn test_advanced_rule_rejects_unknown_collection() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-unknown-collection",
        "conditions": [condition_input_json("nope", None, "^abc$")],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "una colección desconocida debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("wc-unknown-collection")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

/// Escenario 8: «Un patrón que no compila se rechaza».
#[tokio::test]
async fn test_advanced_rule_rejects_non_compiling_pattern() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-bad-pattern",
        "conditions": [condition_input_json("path", None, "[")],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "un patrón que no compila debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("wc-bad-pattern")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

/// Escenario 9: «El ciclo de una regla avanzada es estable».
///
/// GIVEN una regla avanzada creada por la API
/// WHEN se lee y se reenvían sus condiciones por `PATCH`
/// THEN la regla SHALL quedar descrita igual.
#[tokio::test]
async fn test_advanced_rule_round_trip_is_stable() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-roundtrip",
        "conditions": [
            condition_input_json("request_cookies", Some("session"), "^abc$"),
            condition_input_json("path", None, "^/admin$"),
        ],
    });
    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(status, StatusCode::CREATED, "creación: {response}");

    let rule_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-roundtrip")
        .fetch_one(&pool)
        .await
        .expect("id de wc-roundtrip");

    // Primera lectura: la representación debe incluir las condiciones.
    let (status, read) =
        call_rules_api(&state, Method::GET, &format!("/?id={rule_id}"), None).await;
    assert_eq!(status, StatusCode::OK, "lectura: {read}");
    let read_conditions = read["data"]["conditions"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("la representación de la regla debe incluir `conditions`: {read}")
        })
        .clone();
    assert_eq!(read_conditions.len(), 2);

    // Reenvío por PATCH con las mismas condiciones.
    let mut update = serde_json::to_value(blank_update_rule(rule_id, "wc-roundtrip"))
        .expect("serializar la actualización");
    update["conditions"] = Value::Array(read_conditions.clone());
    let (status, patched) = call_rules_api(&state, Method::PATCH, "/", Some(update)).await;
    assert_eq!(status, StatusCode::OK, "edición: {patched}");

    // Segunda lectura: debe coincidir con la primera.
    let (status, read_again) =
        call_rules_api(&state, Method::GET, &format!("/?id={rule_id}"), None).await;
    assert_eq!(status, StatusCode::OK, "segunda lectura: {read_again}");
    let again = read_again["data"]["conditions"]
        .as_array()
        .unwrap_or_else(|| panic!("la segunda lectura debe incluir `conditions`: {read_again}"));
    assert_eq!(again, &read_conditions, "el ciclo debe ser estable");
}

/// Escenario 10: «La exportación e importación conservan una regla avanzada».
#[tokio::test]
async fn test_export_import_preserves_advanced_rule() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "wc-export-import",
        "is_tor": true,
        "conditions": [
            condition_input_json("request_cookies", Some("session"), "^abc$"),
            condition_input_json("path", None, "^/admin$"),
        ],
    });
    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(status, StatusCode::CREATED, "creación: {response}");

    let rule_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-export-import")
        .fetch_one(&pool)
        .await
        .expect("id de wc-export-import");

    let (status, exported) = call_rules_api(&state, Method::GET, "/export", None).await;
    assert_eq!(status, StatusCode::OK, "exportación: {exported}");
    let exported_rules = exported["data"]
        .as_array()
        .expect("la exportación debe traer el array de reglas");
    let exported_rule = exported_rules
        .iter()
        .find(|rule| rule["name"] == "wc-export-import")
        .expect("la regla avanzada debe estar en la exportación");
    let exported_conditions = exported_rule["conditions"]
        .as_array()
        .unwrap_or_else(|| panic!("la exportación debe incluir `conditions`: {exported_rule}"));
    assert_eq!(exported_conditions.len(), 2);

    // Se borra la regla y se reimporta el JSON exportado.
    Rule::delete(&pool, rule_id)
        .await
        .expect("borrar la regla avanzada");
    let (status, imported) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({ "rules": [exported_rule] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "importación: {imported}");

    let new_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-export-import")
        .fetch_one(&pool)
        .await
        .expect("id de la regla reimportada");
    let stored_is_tor: bool = sqlx::query_scalar("SELECT is_tor FROM rules WHERE id = ?")
        .bind(new_id)
        .fetch_one(&pool)
        .await
        .expect("is_tor de la regla reimportada");
    assert!(
        stored_is_tor,
        "la importación debe conservar el predicado `is_tor = true`"
    );
    let rows = stored_conditions(&pool, new_id).await;
    assert_eq!(
        rows.len(),
        2,
        "la importación debe conservar las dos condiciones; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].key.as_deref(), Some("session"));
}

/// Escenario 11: «La importación sin condiciones proyecta los 14 campos».
#[tokio::test]
async fn test_import_without_conditions_projects_14_fields() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let (status, imported) = call_rules_api(
        &state,
        Method::POST,
        "/import",
        Some(json!({
            "rules": [{
                "name": "wc-import-simple",
                "description": "",
                "weight": 100,
                "mode": "enforce",
                "pipeline": "waf",
                "allow": false,
                "path": "^/admin$",
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "importación: {imported}");

    let rule_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-import-simple")
        .fetch_one(&pool)
        .await
        .expect("id de wc-import-simple");

    // La tabla debe tener la condición proyectada desde los 14 campos.
    let rows = stored_conditions(&pool, rule_id).await;
    assert_eq!(
        rows.len(),
        1,
        "la proyección debe persistir `path`; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].collection, "path");

    // La representación devuelta por la API debe incluir las condiciones.
    let (status, read) =
        call_rules_api(&state, Method::GET, &format!("/?id={rule_id}"), None).await;
    assert_eq!(status, StatusCode::OK, "lectura: {read}");
    let conditions = read["data"]["conditions"].as_array().unwrap_or_else(|| {
        panic!("la representación de la regla debe incluir las condiciones proyectadas: {read}")
    });
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0]["collection"], "path");
}

/// Escenario: Una condición con colección con clave y `key` nula se descarta al
/// leer. La regla queda con las demás condiciones.
#[tokio::test]
async fn test_invalid_condition_missing_key_is_discarded() {
    let pool = create_migrated_pool().await;

    // Crear una regla válida con una condición de `path`.
    let mut rule = blank_new_rule("wc-invalid-key");
    rule.conditions = Some(vec![ConditionInput {
        collection: "path".to_string(),
        key: None,
        transformations: String::new(),
        operator: None,
        pattern: "^/admin$".to_string(),
    }]);

    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con condición válida");

    // Insertar a mano una fila inválida: request_cookies sin key.
    sqlx::query(
        "INSERT INTO rule_conditions (rule_id, position, collection, key, transformations, operator, pattern, negate)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(created.id)
    .bind(1)
    .bind("request_cookies")
    .bind(None::<String>)
    .bind("")
    .bind("@rx")
    .bind("^abc$")
    .bind(0)
    .execute(&pool)
    .await
    .expect("insertar condición inválida");

    // La tabla debe tener las dos condiciones (la válida y la inválida).
    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        2,
        "la tabla debe tener las dos condiciones; almacenadas: {rows:?}"
    );

    // Al leer, la condición inválida debe descartarse.
    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == created.id)
        .expect("la regla creada debe estar en la caché");
    assert_eq!(
        cached.conditions.len(),
        1,
        "la condición inválida debe descartarse; obtenidas: {:?}",
        cached.conditions
    );
    assert_eq!(cached.conditions[0].collection, Collection::Path);
}

/// Escenario: Un `PATCH` con una condición inválida se rechaza y la regla
/// conserva sus condiciones originales.
#[tokio::test]
async fn test_advanced_rule_reject_invalid_condition_on_update() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    // Crear una regla válida con una condición de `path`.
    let mut rule = blank_new_rule("wc-invalid-update");
    rule.conditions = Some(vec![ConditionInput {
        collection: "path".to_string(),
        key: None,
        transformations: String::new(),
        operator: None,
        pattern: "^/admin$".to_string(),
    }]);

    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla válida");

    // Preparar una actualización con una condición inválida: request_cookies sin key.
    let mut update = blank_update_rule(created.id, "wc-invalid-update");
    update.conditions = Some(vec![ConditionInput {
        collection: "request_cookies".to_string(),
        key: None,
        transformations: String::new(),
        operator: None,
        pattern: "^abc$".to_string(),
    }]);

    let (status, response) = call_rules_api(
        &state,
        Method::PATCH,
        "/",
        Some(serde_json::to_value(update).expect("serializar UpdateRule")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "un `PATCH` con condición inválida debe rechazarse: {response}"
    );

    // La regla debe conservar sus condiciones originales.
    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        1,
        "la regla debe conservar su condición original; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].collection, "path");
}

/// Escenario: La importación rechaza por entradas las condiciones inválidas y
/// conserva las válidas.
#[tokio::test]
async fn test_import_reject_invalid_conditions() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "rules": [
            {
                "name": "wc-import-invalid",
                "conditions": [
                    condition_input_json("request_cookies", None, "^abc$"),
                ],
            },
            {
                "name": "wc-import-valid",
                "conditions": [
                    condition_input_json("path", None, "^/admin$"),
                ],
            }
        ]
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/import", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la importación debe devolver 200: {response}"
    );

    let imported = response["imported"].as_u64().unwrap_or(0);
    let rejected = response["rejected"].as_array().map_or(0, Vec::len);
    assert_eq!(imported, 1, "solo la regla válida debe importarse");
    assert_eq!(rejected, 1, "la regla inválida debe rechazarse");

    // La regla inválida no debe existir.
    let count_invalid: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("wc-import-invalid")
        .fetch_one(&pool)
        .await
        .expect("contar la regla inválida");
    assert_eq!(count_invalid, 0, "la regla inválida no debe persistirse");

    // La regla válida debe existir con su condición.
    let valid_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("wc-import-valid")
        .fetch_one(&pool)
        .await
        .expect("id de la regla válida");
    let rows = stored_conditions(&pool, valid_id).await;
    assert_eq!(
        rows.len(),
        1,
        "la regla válida debe tener su condición; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].collection, "path");
}

// ---------------------------------------------------------------------------
// F2b-3 (is-tor-collection) — la columna `is_tor` y el operador `@truthy`
// ---------------------------------------------------------------------------
//
// Contrato verificado:
// - La columna `is_tor` se proyecta a una fila `is_tor`/`@truthy` al final.
// - La autoría avanzada acepta la condición `is_tor`/`@truthy`.
// - La lectura reconstruye el operador de la columna `operator`.
// - Una fila incoherente con su colección se descarta con diagnóstico.
// - La proyección desde la tabla equivale a la de las columnas.
// - La migración de backfill rellena las reglas `is_tor = 1` y es idempotente.

/// Inserta por SQL crudo una regla «legacy» y devuelve su `id`, sin pasar por la
/// API (que en RED todavía no proyecta `is_tor`).
async fn insert_legacy_rule(pool: &SqlitePool, name: &str, is_tor: i32, path: Option<&str>) -> i32 {
    sqlx::query(
        "INSERT INTO rules
            (name, description, weight, mode, pipeline, allow, is_tor, path, active, created_at, updated_at)
         VALUES (?, '', 100, 'enforce', 'waf', 0, ?, ?, 1, datetime('now'), datetime('now'))",
    )
    .bind(name)
    .bind(is_tor)
    .bind(path)
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("insertar la regla legacy {name}: {error}"));
    sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("id de la regla legacy {name}: {error}"))
}

/// Inserta por SQL crudo una fila de `rule_conditions`.
async fn insert_raw_condition(
    pool: &SqlitePool,
    rule_id: i32,
    position: i32,
    collection: &str,
    transformations: &str,
    operator: &str,
    pattern: &str,
) {
    sqlx::query(
        "INSERT INTO rule_conditions
            (rule_id, position, collection, key, transformations, operator, pattern, negate)
         VALUES (?, ?, ?, NULL, ?, ?, ?, 0)",
    )
    .bind(rule_id)
    .bind(position)
    .bind(collection)
    .bind(transformations)
    .bind(operator)
    .bind(pattern)
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("insertar la fila cruda {collection}/{operator}: {error}"));
}

/// Escenario (rules/spec): «La columna `is_tor` genera su fila».
///
/// GIVEN una regla con `path` restringido y `is_tor = Some(true)`
/// WHEN se crea por la API
/// THEN SHALL existir una fila `is_tor`/`@truthy` con patrón vacío, `key` nula y
///      `negate` 0
/// AND esa fila SHALL ir después de la de `path`.
#[tokio::test]
async fn test_is_tor_column_persists_a_truthy_condition_at_the_end() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("it-column");
    rule.path = Some("^/admin$".to_string());
    rule.is_tor = Some(true);
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con `is_tor`");

    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        2,
        "`path` más `is_tor` deben persistir dos filas; almacenadas: {rows:?}"
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
        "la condición de `path` va primero"
    );
    assert_eq!(
        rows[1],
        StoredCondition {
            position: 1,
            collection: "is_tor".to_string(),
            key: None,
            transformations: String::new(),
            operator: "@truthy".to_string(),
            pattern: String::new(),
            negate: 0,
        },
        "la fila `is_tor`/`@truthy` va después, con patrón vacío y `key` nula"
    );
}

/// Escenario (rules/spec): «Una condición de `is_tor` con `@truthy` se acepta».
#[tokio::test]
async fn test_advanced_rule_accepts_is_tor_truthy() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "it-advanced",
        "conditions": [{
            "collection": "is_tor",
            "key": null,
            "transformations": "",
            "operator": "@truthy",
            "pattern": "",
        }],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "la condición `is_tor`/`@truthy` debe aceptarse; respuesta: {response}"
    );

    let rule_id: i32 = sqlx::query_scalar("SELECT id FROM rules WHERE name = ?")
        .bind("it-advanced")
        .fetch_one(&pool)
        .await
        .expect("id de it-advanced");
    let rows = stored_conditions(&pool, rule_id).await;
    assert_eq!(
        rows.len(),
        1,
        "la condición debe persistirse; almacenadas: {rows:?}"
    );
    assert_eq!(rows[0].collection, "is_tor");
    assert_eq!(
        rows[0].operator, "@truthy",
        "el operador persistido debe ser `@truthy`"
    );
    assert_eq!(rows[0].pattern, "", "el patrón debe quedar vacío");
}

/// Escenario (rules/spec): el round-trip del operador en `rule_conditions`.
///
/// GIVEN una regla con una fila `@rx` y otra `@truthy` escritas en la tabla
/// WHEN se lee por la vía de producción (`read_all_active`)
/// THEN la lectura SHALL reconstruir el operador de cada fila.
#[tokio::test]
async fn test_reading_reconstructs_the_operator_from_the_column() {
    let pool = create_migrated_pool().await;
    let rule_id = insert_legacy_rule(&pool, "it-operator", 0, None).await;

    insert_raw_condition(&pool, rule_id, 0, "path", "", "@rx", "^/admin$").await;
    insert_raw_condition(&pool, rule_id, 1, "is_tor", "", "@truthy", "").await;

    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == rule_id)
        .expect("la regla debe estar en la caché");

    assert_eq!(
        observable(&cached.conditions),
        vec![
            ("path".to_string(), "^/admin$".to_string()),
            ("is_tor".to_string(), "@truthy".to_string()),
        ],
        "la lectura debe reconstruir el operador de cada fila; obtenidas: {:?}",
        cached.conditions
    );
}

/// Escenario (design.md D6): una fila incoherente se descarta con diagnóstico.
///
/// GIVEN filas con `@truthy` en una colección de texto, `is_tor` con `@rx`, y
///      `@truthy` con transformaciones no vacías
/// WHEN se lee
/// THEN SHALL descartarse todas, conservando solo la fila de control válida.
#[tokio::test]
async fn test_incoherent_condition_rows_are_discarded() {
    let pool = create_migrated_pool().await;
    let rule_id = insert_legacy_rule(&pool, "it-incoherent", 0, None).await;

    // `@truthy` en una colección de texto.
    insert_raw_condition(&pool, rule_id, 0, "path", "", "@truthy", "^/admin$").await;
    // `is_tor` con `@rx`.
    insert_raw_condition(&pool, rule_id, 1, "is_tor", "", "@rx", "true").await;
    // `@truthy` con transformaciones no vacías.
    insert_raw_condition(&pool, rule_id, 2, "is_tor", "lowercase", "@truthy", "").await;
    // Fila de control válida.
    insert_raw_condition(&pool, rule_id, 3, "query", "", "@rx", "union").await;

    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == rule_id)
        .expect("la regla debe estar en la caché");

    assert_eq!(
        observable(&cached.conditions),
        vec![("query".to_string(), "union".to_string())],
        "solo la fila de control debe sobrevivir; obtenidas: {:?}",
        cached.conditions
    );
}

/// Escenario (rules/spec): la proyección desde la tabla equivale a la de las
/// columnas para una regla con `is_tor`.
#[tokio::test]
async fn test_projection_from_table_matches_columns_for_is_tor_rule() {
    let pool = create_migrated_pool().await;

    let mut rule = blank_new_rule("it-equivalence");
    rule.path = Some("^/admin$".to_string());
    rule.is_tor = Some(true);
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla con `is_tor`");

    let pipeline = TransformationPipeline::parse(&created.transformations).unwrap_or_default();
    let expected = serialized(&project(&created, &pipeline));

    let stored: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT collection, operator, pattern FROM rule_conditions WHERE rule_id = ? ORDER BY position ASC",
    )
    .bind(created.id)
    .fetch_all(&pool)
    .await
    .expect("leer `rule_conditions`");

    assert_eq!(
        stored, expected,
        "la proyección desde la tabla debe coincidir con la de las columnas"
    );
    assert!(
        expected
            .iter()
            .any(|(collection, _, _)| collection == "is_tor"),
        "la proyección debe incluir la condición `is_tor`; proyección: {expected:?}"
    );
}

/// Escenario (rules/spec): «La migración rellena las reglas con `is_tor`».
///
/// GIVEN una base con reglas `is_tor = 1` anteriores a esta sub-entrega
/// WHEN se aplica la migración de backfill
/// THEN cada regla SHALL recibir su fila `is_tor`/`@truthy` (patrón vacío, `key`
///      nula, `negate` 0), tras las filas de los campos
/// AND las reglas con `is_tor` falso NO SHALL recibir fila
/// AND reaplicarla SHALL ser idempotente.
#[tokio::test]
async fn test_migration_backfills_is_tor_rules_into_conditions() {
    let (pool, create_sql, is_tor_sql) = migrated_pool_without_conditions_migrations().await;

    let create_sql = create_sql.unwrap_or_else(|| {
        panic!("no existe la migración que crea `rule_conditions` en `migrations/`")
    });
    let is_tor_sql = is_tor_sql.unwrap_or_else(|| {
        panic!(
            "no existe ninguna migración que proyecte `is_tor` a `rule_conditions` en \
             `migrations/`: es el motivo por el que este test falla en la fase RED"
        )
    });

    let tor_id = insert_legacy_rule(&pool, "it-legacy-tor", 1, None).await;
    let no_tor_id = insert_legacy_rule(&pool, "it-legacy-not-tor", 0, None).await;
    let tor_path_id = insert_legacy_rule(&pool, "it-legacy-tor-path", 1, Some("^/admin$")).await;

    // La migración de F2b-1 crea el esquema y rellena los 14 campos.
    sqlx::raw_sql(&create_sql)
        .execute(&pool)
        .await
        .expect("aplicar la creación de `rule_conditions`");
    // El backfill de `is_tor` (F2b-3), dos veces: la segunda debe ser idempotente.
    sqlx::raw_sql(&is_tor_sql)
        .execute(&pool)
        .await
        .expect("aplicar el backfill de `is_tor`");
    sqlx::raw_sql(&is_tor_sql)
        .execute(&pool)
        .await
        .expect("reaplicar el backfill de `is_tor` (idempotencia)");

    let tor_rows = stored_conditions(&pool, tor_id).await;
    assert_eq!(
        tor_rows.len(),
        1,
        "una regla `is_tor = 1` sin campos debe recibir una única fila `is_tor`; \
         almacenadas: {tor_rows:?}"
    );
    assert_eq!(
        tor_rows[0],
        StoredCondition {
            position: 0,
            collection: "is_tor".to_string(),
            key: None,
            transformations: String::new(),
            operator: "@truthy".to_string(),
            pattern: String::new(),
            negate: 0,
        },
        "la fila debe ser `is_tor`/`@truthy` con patrón vacío, `key` nula y `negate` 0"
    );

    assert_eq!(
        stored_conditions_count(&pool, no_tor_id).await,
        0,
        "una regla `is_tor = 0` NO debe recibir ninguna fila"
    );

    let tor_path_rows = stored_conditions(&pool, tor_path_id).await;
    assert_eq!(
        tor_path_rows.len(),
        2,
        "una regla con `path` e `is_tor` debe quedar con dos filas (idempotente); \
         almacenadas: {tor_path_rows:?}"
    );
    assert_eq!(tor_path_rows[0].collection, "path");
    assert_eq!(
        (
            tor_path_rows[1].position,
            tor_path_rows[1].collection.as_str(),
            tor_path_rows[1].operator.as_str(),
        ),
        (1, "is_tor", "@truthy"),
        "la fila de `is_tor` va después de las de los campos"
    );
}

// ---------------------------------------------------------------------------
// Remediación de la auditoría de F2b-3
// ---------------------------------------------------------------------------
//
// MEDIO-1: en modo avanzado la columna `is_tor` NO participa fuera del conjunto
// (requisito de `rule-matching`). BAJO-1: los 400 de la validación del operador
// vistos por HTTP.

/// Un `NewRequest` mínimo con el `path` y el booleano Tor indicados.
fn request_with_path_and_tor(path: Option<&str>, is_tor: Option<bool>) -> NewRequest {
    let mut request = NewRequest::from_request(&axum::http::HeaderMap::new(), None, None);
    request.path = path.map(str::to_string);
    request.is_tor = is_tor;
    request
}

/// Hallazgo MEDIO-1: en modo avanzado la columna `is_tor` no participa del
/// matcheo.
///
/// GIVEN una regla creada con `conditions` que NO incluyen `is_tor` y con la
///       columna `is_tor = 1`
/// WHEN se evalúa con el conjunto persistido (`read_all_active`)
/// THEN NO SHALL casar una petición Tor cuyas condiciones no casan: la columna no
///      aporta ningún predicado al conjunto
/// AND SHALL casar una petición **no-Tor** cuyas condiciones sí casan: la columna
///     deja de restringir el tráfico a Tor, conforme al requisito de
///     `rule-matching`.
#[tokio::test]
async fn test_advanced_mode_ignores_the_is_tor_column() {
    let pool = create_migrated_pool().await;

    // Regla avanzada: la lista NO incluye `is_tor`, pero la columna vale 1. Los 14
    // campos (aquí `path`) se almacenan pero NO participan del matcheo.
    let mut rule = blank_new_rule("it-advanced-column-ignored");
    rule.is_tor = Some(true);
    rule.path = Some("^/ignored$".to_string());
    rule.conditions = Some(vec![ConditionInput {
        collection: "path".to_string(),
        key: None,
        transformations: String::new(),
        operator: None,
        pattern: "^/admin$".to_string(),
    }]);
    let created = Rule::create(&pool, rule)
        .await
        .expect("crear la regla avanzada");

    // La tabla contiene exactamente la condición de la lista: la columna no añade
    // una fila `is_tor`.
    let rows = stored_conditions(&pool, created.id).await;
    assert_eq!(
        rows.len(),
        1,
        "el conjunto avanzado no debe ampliarse con la columna `is_tor`; almacenadas: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| row.collection != "is_tor"),
        "no debe persistirse una condición `is_tor` desde la columna; almacenadas: {rows:?}"
    );

    // El matcheo se alimenta de la tabla (`read_all_active`), no de la proyección
    // de los campos: es la vía de producción.
    let cache = CacheRule::read_all_active(&pool)
        .await
        .expect("leer las reglas activas");
    let cached = cache
        .iter()
        .find(|cache_rule| cache_rule.rule.id == created.id)
        .expect("la regla creada debe estar en la caché");

    // Una petición Tor cuyas condiciones no casan (`/other` ≠ `^/admin$`) NO casa:
    // la columna `is_tor = 1` no aporta ningún predicado que la haga casar.
    assert!(
        !cached.matches(&request_with_path_and_tor(Some("/other"), Some(true))),
        "la columna `is_tor = 1` no debe hacer casar una regla avanzada cuyas \
         condiciones no casan"
    );

    // Una petición NO-Tor cuyas condiciones sí casan: la columna no restringe a
    // Tor, así que la regla casa.
    assert!(
        cached.matches(&request_with_path_and_tor(Some("/admin"), Some(false))),
        "en modo avanzado la columna `is_tor` no debe restringir el tráfico a Tor"
    );

    // Control: la misma petición marcada como Tor casa igual, por las condiciones.
    assert!(
        cached.matches(&request_with_path_and_tor(Some("/admin"), Some(true))),
        "con las condiciones casando, la regla casa sea o no Tor"
    );
}

/// Hallazgo BAJO-1: `@truthy` en una colección de texto se rechaza con 400.
#[tokio::test]
async fn test_http_create_rejects_truthy_on_text_collection() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "it-http-truthy-on-text",
        "conditions": [{
            "collection": "path",
            "key": null,
            "transformations": "",
            "operator": "@truthy",
            "pattern": "^/admin$",
        }],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "`@truthy` en una colección de texto debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("it-http-truthy-on-text")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

/// Hallazgo BAJO-1: `@rx` sobre la colección booleana `is_tor` se rechaza con 400.
#[tokio::test]
async fn test_http_create_rejects_rx_on_is_tor() {
    let pool = create_migrated_pool().await;
    let state = build_app_state(pool.clone());

    let payload = json!({
        "name": "it-http-rx-on-is-tor",
        "conditions": [{
            "collection": "is_tor",
            "key": null,
            "transformations": "",
            "operator": "@rx",
            "pattern": "true",
        }],
    });

    let (status, response) = call_rules_api(&state, Method::POST, "/", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "`@rx` sobre `is_tor` debe rechazarse con 400; respuesta: {response}"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM rules WHERE name = ?")
        .bind("it-http-rx-on-is-tor")
        .fetch_one(&pool)
        .await
        .expect("contar reglas");
    assert_eq!(count, 0, "la regla rechazada NO debe almacenarse");
}

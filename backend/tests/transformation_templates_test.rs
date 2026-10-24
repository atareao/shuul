//! Tests de integración (fase RED) del change `waf-transformations`, delta
//! `templates`.
//!
//! Spec: `openspec/changes/waf-transformations/specs/templates/spec.md`
//! → «Una plantilla declara su pipeline de transformaciones».
//!
//! Cubre sus cuatro escenarios —«El endpoint sirve la pipeline de cada
//! plantilla», «Aplicar una plantilla traslada su pipeline», «La plantilla de
//! detección casa una evasión codificada» y «Las plantillas de Jail no declaran
//! pipeline»— y la tarea 4.3 del change (la severidad de las cuatro plantillas
//! de detección).
//!
//! El endpoint se conduce con el harness de `tests/templates_listing_test.rs`
//! (`template_router()` + `tower::util::ServiceExt::oneshot`), que ejercita la
//! serialización **real** del handler; el handler ignora la base de datos, así
//! que vale un pool en memoria sin esquema.
//!
//! ## Estado esperado en RED
//!
//! El andamiaje del change ya añade el campo `transformations` a `RuleTemplate`
//! con la cadena vacía en **todas** las plantillas, así que:
//!
//! - el escenario del endpoint y el de las plantillas de Jail **pasan** —el
//!   campo existe, el handler lo serializa solo y vale la cadena vacía—;
//! - la tabla de pipelines de las cuatro plantillas de detección, el matcheo de
//!   la evasión codificada y la severidad nueva de la tarea 4.3 **fallan por
//!   aserción**: son el comportamiento de producción que implementa la ronda
//!   GREEN siguiente.
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::template_router;
use backend::models::{
    AppState, BanManager, CacheRule, GeoIpService, NewRequest, RateLimiter, Rule, Settings,
    StatsCollector, TorService,
};
use backend::templates::{RuleTemplate, all_rule_templates};
use serde_json::Value;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

// ---------------------------------------------------------------------------
// Tablas del spec
// ---------------------------------------------------------------------------

/// La tabla exacta del delta `templates`: las cuatro plantillas de detección,
/// el filtro que restringen y la pipeline que necesitan para alcanzar las
/// evasiones codificadas.
const DETECTION_TEMPLATES: [(&str, &str, &str); 4] = [
    ("Path traversal probes", "path", "urlDecodeUni,removeNulls"),
    (
        "SQL injection probes",
        "query",
        "urlDecodeUni,replaceNulls,compressWhitespace",
    ),
    (
        "XSS probes",
        "query",
        "urlDecodeUni,htmlEntityDecode,removeNulls",
    ),
    (
        "Command injection probes",
        "query",
        "urlDecodeUni,removeNulls",
    ),
];

/// La severidad que fija la tarea 4.3 del change: `SQL injection probes` y
/// `Command injection probes` suben a `critico`, `XSS probes` a `alto` y
/// `Path traversal probes` se queda en `alto` —`../` es señal de intento, no
/// vulnerabilidad explotable por sí sola—.
const DETECTION_SEVERITIES: [(&str, &str); 4] = [
    ("SQL injection probes", "critico"),
    ("Command injection probes", "critico"),
    ("XSS probes", "alto"),
    ("Path traversal probes", "alto"),
];

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Construye un `AppState` mínimo en memoria. El handler de plantillas ignora
/// la base de datos, así que un pool sin esquema es suficiente.
async fn build_app_state() -> Arc<AppState> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("url sqlite válida")
        .create_if_missing(true);
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria");

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

/// Conduce el handler real de plantillas y devuelve el cuerpo JSON parseado.
async fn get_templates_json() -> Value {
    let router = template_router().with_state(build_app_state().await);
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .body(Body::empty())
                .expect("petición"),
        )
        .await
        .expect("respuesta del handler");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("leer el cuerpo");
    serde_json::from_slice(&bytes).expect("cuerpo JSON válido")
}

/// La plantilla del catálogo con ese nombre, o un pánico que la nombra.
fn template_named(name: &str) -> RuleTemplate {
    all_rule_templates()
        .into_iter()
        .find(|template| template.name == name)
        .unwrap_or_else(|| panic!("la plantilla `{name}` no está en el catálogo"))
}

/// Construye una [`Rule`] con los campos de una plantilla del catálogo.
///
/// El `path`, la `query`, la especificación de transformaciones y el `allow`
/// salen de la plantilla; el modo se fija a `enforce` y el resto de los campos
/// de matcheo a los valores de la plantilla, de modo que la regla decide
/// exactamente igual que la plantilla aplicada.
fn rule_from_template(template: &RuleTemplate) -> Rule {
    Rule {
        id: 1,
        name: template.name.clone(),
        description: template.description.clone(),
        weight: template.weight,
        mode: "enforce".to_string(),
        pipeline: template.pipeline.clone(),
        transformations: template.transformations.clone(),
        allow: template.allow,
        is_tor: template.is_tor,
        ip_address: template.ip_address.clone(),
        protocol: template.protocol.clone(),
        fqdn: template.fqdn.clone(),
        path: template.path.clone(),
        query: template.query.clone(),
        city_name: template.city_name.clone(),
        country_name: template.country_name.clone(),
        country_code: template.country_code.clone(),
        user_agent: template.user_agent.clone(),
        method: template.method.clone(),
        referer: template.referer.clone(),
        content_type: template.content_type.clone(),
        accept_language: template.accept_language.clone(),
        x_request_id: template.x_request_id.clone(),
        rate_limit_profile_id: template.rate_limit_profile_id,
        rate_limit_profile_name: template.rate_limit_profile_name.clone(),
        active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

// ---------------------------------------------------------------------------
// Escenario: El endpoint sirve la pipeline de cada plantilla
// ---------------------------------------------------------------------------

/// Escenario: «El endpoint sirve la pipeline de cada plantilla».
///
/// GIVEN una petición a `GET /api/v1/templates`
/// WHEN el handler responde
/// THEN cada plantilla de `waf` y de `jail` SHALL llevar el campo
/// `transformations`
///
/// El campo puede valer la cadena vacía —es el valor por defecto del delta—,
/// pero tiene que **estar** en cada entrada de las dos listas: es lo que el
/// cliente necesita para trasladarlo a la regla creada desde la plantilla.
#[tokio::test]
async fn el_endpoint_sirve_la_especificacion_de_cada_plantilla() {
    let json = get_templates_json().await;

    let data = json
        .get("data")
        .and_then(Value::as_object)
        .expect("la respuesta debe llevar un objeto `data`");

    for key in ["waf", "jail"] {
        let templates = data
            .get(key)
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("la respuesta debe llevar la lista `{key}`: {json}"));

        assert!(
            !templates.is_empty(),
            "la lista `{key}` no puede venir vacía: el catálogo tiene plantillas"
        );

        for template in templates {
            let name = template
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<sin nombre>");
            let spec = template
                .get("transformations")
                .unwrap_or_else(|| panic!("la plantilla `{name}` no lleva `transformations`"));

            assert!(
                spec.is_string(),
                "`transformations` de `{name}` SHALL ser una cadena (puede ser la vacía); \
                 obtenido: {spec}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Escenario: Aplicar una plantilla traslada su pipeline
// ---------------------------------------------------------------------------

/// Escenario: «Aplicar una plantilla traslada su pipeline».
///
/// GIVEN la plantilla de cada fila de la tabla del delta `templates`
/// WHEN se aplica para crear una regla
/// THEN la regla creada SHALL declarar la pipeline de esa fila
///
/// Y además cada una SHALL restringir el filtro que la tabla dice —`path` o
/// `query`—: una pipeline sin el filtro correspondiente no serviría de nada,
/// porque no habría valor que transformar.
#[test]
fn las_cuatro_plantillas_de_deteccion_declaran_su_pipeline() {
    for (name, field, pipeline) in DETECTION_TEMPLATES {
        let template = template_named(name);

        assert_eq!(
            template.transformations, pipeline,
            "la plantilla `{name}` SHALL declarar la pipeline `{pipeline}`; \
             obtenido: `{}`",
            template.transformations
        );

        let filter = match field {
            "path" => template.path.as_deref(),
            "query" => template.query.as_deref(),
            other => panic!(
                "la tabla del spec solo declara los filtros `path` y `query`; obtenido: `{other}`"
            ),
        };
        assert!(
            filter.is_some(),
            "la plantilla `{name}` SHALL restringir el filtro `{field}`: una pipeline sin el \
             filtro correspondiente no tiene valor que transformar"
        );
    }
}

// ---------------------------------------------------------------------------
// Escenario: Las plantillas de Jail no declaran pipeline
// ---------------------------------------------------------------------------

/// Escenario: «Las plantillas de Jail no declaran pipeline».
///
/// GIVEN el catálogo de plantillas
/// WHEN se recorren las plantillas cuyo pipeline es `jail`
/// THEN todas SHALL declarar la cadena vacía como especificación
///
/// El Jail decide sobre el status code de la respuesta, no sobre un valor de la
/// petición: una pipeline ahí no tendría nada que transformar.
#[test]
fn las_plantillas_de_jail_no_declaran_pipeline() {
    let jail: Vec<RuleTemplate> = all_rule_templates()
        .into_iter()
        .filter(|template| template.pipeline == "jail")
        .collect();

    assert!(
        !jail.is_empty(),
        "el catálogo debe tener plantillas de Jail que comprobar"
    );

    for template in &jail {
        assert!(
            template.transformations.is_empty(),
            "la plantilla de Jail `{}` NO SHALL declarar pipeline; obtenido: `{}`",
            template.name,
            template.transformations
        );
    }
}

// ---------------------------------------------------------------------------
// Escenario: La plantilla de detección casa una evasión codificada
// ---------------------------------------------------------------------------

/// Escenario: «La plantilla de detección casa una evasión codificada».
///
/// GIVEN una regla creada desde `Path traversal probes`
/// WHEN se evalúa contra una petición cuyo `path` es `%2e%2e%2f`
/// THEN la regla SHALL casar
///
/// La petición se construye con [`NewRequest::from_request`] y la cabecera
/// `x-forwarded-uri: /%2e%2e%2f`, que es el camino real: `http::Uri` entrega el
/// *request-target* **codificado** (tarea 1.13), así que el patrón `\.\./` solo
/// lo alcanza la pipeline de la plantilla.
///
/// El segundo aserto es el **control** que hace el test no vacuo: la misma
/// regla con la pipeline vacía NO casa, porque el valor crudo `%2e%2e%2f` no
/// contiene `../`.
#[test]
fn la_plantilla_de_path_traversal_casa_una_evasion_codificada() {
    let template = template_named("Path traversal probes");

    let mut headers = http::HeaderMap::new();
    headers.insert(
        "x-forwarded-uri",
        http::HeaderValue::from_static("/%2e%2e%2f"),
    );
    let request = NewRequest::from_request(&headers, None, None);

    let rule = rule_from_template(&template);
    let outcome = CacheRule::from_rule(&rule).evaluate(&request);

    assert!(
        outcome.matched,
        "la regla creada desde `Path traversal probes` SHALL casar un `path` `%2e%2e%2f`: \
         su pipeline `{}` tiene que decodificarlo antes de compararlo con `{}`",
        rule.transformations,
        rule.path.as_deref().unwrap_or("<sin path>")
    );

    // Control: sin pipeline el valor crudo no contiene `../` y la regla no casa.
    let mut without_pipeline = rule;
    without_pipeline.transformations = String::new();
    let control = CacheRule::from_rule(&without_pipeline).evaluate(&request);

    assert!(
        !control.matched,
        "el control SHALL NO casar: con la pipeline vacía el valor `%2e%2e%2f` no contiene `../`, \
         que es justo el agujero que la pipeline de la plantilla cierra"
    );
}

// ---------------------------------------------------------------------------
// Tarea 4.3 — La severidad de las cuatro plantillas de detección
// ---------------------------------------------------------------------------

/// Escenario: la severidad que fija la tarea 4.3 del change.
///
/// `SQL injection probes` y `Command injection probes` suben de `alto` a
/// `critico`, `XSS probes` de `medio` a `alto`, y `Path traversal probes` se
/// queda en `alto`.
///
/// Es el **único** sitio donde la tarea 4.3 queda verificada: la severidad es
/// un dato del catálogo, no un comportamiento que otro test pueda observar.
#[test]
fn las_cuatro_plantillas_de_deteccion_suben_su_severidad() {
    for (name, severity) in DETECTION_SEVERITIES {
        let template = template_named(name);

        assert_eq!(
            template.severity, severity,
            "la tarea 4.3 fija la severidad de `{name}` en `{severity}`; \
             obtenido: `{}`",
            template.severity
        );
    }
}

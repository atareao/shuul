//! API test: `GET /api/v1/templates` serves only rule templates.
//!
//! Spec: `openspec/changes/templates-listing/specs/templates/spec.md`
//! → Requirement «The templates endpoint no longer serves rate limit profiles».
//!
//! The response MUST include the `waf` and `jail` keys and MUST NOT include a
//! `profiles` key. This test drives the real `template_router` handler through
//! `tower::ServiceExt::oneshot`, the same HTTP harness used by
//! `tests/report_per_rule_ban_test.rs`, so it exercises the exact serialization
//! the production handler emits (not a copy of it).
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::template_router;
use backend::models::{
    AppState, BanManager, GeoIpService, RateLimiter, Settings, StatsCollector, TorService,
};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

/// Build a minimal in-memory `AppState`. The templates handler ignores the
/// database entirely, so an empty schema-less pool is enough.
async fn build_app_state() -> Arc<AppState> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("valid sqlite url")
        .create_if_missing(true);
    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("in-memory pool");

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

/// Drive the real templates handler and return the parsed JSON body.
async fn get_templates_json() -> serde_json::Value {
    let router = template_router().with_state(build_app_state().await);
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("handler response");

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

/// Scenario: Response contains only rule templates.
#[tokio::test]
async fn templates_response_has_waf_and_jail_but_no_profiles() {
    let json = get_templates_json().await;

    let data = json
        .get("data")
        .and_then(serde_json::Value::as_object)
        .expect("response has an object `data`");

    assert!(
        data.contains_key("waf"),
        "response MUST include the `waf` key; got keys: {:?}",
        data.keys().collect::<Vec<_>>()
    );
    assert!(
        data.contains_key("jail"),
        "response MUST include the `jail` key; got keys: {:?}",
        data.keys().collect::<Vec<_>>()
    );
    assert!(
        !data.contains_key("profiles"),
        "response MUST NOT include the `profiles` key; got keys: {:?}",
        data.keys().collect::<Vec<_>>()
    );
}

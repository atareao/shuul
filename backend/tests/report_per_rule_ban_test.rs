//! Integration tests: per-rule ban check in the Jail pipeline.
//!
//! `report_handler` iterates over EVERY matched Jail rule (fail2ban-style).
//! A pre-existing ban for rule A must NOT block the processing of rule B.
//! This verifies the fix where `is_banned(&ip)` (checks ANY active ban)
//! was replaced by `is_banned_for_rule(&ip, Some(rule_id))` (checks the
//! ban for the specific rule being processed).
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use backend::http::report_router;
use backend::models::{
    AppState, BanManager, CacheRule, GeoIpService, PendingBan, RateLimiter, Settings,
    StatsCollector, TorService,
};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

/// Base report payload used across scenarios.
const RT_PAYLOAD: &str =
    r#"{"ip_address":"1.2.3.4","status_code":401,"path":"/admin","method":"GET"}"#;

/// Build an in-memory `SQLite` pool with the minimal schema needed by
/// `report_handler`: `rate_limit_profiles`, `rules` and `bans`.
async fn create_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rate_limit_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            max_retry INTEGER NOT NULL DEFAULT 5,
            find_time_seconds INTEGER NOT NULL DEFAULT 600,
            ban_time_seconds INTEGER NOT NULL DEFAULT 3600,
            bantime_increment INTEGER NOT NULL DEFAULT 0,
            bantime_multipliers TEXT NOT NULL DEFAULT '[1,2,4,8]',
            bantime_maxtime_seconds INTEGER NOT NULL DEFAULT 604800,
            ban_count_decay_days INTEGER NOT NULL DEFAULT 30,
            fail_codes TEXT NOT NULL DEFAULT '[401,403,404]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            weight INTEGER NOT NULL DEFAULT 100,
            mode TEXT NOT NULL DEFAULT 'log_only',
            pipeline TEXT NOT NULL DEFAULT 'waf',
            allow INTEGER NOT NULL DEFAULT 1,
            is_tor INTEGER NOT NULL DEFAULT 0,
            ip_address TEXT, protocol TEXT, fqdn TEXT, path TEXT, query TEXT,
            city_name TEXT, country_name TEXT, country_code TEXT,
            user_agent TEXT, method TEXT, referer TEXT, content_type TEXT,
            accept_language TEXT, x_request_id TEXT,
            rate_limit_profile_id INTEGER REFERENCES rate_limit_profiles(id) ON DELETE SET NULL,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bans (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ip_address TEXT NOT NULL,
            rule_id INTEGER REFERENCES rules(id) ON DELETE SET NULL,
            reason TEXT NOT NULL DEFAULT '',
            banned_at TEXT NOT NULL,
            ban_duration_seconds INTEGER NOT NULL,
            escalation_level INTEGER NOT NULL DEFAULT 0,
            expired INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

/// Insert a rate limit profile that bans on the first `401` report.
async fn insert_profile(pool: &SqlitePool) -> i32 {
    sqlx::query(
        "INSERT INTO rate_limit_profiles
            (name, description, max_retry, find_time_seconds, ban_time_seconds,
             bantime_increment, bantime_multipliers, bantime_maxtime_seconds,
             ban_count_decay_days, fail_codes, created_at, updated_at)
         VALUES (?, '', 1, 60, 3600, 0, '[1]', 3600, 30, '[401]', datetime('now'), datetime('now'))",
    )
    .bind("test_profile")
    .execute(pool)
    .await
    .unwrap();
    // find the id of the inserted row
    sqlx::query_scalar("SELECT id FROM rate_limit_profiles WHERE name = 'test_profile'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Insert a Jail rule pointing at `profile_id`, matching requests to `^/admin`.
async fn insert_jail_rule(pool: &SqlitePool, id: i32, name: &str, weight: i32, profile_id: i32) {
    sqlx::query(
        "INSERT INTO rules
            (id, name, description, weight, mode, pipeline, allow,
             path, rate_limit_profile_id, active, created_at, updated_at)
         VALUES (?, ?, '', ?, 'enforce', 'jail', 0, '^/admin', ?, 1, datetime('now'), datetime('now'))",
    )
    .bind(id)
    .bind(name)
    .bind(weight)
    .bind(profile_id)
    .execute(pool)
    .await
    .unwrap();
}

/// Build a full `AppState` configured for the Jail pipeline test.
async fn build_app_state(pool: SqlitePool, pre_banned_rule: Option<i32>) -> (Arc<AppState>, i32) {
    let profile_id = insert_profile(&pool).await;

    // Two Jail rules: rule_a (id=1) and rule_b (id=3).
    insert_jail_rule(&pool, 1, "rule_a", 10, profile_id).await;
    insert_jail_rule(&pool, 3, "rule_b", 20, profile_id).await;

    // Load rules into the in-memory cache exactly as main.rs does.
    let rules = CacheRule::read_all_active(&pool).await.unwrap();

    // Seed a pre-existing pending ban if requested.
    let mut ban_manager = BanManager::new(3600, false, vec![1], 86400, 30);
    let mut pending_bans = Vec::new();
    if let Some(rule_id) = pre_banned_rule {
        let ip = std::net::IpAddr::from_str("1.2.3.4").unwrap();
        pending_bans.push(PendingBan {
            ip,
            rule_id,
            reason: "pre-ban".to_string(),
            ban_duration_seconds: None,
            escalation_level: 0,
            created_at: std::time::Instant::now(),
        });
    }

    let app_state = AppState {
        pool,
        secret: "test-secret".to_string(),
        geoip: GeoIpService::new(
            maxminddb::Reader::open_readfile("geo/GeoLite2-City.mmdb").unwrap(),
        ),
        rules: Mutex::new(rules),
        stats: StatsCollector::new(),
        static_dir: ".".to_string(),
        ban_manager: Mutex::new(ban_manager),
        rate_limiter: Mutex::new(HashMap::<i32, RateLimiter>::new()),
        pending_bans: Mutex::new(pending_bans),
        tor_service: TorService::new(),
        settings: Mutex::new(Settings::default()),
        oidc_metadata: tokio::sync::RwLock::new(None),
        jwt_validator: tokio::sync::RwLock::new(None),
        oidc_states: tokio::sync::Mutex::new(HashMap::new()),
        oidc_client_id: None,
        oidc_redirect_url: None,
    };

    (Arc::new(app_state), profile_id)
}

/// POST a report payload to the report router and return the status.
async fn post_report(app_state: Arc<AppState>) -> StatusCode {
    let router = report_router().with_state(app_state);
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(RT_PAYLOAD))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

/// Whether the IP has a pending ban for the given rule id.
fn has_pending_ban(state: &AppState, rule_id: i32) -> bool {
    let ip = std::net::IpAddr::from_str("1.2.3.4").unwrap();
    state
        .pending_bans
        .lock()
        .is_ok_and(|pb| pb.iter().any(|b| b.ip == ip && b.rule_id == rule_id))
}

// ---------------------------------------------------------------------
// Scenario 1: IP banned by rule A allows a ban for rule B
// ---------------------------------------------------------------------

#[tokio::test]
async fn banned_for_rule_a_does_not_block_rule_b() {
    let pool = create_pool().await;
    let (state, _profile_id) = build_app_state(pool, Some(1)).await;

    // Pre-state: IP has a pending ban for rule_a (id=1), NOT for rule_b (id=3).
    assert!(has_pending_ban(&state, 1));
    assert!(!has_pending_ban(&state, 3));

    // The report matches BOTH rule_a and rule_b. With the old `is_banned()`
    // check, the pre-existing rule_a ban would also block rule_b.
    let status = post_report(state.clone()).await;
    assert_eq!(status, StatusCode::OK);

    // Rule_a stays banned (it was pre-banned). Rule_b MUST now be processed
    // and result in its own pending ban. This is the core of the bug fix.
    assert!(
        has_pending_ban(&state, 3),
        "rule_b should be processed despite the rule_a ban"
    );
    assert!(has_pending_ban(&state, 1));
}

// ---------------------------------------------------------------------
// Scenario 2: IP banned for a rule does not re-process the same rule
// ---------------------------------------------------------------------

#[tokio::test]
async fn banned_for_rule_skips_same_rule() {
    let pool = create_pool().await;
    let (state, _profile_id) = build_app_state(pool, Some(3)).await;

    // Pre-state: IP has a pending ban for rule_b (id=3) only.
    assert!(has_pending_ban(&state, 3));
    assert!(!has_pending_ban(&state, 1));

    let status = post_report(state.clone()).await;
    assert_eq!(status, StatusCode::OK);

    // `is_banned_for_rule(&ip, Some(3))` returns true → the rule is skipped,
    // so it does not get a fresh pending ban triggered by this report.
    // The pending ban for rule_b must still be present.
    assert!(
        has_pending_ban(&state, 3),
        "same-rule pending ban should remain"
    );
}

// ---------------------------------------------------------------------
// Scenario 3: an unbanned IP processes ALL matching rules
// ---------------------------------------------------------------------

#[tokio::test]
async fn unbanned_ip_processes_all_rules() {
    let pool = create_pool().await;
    let (state, _profile_id) = build_app_state(pool, None).await;

    // Pre-state: IP has no pending bans at all.
    assert!(!has_pending_ban(&state, 1));
    assert!(!has_pending_ban(&state, 3));

    let status = post_report(state.clone()).await;
    assert_eq!(status, StatusCode::OK);

    // Both rules are evaluated independently → both result in a pending ban.
    assert!(has_pending_ban(&state, 1), "rule_a should process normally");
    assert!(has_pending_ban(&state, 3), "rule_b should process normally");
}

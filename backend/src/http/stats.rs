//! # Endpoints de estadísticas
//!
//! Consulta de estadísticas agregadas desde `StatsCollector` (en memoria):
//! evolución temporal, top reglas, top países e información general.

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Data};
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;

pub fn stats_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(read_info_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/top_countries", routing::get(read_top_countries))
        .route("/top_rules", routing::get(read_top_rules))
        .route("/top_methods", routing::get(read_top_methods))
        .route("/top_paths", routing::get(read_top_paths))
        .route("/top_fqdns", routing::get(read_top_fqdns))
        .route("/evolution", routing::get(read_evolution))
        .route(
            "/evolution_by_method",
            routing::get(read_evolution_by_method),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        BanManager, CacheRule, GeoIpService, Rule, Settings, StatsCollector, TorService,
    };
    use axum::body::to_bytes;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use maxminddb::Reader;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_read_top_rules_returns_names() {
        // Create an in-memory SQLite pool (required by AppState)
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // --- Setup StatsCollector with some blocked requests ---
        let stats = StatsCollector::new();
        // Rule id=1 blocked 3 times
        stats.record_blocked(
            Some(1),
            Some("US"),
            Some("GET"),
            Some("/login"),
            Some("example.com"),
        );
        stats.record_blocked(
            Some(1),
            Some("US"),
            Some("GET"),
            Some("/login"),
            Some("example.com"),
        );
        stats.record_blocked(
            Some(1),
            Some("US"),
            Some("GET"),
            Some("/login"),
            Some("example.com"),
        );
        // Rule id=2 blocked 2 times
        stats.record_blocked(
            Some(2),
            Some("FR"),
            Some("GET"),
            Some("/admin"),
            Some("mysite.com"),
        );
        stats.record_blocked(
            Some(2),
            Some("FR"),
            Some("GET"),
            Some("/admin"),
            Some("mysite.com"),
        );

        // --- Setup CacheRules with ID→name mapping ---
        let rule1 = Rule {
            id: 1,
            name: "Auth Guard".to_string(),
            description: "Protects auth endpoints".to_string(),
            weight: 10,
            mode: "enforce".to_string(),
            pipeline: "waf".to_string(),
            allow: false,
            is_tor: None,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: Some(r"/login".to_string()),
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
            rate_limit_profile_name: None,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let rule2 = Rule {
            id: 2,
            name: "Path Scanner".to_string(),
            description: "Blocks path scanning".to_string(),
            weight: 20,
            mode: "enforce".to_string(),
            pipeline: "waf".to_string(),
            allow: false,
            is_tor: None,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: Some(r"/admin".to_string()),
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
            rate_limit_profile_name: None,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let cache_rules = vec![CacheRule::from_rule(&rule1), CacheRule::from_rule(&rule2)];

        // --- Minimal supporting fields ---
        let ban_manager = BanManager::new(3600, false, vec![1], 86400, 7);
        let settings = Settings {
            default_rule_mode: "enforce".to_string(),
            log_retention_days: 30,
            log_all_requests: "all".to_string(),
        };

        // GeoIP: try common paths, fall back to /tmp/test.mmdb
        let geoip_paths = [
            "../geo/GeoLite2-City.mmdb",
            "/app/geo/GeoLite2-City.mmdb",
            "geo/GeoLite2-City.mmdb",
            "/tmp/test.mmdb",
        ];
        let mut geoip = None;
        for p in &geoip_paths {
            if let Ok(reader) = Reader::open_readfile(p) {
                geoip = Some(GeoIpService::new(reader));
                break;
            }
        }
        let geoip =
            geoip.expect("No GeoIP database found. Create one with: python3 create_test_mmdb.py");

        let app_state = Arc::new(AppState {
            pool,
            secret: "test-secret".to_string(),
            geoip,
            tor_service: TorService::new(),
            rules: Mutex::new(cache_rules),
            stats,
            static_dir: "static".to_string(),
            ban_manager: Mutex::new(ban_manager),
            rate_limiter: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            pending_bans: Mutex::new(Vec::new()),
            oidc_metadata: tokio::sync::RwLock::new(None),
            jwt_validator: tokio::sync::RwLock::new(None),
            oidc_states: tokio::sync::Mutex::new(HashMap::new()),
            oidc_client_id: None,
            oidc_redirect_url: None,
        });

        // --- Call the handler ---
        let response = read_top_rules(State(app_state))
            .await
            .unwrap()
            .into_response();

        // --- Assertions ---
        assert_eq!(
            response.status(),
            200,
            "read_top_rules should return 200 OK"
        );

        // Parse the JSON body
        let body_bytes = to_bytes(response.into_body(), 1024 * 128)
            .await
            .expect("Failed to read response body");
        let body_json: serde_json::Value =
            serde_json::from_slice(&body_bytes).expect("Failed to parse JSON body");

        // Structure: { "status": 200, "message": "...", "data": [[name, count, pct], ...] }
        let data = body_json["data"]
            .as_array()
            .expect("data should be a JSON array");

        assert!(!data.is_empty(), "top_rules data should not be empty");

        // The rule with id=1 has 3 blocked → should be first (sorted by count desc)
        let first_entry = data[0]
            .as_array()
            .expect("each rule entry should be an array");
        let first_name = first_entry[0]
            .as_str()
            .expect("first tuple element should be a string");

        // THIS ASSERTION WILL FAIL with the current implementation:
        // current code returns rule_id.to_string() ("1"), not the rule name ("Auth Guard")
        assert_eq!(
            first_name, "Auth Guard",
            "First top rule should be 'Auth Guard' (name), not '1' (id)"
        );

        // The rule with id=2 has 2 blocked → should be second
        let second_entry = data[1]
            .as_array()
            .expect("each rule entry should be an array");
        let second_name = second_entry[0]
            .as_str()
            .expect("first tuple element should be a string");

        assert_eq!(
            second_name, "Path Scanner",
            "Second top rule should be 'Path Scanner' (name), not '2' (id)"
        );
    }
}

#[derive(Debug, Deserialize)]
pub struct EvolutionParams {
    pub unit: Option<String>,
    pub last: Option<i32>,
}

/// Returns time‑series evolution data for requests.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, caches, etc.).
///   - `params`: Query parameters containing `unit` (`day|hour|minute`) and `last` (how many periods).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the evolution data or an error message.
#[allow(clippy::cast_sign_loss)]
pub async fn read_evolution(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EvolutionParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Evolution params: {:?}", params);
    let unit = params.unit.as_deref().unwrap_or("day").to_string();
    let mut evolution = app_state.stats.get_evolution(&unit);

    // Apply `last` limit: keep only the last N buckets
    if let Some(last) = params.last.filter(|n| *n > 0) {
        let last = last as usize;
        if evolution.len() > last {
            evolution.drain(..evolution.len() - last);
        }
    }

    // Convert buckets into frontend-friendly series format:
    //   [{"id": "blocked", "data": [{"x": "2024-01-01T00:00:00Z", "y": 5}, ...]},
    //    {"id": "allowed", "data": [{"x": "2024-01-01T00:00:00Z", "y": 10}, ...]}]
    let blocked_series: Vec<serde_json::Value> = evolution
        .iter()
        .map(|bucket| {
            let chrono_dt =
                chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
            serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.blocked})
        })
        .collect();

    let allowed_series: Vec<serde_json::Value> = evolution
        .iter()
        .map(|bucket| {
            let chrono_dt =
                chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
            serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.allowed})
        })
        .collect();

    let result = serde_json::json!([
        {"id": "blocked", "data": blocked_series},
        {"id": "allowed", "data": allowed_series},
    ]);

    debug!("Request evolution: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Request evolution",
        Data::Some(result),
    ))
}

/// Returns the top rules based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top rules or an error.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
pub async fn read_top_rules(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_rules = app_state.stats.get_top_rules();
    let total_blocked = app_state.stats.get_total_blocked();

    // Build a lookup map from rule_id to rule_name while holding the lock.
    // The lock is released before any async operation.
    let rule_names: std::collections::HashMap<i32, String> = {
        let rules = app_state.rules.lock().unwrap();
        rules
            .iter()
            .map(|cr| (cr.rule.id, cr.rule.name.clone()))
            .collect()
    };

    let result: Vec<(String, i32, f32)> = top_rules
        .into_iter()
        .map(|(rule_id, count)| {
            let name = rule_names
                .get(&rule_id)
                .cloned()
                .unwrap_or_else(|| format!("Unknown rule #{rule_id}"));
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (name, count as i32, percentage)
        })
        .collect();
    debug!("Top rules: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top rules",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top countries based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top countries or an error.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
pub async fn read_top_countries(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_countries = app_state.stats.get_top_countries();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_countries
        .into_iter()
        .map(|(country, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (country, count as i32, percentage)
        })
        .collect();
    debug!("Top countries: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top countries",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top HTTP methods based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top methods or an error.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
pub async fn read_top_methods(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_methods = app_state.stats.get_top_methods();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_methods
        .into_iter()
        .map(|(method, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (method, count as i32, percentage)
        })
        .collect();
    debug!("Top methods: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top methods",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top paths based on request count.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the top paths or an error.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
pub async fn read_top_paths(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_paths = app_state.stats.get_top_paths();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_paths
        .into_iter()
        .map(|(path, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (path, count as i32, percentage)
        })
        .collect();
    debug!("Top paths: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top paths",
        Data::Some(serde_json::to_value(result)?),
    ))
}

/// Returns the top FQDNs based on request count.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
pub async fn read_top_fqdns(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let top_fqdns = app_state.stats.get_top_fqdns();
    let total_blocked = app_state.stats.get_total_blocked();
    let result: Vec<(String, i32, f32)> = top_fqdns
        .into_iter()
        .map(|(fqdn, count)| {
            let percentage = if total_blocked > 0 {
                (count as f32 / total_blocked as f32) * 100.0
            } else {
                0.0
            };
            (fqdn, count as i32, percentage)
        })
        .collect();
    debug!("Top FQDNs: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Top FQDNs",
        Data::Some(serde_json::to_value(result)?),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ReadInfoParams {
    pub option: Option<String>,
}

/// Retrieves aggregated information about requests (e.g., total count, per‑rule stats).
///
/// * **Parameters**
///   - `app_state`: Shared state with DB pool.
///   - `params`: Query parameters containing an optional `option` (e.g., "total").
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the requested info or an error.
pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" && opt != "filtered" {
                return Ok(ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'filtered'",
                    Data::None,
                )
                .into_response());
            }
            let info = if opt == "total" {
                app_state.stats.get_total_allowed() + app_state.stats.get_total_blocked()
            } else {
                app_state.stats.get_total_blocked()
            };
            debug!("Request info: {:?}", info);
            Ok(ApiResponse::new(
                StatusCode::OK,
                "Request info",
                Data::Some(serde_json::to_value(info)?),
            )
            .into_response())
        },
        None => Ok(ApiResponse::new(
            StatusCode::BAD_REQUEST,
            "Option parameter is required",
            Data::None,
        )
        .into_response()),
    }
}

/// Returns time-series evolution data per HTTP method.
#[allow(clippy::cast_sign_loss)]
pub async fn read_evolution_by_method(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EvolutionParams>,
) -> Result<impl IntoResponse, AppError> {
    let unit = params.unit.as_deref().unwrap_or("day").to_string();
    let mut method_evolution = app_state.stats.get_method_evolution(&unit);

    // Apply `last` limit to each method's series
    if let Some(last) = params.last.filter(|n| *n > 0) {
        let last = last as usize;
        for (_, series) in &mut method_evolution {
            if series.len() > last {
                series.drain(..series.len() - last);
            }
        }
    }

    let result: Vec<serde_json::Value> = method_evolution
        .into_iter()
        .map(|(method, series)| {
            let data: Vec<serde_json::Value> = series
                .iter()
                .map(|bucket| {
                    let chrono_dt =
                        chrono::DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_default();
                    serde_json::json!({"x": chrono_dt.to_rfc3339(), "y": bucket.count})
                })
                .collect();
            serde_json::json!({"id": method, "data": data})
        })
        .collect();

    debug!("Evolution by method: {:?}", result);
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Evolution by method",
        Data::Some(serde_json::to_value(result)?),
    ))
}

use crate::models::{AppState, EmptyResponse, NewRequest, Request};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header::HeaderMap},
    response::IntoResponse,
    routing,
};
use std::mem;
use std::sync::Arc;
use tracing::{debug, error};

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // <-- Ahora solo hay un punto de retorno
    let mut request = NewRequest::from_request(&headers, &app_state.maxmind_db);
    debug!("Captured request: {:?}", request);

    let mut final_status = StatusCode::OK;
    let mut final_message = "Ok";
    let mut must_save = true;

    if let Ok(rules) = app_state.rules.lock() {
        for rule in rules.iter() {
            if rule.matches(&request) {
                request.rule_id = Some(rule.id);
                debug!("Matched rule with rule: {:?}", rule);
                must_save = rule.store;
                if rule.allow {
                    final_status = StatusCode::OK;
                    final_message = "Ok";
                } else {
                    final_status = StatusCode::FORBIDDEN;
                    final_message = "Ko";
                }
                break;
            }
        }
    }
    if must_save {
        save_on_cache_or_db(&app_state, request).await;
    }
    EmptyResponse::create(final_status, final_message)
}

async fn save_on_cache_or_db(app_state: &AppState, request: NewRequest) {
    if app_state.cache_enabled {
        debug!("Cache is enabled, saving request to cache");
        let mut requests_to_save: Option<Vec<NewRequest>> = None;
        {
            if let Ok(mut cache_guard) = app_state.cache.lock() {
                cache_guard.push(request);
                debug!("Request saved to cache. Cache size: {}", cache_guard.len());
                if cache_guard.len() >= app_state.cache_size {
                    requests_to_save = Some(mem::take(&mut *cache_guard));
                    debug!("Cache size reached limit, preparing to bulk save to database");
                    cache_guard.clear();
                }
            }
        }
        if let Some(requests) = requests_to_save {
            debug!(
                "Caching limit reached, saving {} requests to database",
                requests.len()
            );
            match Request::create_bulk(&app_state.pool, requests).await {
                Ok(data) => debug!("Saved {} requests from cache to database", data.len()),
                Err(e) => error!("Error saving requests from cache to database: {:?}", e),
            }
        }
    } else {
        match Request::create(&app_state.pool, request.clone()).await {
            Ok(request) => debug!("Saved request to database: {:?}", request),
            Err(e) => error!("Error saving request to database: {:?}", e),
        }
    }
}

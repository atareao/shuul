use axum::{
    routing,
    Router,
    response::IntoResponse,
    http::{
        StatusCode,
        header::HeaderMap,
    },
    extract::State,
};
use tracing::{
    debug,
};
use crate::models::{
    AppState,
    EmptyResponse,
    NewRecord,
    Record,
    Rule
};
use std::sync::Arc;


pub fn zuul_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::any(zuul))
}

async fn zuul(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let mut record = NewRecord::from_request(
        &headers,
        &app_state.maxmind_db
    );
    debug!("Captured record: {:?}", record);
    let rules = Rule::read_all_active(&app_state.pool)
        .await
        .unwrap_or_default();
    for rule in rules {
        if rule.matches(&record) {
            debug!("Matched rule with rule: {:?}", rule);
            record.rule_id = Some(rule.id);
            let record = Record::create(&app_state.pool, record).await;
            debug!("Created record: {:?}", record);
            if rule.allow {
                return EmptyResponse::create(StatusCode::OK, "Ok");
            }
            return EmptyResponse::create(StatusCode::FORBIDDEN, "Ko");
        }
    }
    let record = Record::create(&app_state.pool, record).await;
    debug!("Created record without rule: {:?}", record);
    EmptyResponse::create(StatusCode::OK, "Ok")
}

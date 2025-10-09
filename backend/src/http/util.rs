use axum::{
    routing,
    Router,
    response::IntoResponse,
    http::StatusCode,
};
use tracing::debug;
use crate::models::{EmptyResponse, AppState};
use std::sync::Arc;


pub fn util_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/complete", routing::get(complete_ip))
}

async fn complete_ip(
) -> impl IntoResponse {
    EmptyResponse::create(StatusCode::OK, "Ok")
}


use axum::{
    routing,
    Router,
    response::IntoResponse,
    http::{
        StatusCode,
        header::HeaderMap,
    },
};
use tracing::info;
use crate::models::{EmptyResponse, AppState};
use std::sync::Arc;


pub fn zuul_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::any(zuul))
}

async fn zuul(
    headers: HeaderMap,
) -> impl IntoResponse {
    let method = headers.get("x-forwarded-method")
        .map(|s| s.to_str())
        .and_then(|result| result.ok())
        .unwrap_or("");
    let protocol = headers.get("x-forwarded-proto")
        .map(|s| s.to_str())
        .and_then(|result| result.ok())
        .unwrap_or("");
    let host = headers.get("x-forwarded-host")
        .map(|s| s.to_str())
        .and_then(|result| result.ok())
        .unwrap_or("");
    let uri = headers.get("x-forwarded-uri")
        .map(|s| s.to_str())
        .and_then(|result| result.ok())
        .unwrap_or("");
    let ip = headers.get("x-forwarded-for")
        .map(|s| s.to_str())
        .and_then(|result| result.ok())
        .unwrap_or("");
    info!("headers: {:?}", headers);
    info!("method: {:?}", method);
    info!("protocol: {}", protocol);
    info!("host: {}", host);
    info!("uri: {}", uri);
    info!("ip: {}", ip);
    EmptyResponse::create(StatusCode::OK, "Ok")
}

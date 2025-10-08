use axum::{
    routing,
    Router,
    response::IntoResponse,
    extract::OriginalUri,
    http::{
        StatusCode,
        Uri,
    },
};
use axum_client_ip::ClientIp;
use tracing::info;
use crate::models::{Data, ApiResponse, AppState};
use std::sync::Arc;


pub fn health_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(check_health))
}

async fn check_health(
    uri: Uri,
    OriginalUri(original_uri): OriginalUri,
    ClientIp(ip): ClientIp
) -> impl IntoResponse {
    info!("Health check from IP: {:?}", ip);
    info!("Url: {:?}", uri);
    info!("scheme: {:?}", uri.scheme_str());
    info!("host: {:?}", uri.host());
    info!("port: {:?}", uri.port());
    info!("path: {:?}", uri.path());
    info!("=======================");
    info!("scheme: {:?}", original_uri.scheme_str());
    info!("authorithy: {:?}", original_uri.authority());
    info!("host: {:?}", original_uri.host());
    info!("port: {:?}", original_uri.port());
    info!("path: {:?}", original_uri.path());
    info!("query: {:?}", original_uri.query());
    ApiResponse::create(StatusCode::OK, "Up and running", Data::None)
}

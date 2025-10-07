use axum::{
    routing,
    Router,
    response::IntoResponse,
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

async fn check_health(uri: Uri, ClientIp(ip): ClientIp) -> impl IntoResponse {
    info!("Health check from IP: {:?}", ip);
    info!("Url: {:?}", uri);
    info!("scheme: {:?}", uri.scheme_str());
    info!("host: {:?}", uri.host());
    info!("port: {:?}", uri.port());
    info!("path: {:?}", uri.path());
    ApiResponse::create(StatusCode::OK, "Up and running", Data::None)
}

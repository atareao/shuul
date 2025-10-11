use axum::{
    routing,
    Router,
    response::IntoResponse,
    http::StatusCode,
    extract::{
        State,
        Query,
    },
};
use tracing::debug;
use crate::models::{
    AppState,
    IPData,
    Data,
    ApiResponse
};
use std::sync::Arc;


pub fn util_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/complete", routing::get(complete_ip))
}

#[derive(Debug, serde::Deserialize)]
pub struct IPParam {
    ip: Option<String>
}

async fn complete_ip(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<IPParam>,
) -> impl IntoResponse {
    debug!("Complete IP: {:?}", params);
    if let Some(ip) = params.ip {
        let ip_data = IPData::complete(&app_state.maxmind_db, &ip);
        debug!("Response IP data: {:?}", ip_data);
        ApiResponse::new(
            StatusCode::OK,
            "Ok",
            Data::Some(serde_json::to_value(ip_data).unwrap_or_default())
        )
    }else{
        ApiResponse::new(
            StatusCode::BAD_REQUEST,
            "Ko",
            Data::None
        )
    }
}


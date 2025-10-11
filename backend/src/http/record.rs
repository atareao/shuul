use serde::Deserialize;
use axum::{
    routing,
    Json,
    Router,
    response::IntoResponse,
    http::StatusCode,
    extract::{
        State,
        Query,
    },
};
use tracing::{
    debug,
    error,
};
use crate::models::{
    AppState,
    Record,
    NewRecord,
    Data,
    ApiResponse
};
use std::sync::Arc;


pub fn record_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/", routing::delete(delete_handler))
}

pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(record): Json<NewRecord>,
) -> impl IntoResponse {
    debug!("Record: {:?}", record);
    match Record::create(&app_state.pool, record).await {
        Ok(record) => {
            debug!("Record created: {:?}", record);
            ApiResponse::new(StatusCode::CREATED, "Record created", Data::Some(serde_json::to_value(record).unwrap()))
        },
        Err(e) => {
            error!("Error creating record: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error creating record", Data::None)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadParams {
    id: Option<i32>,
    ip_address: Option<String>,
    protocol: Option<String>,
    fqdn: Option<String>,
    path: Option<String>,
    query: Option<String>,
    city_name: Option<String>,
    country_name: Option<String>,
    country_code: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
}
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Record::read(&app_state.pool, id).await {
            Ok(record) => {
                debug!("Record: {:?}", record);
                ApiResponse::new(StatusCode::OK, "Record", Data::Some(serde_json::to_value(record).unwrap()))
            },
            Err(e) => {
                error!("Error reading record: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading record", Data::None)
            }
        }
    }else{
        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);
        match Record::read_paged(
            &app_state.pool,
            params.ip_address,
            params.protocol,
            params.fqdn,
            params.path,
            params.query,
            params.city_name,
            params.country_name,
            params.country_code,
            limit,
            offset
        ).await {
            Ok(records) => {
                debug!("Records: {:?}", records);
                ApiResponse::new(StatusCode::OK, "Records", Data::Some(serde_json::to_value(records).unwrap()))
            },
            Err(e) => {
                error!("Error reading records: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading records", Data::None)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    days: Option<i32>,
}

pub async fn delete_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if params.days.is_none() {
        ApiResponse::new(StatusCode::BAD_REQUEST, "Days parameter is required", Data::None)
    }else{
        let days = params.days.unwrap_or(30);
        match Record::delete_before(&app_state.pool, days).await {
            Ok(record) => {
                debug!("Record deleted: {:?}", record);
                ApiResponse::new(StatusCode::OK, "Records deleted", Data::Some(serde_json::to_value(record).unwrap()))
            },
            Err(e) => {
                error!("Error deleting record: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error deleting records", Data::None)
            }
        }
    }
}



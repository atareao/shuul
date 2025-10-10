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


pub fn util_record() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create))
        .route("/", routing::get(read))
        .route("/", routing::patch(update))
        .route("/", routing::delete(delete))
}

pub async fn create(
    State(app_state): State<Arc<AppState>>,
    Json(record): Json<NewRecord>,
) -> impl IntoResponse {
    debug!("Record: {:?}", record);
    match Record::create(&app_state.pool, record).await {
        Ok(record) => {
            debug!("Record created: {:?}", record);
            ApiResponse::new(StatusCode::CREATED, "Record created", Data::One(serde_json::to_value(record).unwrap()))
        },
        Err(e) => {
            error!("Error creating record: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error creating record", Data::None)
        }
    }
}

#[derive(Debug)]
pub struct ReadParams {
    id: Option<i32>
    ip_address: Option<String>,
    fqdn: Option<String>,
}
pub async fn read(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadParams>,
) -> impl IntoResponse {
    debug!("Record: {:?}", params);
    if let Some(township_id) = params.record_id {
        match Record::read_by_township(&app_state.pool, township_id).await {
            Ok(records) => {
                debug!("Records: {:?}", records);
                ApiResponse::new(StatusCode::OK, "Records", Data::One(serde_json::to_value(records).unwrap()))
            },
            Err(e) => {
                error!("Error reading records: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading records", Data::None)
            }
        }
    }else{
        match Record::read_all(&app_state.pool).await {
            Ok(records) => {
                debug!("Records: {:?}", records);
                ApiResponse::new(StatusCode::OK, "Records", Data::One(serde_json::to_value(records).unwrap()))
            },
            Err(e) => {
                error!("Error reading records: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading records", Data::None)
            }
        }
    }
}

#[derive(Debug)]
pub struct DeleteParams {
    pub days: Option<i32>,
}

pub async fn delete(
    State(app_state): State<Arc<AppState>>,
    params: Query<DeleteParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if params.days.is_none() {
        return ApiResponse::new(StatusCode::BAD_REQUEST, "Days parameter is required", Data::None);
    }
    let days = params.days.unwrap_or(30);
    match Record::delete_before(&app_state.pool, days).await {
        Ok(record) => {
            debug!("Record deleted: {:?}", record);
            ApiResponse::new(StatusCode::OK, "Records deleted", Data::One(serde_json::to_value(record).unwrap()))
        },
        Err(e) => {
            error!("Error deleting record: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error deleting records", Data::None)
        }
    }
}



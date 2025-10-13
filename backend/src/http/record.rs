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
    ApiResponse,
    PagedResponse,
    EmptyResponse,
    Pagination,
    ReadRecordParams,
};
use std::sync::Arc;
use crate::constants::DEFAULT_PAGE;
use crate::constants::DEFAULT_LIMIT;

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
            ApiResponse::new(
                StatusCode::CREATED,
                "Record created",
                Data::Some(serde_json::to_value(record).unwrap()))
                .into_response()
        },
        Err(e) => {
            error!("Error creating record: {:?}", e);
            EmptyResponse {
                status: StatusCode::BAD_REQUEST, 
                message: "Error creating record".to_string(),
            }.into_response()
        }
    }
}

pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRecordParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Record::read(&app_state.pool, id).await {
            Ok(record) => {
                debug!("Record: {:?}", record);
                ApiResponse::new(
                    StatusCode::OK,
                    "Record", 
                    Data::Some(serde_json::to_value(record).unwrap())
                ).into_response()
            },
            Err(e) => {
                error!("Error reading record: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error reading record",
                    Data::None).into_response()
            }
        }
    }else{
        if let Ok(records) = Record::read_paged(&app_state.pool, &params).await &&
                let Ok(count) = Record::count_paged(&app_state.pool, &params).await {
            let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
            let offset = params.page.unwrap_or(DEFAULT_PAGE) - 1;
            let total_pages = (count as f32 / limit as f32).ceil() as u32;
            let pagination = Pagination {
                page: offset + 1,
                limit,
                pages: total_pages,
                records: count,
                prev: if offset > 0 {
                    Some(format!("/records?page={}&limit={}", offset, limit))
                }else{
                    None
                },
                next: if (offset + 1) < total_pages {
                    Some(format!("/records?page={}&limit={}", offset + 2, limit))
                }else{
                    None
                },
            };
            return PagedResponse::new(StatusCode::OK, "Records", 
                Data::Some(serde_json::to_value(records).unwrap()),
                pagination)
                .into_response();
        }
        EmptyResponse{
            status: StatusCode::BAD_REQUEST,
            message: "Error reading records".to_string(),
        }.into_response()
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
        ApiResponse::new(
            StatusCode::BAD_REQUEST,
            "Days parameter is required",
            Data::None
        )
    }else{
        let days = params.days.unwrap_or(30);
        match Record::delete_before(&app_state.pool, days).await {
            Ok(record) => {
                debug!("Record deleted: {:?}", record);
                ApiResponse::new(
                    StatusCode::OK,
                    "Records deleted",
                    Data::Some(serde_json::to_value(record).unwrap())
                )
            },
            Err(e) => {
                error!("Error deleting record: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error deleting records",
                    Data::None
                )
            }
        }
    }
}



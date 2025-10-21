use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::{
    ApiResponse, AppState, Data, EmptyResponse, NewRecord, PagedResponse, Pagination,
    ReadRecordParams, Record,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error};

pub fn record_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
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
                Data::Some(serde_json::to_value(record).unwrap()),
            )
            .into_response()
        }
        Err(e) => {
            error!("Error creating record: {:?}", e);
            EmptyResponse {
                status: StatusCode::BAD_REQUEST,
                message: "Error creating record".to_string(),
            }
            .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadInfoParams {
    pub option: Option<String>,
}

pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> impl IntoResponse {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" && opt != "filtered" {
                return ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'filtered'",
                    Data::None,
                )
                .into_response();
            }
            match Record::read_info(&app_state.pool, opt).await {
                Ok(info) => {
                    debug!("Record info: {:?}", info);
                    ApiResponse::new(
                        StatusCode::OK,
                        "Record info",
                        Data::Some(serde_json::to_value(info).unwrap()),
                    )
                    .into_response()
                }
                Err(e) => {
                    error!("Error reading record info: {:?}", e);
                    ApiResponse::new(
                        StatusCode::BAD_REQUEST,
                        "Error reading record info",
                        Data::None,
                    )
                    .into_response()
                }
            }
        }
        None => {
            ApiResponse::new(
                StatusCode::BAD_REQUEST,
                "Option parameter is required",
                Data::None,
            )
            .into_response()
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
                    Data::Some(serde_json::to_value(record).unwrap()),
                )
                .into_response()
            }
            Err(e) => {
                error!("Error reading record: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading record", Data::None)
                    .into_response()
            }
        }
    } else {
        if let Ok(records) = Record::read_paged(&app_state.pool, &params).await
            && let Ok(count) = Record::count_paged(&app_state.pool, &params).await
        {
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
                } else {
                    None
                },
                next: if (offset + 1) < total_pages {
                    Some(format!("/records?page={}&limit={}", offset + 2, limit))
                } else {
                    None
                },
            };
            return PagedResponse::new(
                StatusCode::OK,
                "Records",
                Data::Some(serde_json::to_value(records).unwrap()),
                pagination,
            )
            .into_response();
        }
        EmptyResponse {
            status: StatusCode::BAD_REQUEST,
            message: "Error reading records".to_string(),
        }
        .into_response()
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
            Data::None,
        )
    } else {
        let days = params.days.unwrap_or(30);
        match Record::delete_before(&app_state.pool, days).await {
            Ok(record) => {
                debug!("Record deleted: {:?}", record);
                ApiResponse::new(
                    StatusCode::OK,
                    "Records deleted",
                    Data::Some(serde_json::to_value(record).unwrap()),
                )
            }
            Err(e) => {
                error!("Error deleting record: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error deleting records",
                    Data::None,
                )
            }
        }
    }
}

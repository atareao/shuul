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
    Ignored,
    NewIgnored,
    UpdateIgnored,
    ReadIgnoredParams,
    Data,
    ApiResponse,
    PagedResponse,
    EmptyResponse,
    Pagination,
};
use std::sync::Arc;
use crate::constants::DEFAULT_PAGE;
use crate::constants::DEFAULT_LIMIT;


pub fn ignored_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/", routing::patch(update_handler))
        .route("/", routing::delete(delete_handler))
}

pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(ignored): Json<NewIgnored>,
) -> impl IntoResponse {
    debug!("Ignored: {:?}", ignored);
    match Ignored::create(&app_state.pool, ignored).await {
        Ok(ignored) => {
            debug!("Ignored created: {:?}", ignored);
            ApiResponse::new(StatusCode::CREATED, "Ignored created", Data::Some(serde_json::to_value(ignored).unwrap()))
        },
        Err(e) => {
            error!("Error creating ignored: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error creating ignored", Data::None)
        }
    }
}

pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadIgnoredParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Ignored::read(&app_state.pool, id).await {
            Ok(ignored) => {
                debug!("Ignored: {:?}", ignored);
                ApiResponse::new(
                    StatusCode::OK,
                    "Ignored", 
                    Data::Some(serde_json::to_value(ignored).unwrap())
                ).into_response()
            },
            Err(e) => {
                error!("Error reading ignored: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error reading record",
                    Data::None).into_response()
            }
        }
    }else{
        if let Ok(records) = Ignored::read_paged(&app_state.pool, &params).await &&
                let Ok(count) = Ignored::count_paged(&app_state.pool, &params).await {
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
            if opt != "total" && opt != "active" {
                return ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'active'",
                    Data::None,
                )
                .into_response();
            }
            match Ignored::read_info(&app_state.pool, opt).await {
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



pub async fn update_handler(
    State(app_state): State<Arc<AppState>>,
    Json(ignored): Json<UpdateIgnored>,
) -> impl IntoResponse {
    debug!("Ignored: {:?}", ignored);
    match Ignored::update(&app_state.pool, ignored).await {
        Ok(ignored) => {
            debug!("Ignored updated: {:?}", ignored);
            ApiResponse::new(StatusCode::OK, "Ignored updated", Data::Some(serde_json::to_value(ignored).unwrap()))
        },
        Err(e) => {
            error!("Error updating ignored: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error updating ignored", Data::None)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    id: Option<i32>,
}

pub async fn delete_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if params.id.is_none() {
        ApiResponse::new(StatusCode::BAD_REQUEST, "Days parameter is required", Data::None)
    }else{
        let id = params.id.unwrap_or(30);
        match Ignored::delete(&app_state.pool, id).await {
            Ok(ignored) => {
                debug!("Ignored deleted: {:?}", ignored);
                ApiResponse::new(StatusCode::OK, "Ignoreds deleted", Data::Some(serde_json::to_value(ignored).unwrap()))
            },
            Err(e) => {
                error!("Error deleting ignored: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error deleting ignored", Data::None)
            }
        }
    }
}



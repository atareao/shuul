use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::{
    ApiResponse, AppState, Data, EmptyResponse, NewRequest, PagedResponse, Pagination,
    ReadRequestParams, Request,
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

pub fn request_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/top_countries", routing::get(read_top_countries))
        .route("/top_rules", routing::get(read_top_rules))
        .route("/evolution", routing::get(read_evolution))
        .route("/", routing::delete(delete_handler))
}

#[derive(Debug, Deserialize)]
pub struct EvolutionParams {
    pub unit: Option<String>,
    pub last: Option<i32>,
}
pub async fn read_evolution(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EvolutionParams>,
) -> impl IntoResponse {
    debug!("Evolution params: {:?}", params);
    let unit = params.unit.clone().unwrap_or("day".to_string());
    let last = params.last.unwrap_or(7);
    match Request::evolution(&app_state.pool, &unit, last).await {
        Ok(evolution) => {
            debug!("Request evolution: {:?}", evolution);
            ApiResponse::new(
                StatusCode::OK,
                "Request evolution",
                Data::Some(serde_json::to_value(evolution).unwrap()),
            )
            .into_response()
        }
        Err(e) => {
            let msg = format!("Error reading request evolution: {:?}", e);
            error!("{msg}");
            ApiResponse::new(
                StatusCode::BAD_REQUEST,
                &msg,
                Data::None,
            )
            .into_response()
        }
    }
}

pub async fn read_top_rules(
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match Request::top_rules(&app_state.pool).await {
        Ok(countries) => {
            debug!("Top rules: {:?}", countries);
            ApiResponse::new(
                StatusCode::OK,
                "Top rules",
                Data::Some(serde_json::to_value(countries).unwrap()),
            )
            .into_response()
        }
        Err(e) => {
            let msg = format!("Error reading top rules: {:?}", e);
            error!("{msg}");
            ApiResponse::new(
                StatusCode::BAD_REQUEST,
                &msg,
                Data::None,
            )
            .into_response()
        }
    }
}

pub async fn read_top_countries(
    State(app_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match Request::top_countries(&app_state.pool).await {
        Ok(countries) => {
            debug!("Top countries: {:?}", countries);
            ApiResponse::new(
                StatusCode::OK,
                "Top countries",
                Data::Some(serde_json::to_value(countries).unwrap()),
            )
            .into_response()
        }
        Err(e) => {
            let msg = format!("Error reading top countries: {:?}", e);
            error!("{msg}");
            ApiResponse::new(
                StatusCode::BAD_REQUEST,
                &msg,
                Data::None,
            )
            .into_response()
        }
    }
}

pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<NewRequest>,
) -> impl IntoResponse {
    debug!("Request: {:?}", request);
    match Request::create(&app_state.pool, request).await {
        Ok(request) => {
            debug!("Request created: {:?}", request);
            ApiResponse::new(
                StatusCode::CREATED,
                "Request created",
                Data::Some(serde_json::to_value(request).unwrap()),
            )
            .into_response()
        }
        Err(e) => {
            error!("Error creating request: {:?}", e);
            EmptyResponse {
                status: StatusCode::BAD_REQUEST,
                message: "Error creating request".to_string(),
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
            match Request::read_info(&app_state.pool, opt).await {
                Ok(info) => {
                    debug!("Request info: {:?}", info);
                    ApiResponse::new(
                        StatusCode::OK,
                        "Request info",
                        Data::Some(serde_json::to_value(info).unwrap()),
                    )
                    .into_response()
                }
                Err(e) => {
                    error!("Error reading request info: {:?}", e);
                    ApiResponse::new(
                        StatusCode::BAD_REQUEST,
                        "Error reading request info",
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
    Query(params): Query<ReadRequestParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Request::read(&app_state.pool, id).await {
            Ok(request) => {
                debug!("Request: {:?}", request);
                ApiResponse::new(
                    StatusCode::OK,
                    "Request",
                    Data::Some(serde_json::to_value(request).unwrap()),
                )
                .into_response()
            }
            Err(e) => {
                error!("Error reading request: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading request", Data::None)
                    .into_response()
            }
        }
    } else {
        if let Ok(requests) = Request::read_paged(&app_state.pool, &params).await
            && let Ok(count) = Request::count_paged(&app_state.pool, &params).await
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
                    Some(format!("/requests?page={}&limit={}", offset, limit))
                } else {
                    None
                },
                next: if (offset + 1) < total_pages {
                    Some(format!("/requests?page={}&limit={}", offset + 2, limit))
                } else {
                    None
                },
            };
            return PagedResponse::new(
                StatusCode::OK,
                "Requests",
                Data::Some(serde_json::to_value(requests).unwrap()),
                pagination,
            )
            .into_response();
        }
        EmptyResponse {
            status: StatusCode::BAD_REQUEST,
            message: "Error reading requests".to_string(),
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
        match Request::delete_before(&app_state.pool, days).await {
            Ok(request) => {
                debug!("Request deleted: {:?}", request);
                ApiResponse::new(
                    StatusCode::OK,
                    "Requests deleted",
                    Data::Some(serde_json::to_value(request).unwrap()),
                )
            }
            Err(e) => {
                error!("Error deleting request: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error deleting requests",
                    Data::None,
                )
            }
        }
    }
}

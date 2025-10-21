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
    Rule,
    NewRule,
    UpdateRule,
    ReadRuleParams,
    Data,
    ApiResponse,
    PagedResponse,
    EmptyResponse,
    Pagination,
};
use std::sync::Arc;
use crate::constants::DEFAULT_PAGE;
use crate::constants::DEFAULT_LIMIT;


pub fn rule_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/", routing::patch(update_handler))
        .route("/", routing::delete(delete_handler))
}

pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(rule): Json<NewRule>,
) -> impl IntoResponse {
    debug!("Rule: {:?}", rule);
    match Rule::create(&app_state.pool, rule).await {
        Ok(rule) => {
            debug!("Rule created: {:?}", rule);
            ApiResponse::new(StatusCode::CREATED, "Rule created", Data::Some(serde_json::to_value(rule).unwrap()))
        },
        Err(e) => {
            error!("Error creating rule: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error creating rule", Data::None)
        }
    }
}

pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRuleParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Rule::read(&app_state.pool, id).await {
            Ok(rule) => {
                debug!("Rule: {:?}", rule);
                ApiResponse::new(
                    StatusCode::OK,
                    "Rule", 
                    Data::Some(serde_json::to_value(rule).unwrap())
                ).into_response()
            },
            Err(e) => {
                error!("Error reading rule: {:?}", e);
                ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Error reading record",
                    Data::None).into_response()
            }
        }
    }else{
        if let Ok(records) = Rule::read_paged(&app_state.pool, &params).await &&
                let Ok(count) = Rule::count_paged(&app_state.pool, &params).await {
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
            match Rule::read_info(&app_state.pool, opt).await {
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
    Json(rule): Json<UpdateRule>,
) -> impl IntoResponse {
    debug!("Rule: {:?}", rule);
    match Rule::update(&app_state.pool, rule).await {
        Ok(rule) => {
            debug!("Rule updated: {:?}", rule);
            ApiResponse::new(StatusCode::OK, "Rule updated", Data::Some(serde_json::to_value(rule).unwrap()))
        },
        Err(e) => {
            error!("Error updating rule: {:?}", e);
            ApiResponse::new(StatusCode::BAD_REQUEST, "Error updating rules", Data::None)
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
        match Rule::delete(&app_state.pool, id).await {
            Ok(rule) => {
                debug!("Rule deleted: {:?}", rule);
                ApiResponse::new(StatusCode::OK, "Rules deleted", Data::Some(serde_json::to_value(rule).unwrap()))
            },
            Err(e) => {
                error!("Error deleting rule: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error deleting rules", Data::None)
            }
        }
    }
}



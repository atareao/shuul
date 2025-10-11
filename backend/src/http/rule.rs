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
    Data,
    ApiResponse
};
use std::sync::Arc;


pub fn rule_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
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

#[derive(Debug, Deserialize)]
pub struct ReadParams {
    id: Option<i32>,
    limit: Option<i32>,
    offset: Option<i32>,
}
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadParams>,
) -> impl IntoResponse {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        match Rule::read(&app_state.pool, id).await {
            Ok(rule) => {
                debug!("Rule: {:?}", rule);
                ApiResponse::new(StatusCode::OK, "Rule", Data::Some(serde_json::to_value(rule).unwrap()))
            },
            Err(e) => {
                error!("Error reading rule: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading rule", Data::None)
            }
        }
    }else{
        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);
        match Rule::read_paged(&app_state.pool, limit, offset).await {
            Ok(rules) => {
                debug!("Rules: {:?}", rules);
                ApiResponse::new(StatusCode::OK, "Rules", Data::Some(serde_json::to_value(rules).unwrap()))
            },
            Err(e) => {
                error!("Error reading rules: {:?}", e);
                ApiResponse::new(StatusCode::BAD_REQUEST, "Error reading rules", Data::None)
            }
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



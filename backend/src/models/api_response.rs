use axum::{
    http::StatusCode,
    Json,
    body::Body,
    response::{
        Response,
        IntoResponse,
    }
};
use serde::Serialize;

use super::Data;

#[derive(Debug, Clone)]
pub enum CustomResponse {
    Api(ApiResponse),
    Empty(EmptyResponse),
}

#[derive(Debug, Clone)]
pub struct EmptyResponse {
    pub status: StatusCode,
    pub message: String,
}
impl EmptyResponse {
    pub fn create(status: StatusCode, message: &str) -> Response<Body> {
        Response::builder()
            .status(status) 
            .body(Body::from(message.to_string())) // Cuerpo de la respuesta
            .unwrap()
    }
}

impl IntoResponse for EmptyResponse {
    fn into_response(self) -> Response {
        EmptyResponse::create(self.status, self.message.as_str())
    }
}


#[derive(Debug, Serialize, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub message: String,
    pub data: Data,
}

impl ApiResponse {
    pub fn new(status: StatusCode, message: &str, data: Data) -> Self {
        Self {
            status: status.as_u16(),
            message: message.to_string(),
            data,
        }
    }
    pub fn create(status: StatusCode, message: &str, data: Data) -> Json<ApiResponse> {
        Json(ApiResponse::new(status, message, data))
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).unwrap();
        Response::builder()
            .status(self.status)
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap()
    }
}

impl IntoResponse for CustomResponse {
    fn into_response(self) -> Response {
        match self {
            CustomResponse::Empty(empty_response) => empty_response.into_response(),
            CustomResponse::Api(api_response) => api_response.into_response(),
        }
    }
}

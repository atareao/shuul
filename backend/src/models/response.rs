//! # Tipos de respuesta de la API
//!
//! Define las estructuras estándar de respuesta: [`ApiResponse`], [`PagedResponse`],
//! [`EmptyResponse`], [`ApiResponse`] y [`PagedResponse`]. Todas implementan [`IntoResponse`]
//! para su uso directo en handlers de Axum.

use axum::{
    Json,
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::Data;

#[derive(Debug, Clone)]
pub struct EmptyResponse {
    pub status: StatusCode,
    pub message: String,
}
impl EmptyResponse {
    pub fn create(status: StatusCode, message: &str) -> Response<Body> {
        Response::builder()
            .status(status)
            .body(Body::from(message.to_string()))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal Server Error"))
                    .unwrap()
            })
    }
}

impl IntoResponse for EmptyResponse {
    fn into_response(self) -> Response {
        Self::create(self.status, self.message.as_str())
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
    pub fn create(status: StatusCode, message: &str, data: Data) -> Json<Self> {
        Json(Self::new(status, message, data))
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).unwrap_or_else(|_| {
            r#"{"status":500,"message":"Serialization error","data":null}"#.to_string()
        });
        Response::builder()
            .status(self.status)
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal Server Error"))
                    .unwrap()
            })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub limit: u32,
    pub pages: u32,
    pub records: i64,
    pub prev: Option<String>, // previous page
    pub next: Option<String>, // next page
}

#[derive(Debug, Clone, Serialize)]
pub struct PagedResponse {
    pub status: u16,
    pub message: String,
    pub data: Data,
    pub pagination: Pagination,
}

impl PagedResponse {
    pub fn new(status: StatusCode, message: &str, data: Data, pagination: Pagination) -> Self {
        Self {
            status: status.as_u16(),
            message: message.to_string(),
            data,
            pagination,
        }
    }
    pub fn create(
        status: StatusCode,
        message: &str,
        data: Data,
        pagination: Pagination,
    ) -> Json<Self> {
        Json(Self::new(status, message, data, pagination))
    }
}

impl IntoResponse for PagedResponse {
    fn into_response(self) -> Response {
        let body = serde_json::to_string(&self).unwrap_or_else(|_| {
            r#"{"status":500,"message":"Serialization error","data":null,"pagination":null}"#
                .to_string()
        });
        Response::builder()
            .status(self.status)
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal Server Error"))
                    .unwrap()
            })
    }
}



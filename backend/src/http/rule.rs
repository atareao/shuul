//! # Endpoints de reglas
//!
//! CRUD completo para las reglas de filtrado HTTP:
//! crear, leer (con paginación), actualizar, eliminar y consultar info agregada.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::error::AppError;
use crate::models::{
    ApiResponse, AppState, Data, NewRule, PagedResponse, Pagination, ReadRuleParams, Rule,
    UpdateRule,
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
use tracing::debug;

pub fn rule_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::post(create_handler))
        .route("/", routing::get(read_handler))
        .route("/info", routing::get(read_info_handler))
        .route("/info/all", routing::get(read_info_all_handler))
        .route("/", routing::patch(update_handler))
        .route("/", routing::delete(delete_handler))
        .route("/export", routing::get(export_handler))
        .route("/import", routing::post(import_handler))
}

/// Creates a new rule in the database and updates the in‑memory cache.
///
/// * **Parameters**
///   - `app_state`: Shared application state (DB pool, cache, etc.).
///   - `rule`: The rule payload received from the client.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON response with the created rule or an error.
pub async fn create_handler(
    State(app_state): State<Arc<AppState>>,
    Json(rule): Json<NewRule>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Rule: {:?}", rule);
    let rule = Rule::create(&app_state.pool, rule).await?;
    debug!("Rule created: {:?}", &rule);
    app_state.reload_rules().await?;
    debug!("Rules cache reloaded after create");
    // Propagar error de serialización con `?`
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "Rule created",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

/// Retrieves one or many rules depending on query parameters.
///
/// * **Parameters**
///   - `app_state`: Shared state containing DB pool and cached rules.
///   - `params`: Query parameters for filtering, pagination, etc.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the rule(s) or an error message.
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRuleParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        let rule = Rule::read(&app_state.pool, id).await?;
        debug!("Rule: {:?}", rule);
        Ok(ApiResponse::new(
            StatusCode::OK,
            "Rule",
            Data::Some(serde_json::to_value(rule)?),
        )
        .into_response())
    } else {
        let records = Rule::read_paged(&app_state.pool, &params).await?;
        let count = Rule::count_paged(&app_state.pool, &params).await?;
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = params.page.unwrap_or(DEFAULT_PAGE).saturating_sub(1);
        let total_pages = (count as f64 / f64::from(limit)).ceil() as u32;
        let pagination = Pagination {
            page: offset + 1,
            limit,
            pages: total_pages,
            records: count,
            prev: if offset > 0 {
                Some(format!("/records?page={offset}&limit={limit}"))
            } else {
                None
            },
            next: if (offset + 1) < total_pages {
                Some(format!("/records?page={}&limit={}", offset + 2, limit))
            } else {
                None
            },
        };
        Ok(PagedResponse::new(
            StatusCode::OK,
            "Records",
            Data::Some(serde_json::to_value(records)?),
            pagination,
        )
        .into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadInfoParams {
    pub option: Option<String>,
}

/// Retrieves aggregated information about rules (e.g., total count or active count).
///
/// * **Parameters**
///   - `app_state`: Shared application state.
///   - `params`: Query parameter with an optional `option` field (`"total"` or `"active"`).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the requested info or an error.
pub async fn read_info_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadInfoParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Read info params: {:?}", params);
    match params.option {
        Some(ref opt) => {
            if opt != "total" && opt != "active" {
                return Ok(ApiResponse::new(
                    StatusCode::BAD_REQUEST,
                    "Parameter option must be 'total' or 'active'",
                    Data::None,
                )
                .into_response());
            }
            let info = Rule::read_info(&app_state.pool, opt).await?;
            debug!("Record info: {:?}", info);
            Ok(ApiResponse::new(
                StatusCode::OK,
                "Record info",
                Data::Some(serde_json::to_value(info)?),
            )
            .into_response())
        },
        None => Ok(ApiResponse::new(
            StatusCode::BAD_REQUEST,
            "Option parameter is required",
            Data::None,
        )
        .into_response()),
    }
}

pub async fn read_info_all_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let info = Rule::read_info_all(&app_state.pool).await?;
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule info",
        Data::Some(serde_json::to_value(info)?),
    ))
}

/// Updates an existing rule in the database and refreshes the in‑memory cache.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache, etc.).
///   - `rule`: The update payload.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the updated rule or an error.
pub async fn update_handler(
    State(app_state): State<Arc<AppState>>,
    Json(rule): Json<UpdateRule>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Rule: {:?}", rule);
    let rule = Rule::update(&app_state.pool, rule).await?;
    app_state.reload_rules().await?;
    debug!("Rules cache reloaded after update");
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule updated",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

#[derive(Debug, Deserialize)]
pub struct DeleteParams {
    id: Option<i32>,
}

/// Deletes a rule by ID.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache).
///   - `params`: Query parameter with the rule `id`.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON confirmation or an error.
pub async fn delete_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    let id = params
        .id
        .ok_or_else(|| AppError::InvalidInput("id parameter is required".to_string()))?;
    let rule = Rule::delete(&app_state.pool, id).await?;
    app_state.reload_rules().await?;
    debug!("Rules cache reloaded after delete");
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rules deleted",
        Data::Some(serde_json::to_value(rule)?),
    ))
}

/// Exporta todas las reglas (activas e inactivas) como JSON array.
///
/// No requiere paginación — devuelve todas las reglas en un solo array.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON array de reglas.
pub async fn export_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Exporting all rules");
    let rules = Rule::read_all(&app_state.pool).await?;
    debug!("Exporting {} rules", rules.len());
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rules exported",
        Data::Some(serde_json::to_value(rules)?),
    ))
}

/// Payload para importar reglas.
#[derive(Debug, Deserialize)]
pub struct ImportPayload {
    pub rules: Vec<NewRule>,
}

/// Importa reglas desde un JSON array.
///
/// Cada regla se inserta con UPSERT por `name`:
/// ```sql
/// INSERT INTO rules (...) VALUES (...)
/// ON CONFLICT (name) DO UPDATE SET ...
/// ```
///
/// Después de importar, recarga la caché de reglas.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache).
///   - `payload`: JSON con array de reglas (`{rules: [...]}`).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – `{imported: N}`.
pub async fn import_handler(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<ImportPayload>,
) -> Result<impl IntoResponse, AppError> {
    let count = payload.rules.len();
    debug!("Importing {} rules", count);

    for rule in &payload.rules {
        let now = chrono::Utc::now();
        let sql = r#"INSERT INTO rules (
            name, description, weight, mode, allow, store,
            ip_address, protocol, fqdn, path, query,
            city_name, country_name, country_code,
            user_agent, method, referer, content_type, accept_language, x_request_id,
            rate_limit_profile_id, active, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                  $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                  $21, $22, $23, $24)
        ON CONFLICT (name) DO UPDATE SET
            description = EXCLUDED.description,
            weight = EXCLUDED.weight,
            mode = EXCLUDED.mode,
            allow = EXCLUDED.allow,
            store = EXCLUDED.store,
            ip_address = EXCLUDED.ip_address,
            protocol = EXCLUDED.protocol,
            fqdn = EXCLUDED.fqdn,
            path = EXCLUDED.path,
            query = EXCLUDED.query,
            city_name = EXCLUDED.city_name,
            country_name = EXCLUDED.country_name,
            country_code = EXCLUDED.country_code,
            user_agent = EXCLUDED.user_agent,
            method = EXCLUDED.method,
            referer = EXCLUDED.referer,
            content_type = EXCLUDED.content_type,
            accept_language = EXCLUDED.accept_language,
            x_request_id = EXCLUDED.x_request_id,
            rate_limit_profile_id = EXCLUDED.rate_limit_profile_id,
            active = EXCLUDED.active,
            updated_at = EXCLUDED.updated_at"#;

        sqlx::query(sql)
            .bind(&rule.name)
            .bind(rule.description.as_deref().unwrap_or(""))
            .bind(rule.weight.unwrap_or(100))
            .bind(rule.mode.as_deref().unwrap_or("log_only"))
            .bind(rule.allow.unwrap_or(true))
            .bind(rule.store.unwrap_or(true))
            .bind(&rule.ip_address)
            .bind(&rule.protocol)
            .bind(&rule.fqdn)
            .bind(&rule.path)
            .bind(&rule.query)
            .bind(&rule.city_name)
            .bind(&rule.country_name)
            .bind(&rule.country_code)
            .bind(&rule.user_agent)
            .bind(&rule.method)
            .bind(&rule.referer)
            .bind(&rule.content_type)
            .bind(&rule.accept_language)
            .bind(&rule.x_request_id)
            .bind(rule.rate_limit_profile_id)
            .bind(rule.active.unwrap_or(true))
            .bind(now)
            .bind(now)
            .execute(&app_state.pool)
            .await?;
    }

    // Recargar la caché de reglas
    app_state.reload_rules().await?;
    debug!("Rules cache reloaded after import");

    let response = serde_json::json!({"imported": count});
    Ok(Json(response))
}

//! # Endpoints de reglas
//!
//! CRUD completo para las reglas de filtrado HTTP:
//! crear, leer (con paginación), actualizar, eliminar y consultar info agregada.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::condition::Condition;
use crate::models::error::AppError;
use crate::models::{
    ApiResponse, AppState, ConditionInput, Data, NewRule, PagedResponse, Pagination,
    ReadRuleParams, Rule, UpdateRule, WAF_PIPELINE, compile_conditions, normalized_transformations,
    normalized_weight, validate_pipeline,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::debug;

/// Representación de una regla en las respuestas de la API.
///
/// Aplana todos los campos de [`Rule`] en el nivel superior —la forma que ya
/// consumían los clientes— y añade `conditions`, el conjunto persistido de
/// condiciones. Es aditivo: los clientes actuales ignoran el campo nuevo.
#[derive(Debug, Serialize)]
struct RuleResponse {
    #[serde(flatten)]
    rule: Rule,
    conditions: Vec<ConditionInput>,
}

/// Construye la representación de cada regla con sus condiciones.
///
/// Carga las condiciones de **todas** las reglas con **una sola** consulta por
/// lote ([`Rule::read_conditions_batch`]), nunca una por regla. Es el camino
/// común del listado y la exportación, y también de la creación y la edición,
/// que recargan así el conjunto recién escrito.
///
/// # Errors
///
/// Devuelve un error de base de datos si la consulta por lote falla.
async fn responses_for_rules(
    pool: &SqlitePool,
    rules: Vec<Rule>,
) -> Result<Vec<RuleResponse>, AppError> {
    let ids: Vec<i32> = rules.iter().map(|rule| rule.id).collect();
    let mut conditions_by_rule = Rule::read_conditions_batch(pool, &ids).await?;
    Ok(rules
        .into_iter()
        .map(|rule| {
            let conditions = conditions_by_rule
                .remove(&rule.id)
                .unwrap_or_default()
                .iter()
                .map(ConditionInput::from)
                .collect();
            RuleResponse { rule, conditions }
        })
        .collect())
}

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
    // La respuesta incluye el conjunto de condiciones recién persistido: se
    // recarga con el helper por lote, igual que la lectura y la exportación.
    let response = responses_for_rules(&app_state.pool, vec![rule])
        .await?
        .pop()
        .ok_or_else(|| AppError::Other("regla creada sin representación".to_string()))?;
    // Propagar error de serialización con `?`
    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "Rule created",
        Data::Some(serde_json::to_value(response)?),
    ))
}

/// Retrieves one or many rules depending on query parameters.
///
/// * **Parameters**
///   - `app_state`: Shared state containing DB pool and cached rules.
///   - `params`: Query parameters for filtering, pagination, etc.
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – JSON with the rule(s) or an error message.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub async fn read_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ReadRuleParams>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Params: {:?}", params);
    if let Some(id) = params.id {
        let rule = Rule::read(&app_state.pool, id).await?;
        debug!("Rule: {:?}", rule);
        let response = responses_for_rules(&app_state.pool, vec![rule])
            .await?
            .pop()
            .ok_or_else(|| AppError::Other("regla leída sin representación".to_string()))?;
        Ok(ApiResponse::new(
            StatusCode::OK,
            "Rule",
            Data::Some(serde_json::to_value(response)?),
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
        // Una sola carga por lote para todas las reglas de la página.
        let responses = responses_for_rules(&app_state.pool, records).await?;
        Ok(PagedResponse::new(
            StatusCode::OK,
            "Records",
            Data::Some(serde_json::to_value(responses)?),
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
    // La respuesta incluye el conjunto de condiciones recién reemplazado.
    let response = responses_for_rules(&app_state.pool, vec![rule])
        .await?
        .pop()
        .ok_or_else(|| AppError::Other("regla actualizada sin representación".to_string()))?;
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule updated",
        Data::Some(serde_json::to_value(response)?),
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
    // Una sola carga por lote de las condiciones de todas las reglas exportadas.
    let responses = responses_for_rules(&app_state.pool, rules).await?;
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rules exported",
        Data::Some(serde_json::to_value(responses)?),
    ))
}

/// Payload para importar reglas.
#[derive(Debug, Deserialize)]
pub struct ImportPayload {
    pub rules: Vec<NewRule>,
}

/// Sentencia de UPSERT por `name` que usa la importación.
///
/// Vive fuera de [`import_handler`] para no alargar la función: el `INSERT`
/// inserta o, en conflicto por `name`, actualiza todos los campos de la versión
/// importada. `RETURNING id` da el id real en ambas ramas.
const IMPORT_RULE_SQL: &str = r"INSERT INTO rules (
            name, description, weight, mode, pipeline, allow, is_tor,
            ip_address, protocol, fqdn, path, query,
            city_name, country_name, country_code,
            user_agent, method, referer, content_type, accept_language, x_request_id,
            rate_limit_profile_id, active, created_at, updated_at, transformations
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                  $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                  $21, $22, $23, $24, $25, $26)
        ON CONFLICT (name) DO UPDATE SET
            description = EXCLUDED.description,
            weight = EXCLUDED.weight,
            mode = EXCLUDED.mode,
            pipeline = EXCLUDED.pipeline,
            transformations = EXCLUDED.transformations,
            allow = EXCLUDED.allow,
            is_tor = EXCLUDED.is_tor,
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
            updated_at = EXCLUDED.updated_at
        RETURNING id";

/// Una entrada de importación ya validada y compilada, lista para el UPSERT.
struct ValidatedImportRule {
    pipeline: String,
    transformations: String,
    weight: i32,
    /// El conjunto compilado de condiciones; `None` = modo simple.
    compiled: Option<Vec<Condition>>,
}

/// Valida y compila una entrada de importación **antes** de tocar la base de datos.
///
/// Valida el pipeline, canoniza la especificación de transformaciones, coerciona
/// el peso y compila las condiciones. Es el punto común de rechazo de la
/// importación: si algo falla, la entrada no se almacena y no aborta el resto.
///
/// # Errors
///
/// Devuelve [`AppError::InvalidInput`] si el pipeline no se admite, si la
/// especificación de transformaciones no se resuelve o si alguna condición no
/// compila.
fn validate_import_rule(rule: &NewRule) -> Result<ValidatedImportRule, AppError> {
    let pipeline = rule.pipeline.as_deref().unwrap_or(WAF_PIPELINE);
    validate_pipeline(pipeline)?;
    let transformations =
        normalized_transformations(rule.transformations.as_deref().unwrap_or_default())?;
    let weight = normalized_weight(pipeline, rule.weight.unwrap_or(100));
    let compiled = compile_conditions(rule.conditions.as_deref())?;
    Ok(ValidatedImportRule {
        pipeline: pipeline.to_string(),
        transformations,
        weight,
        compiled,
    })
}

/// Registra en `rejected` una entrada de la importación que no se almacenó.
///
/// La importación es **por entradas**: la entrada rechazada viaja de vuelta con su
/// nombre y el motivo —que nombra el token o el pipeline no admitido—, en lugar de
/// abortar la importación entera o descartarse en silencio. Lo usan las dos
/// validaciones de [`import_handler`] —el pipeline y la especificación de
/// transformaciones— para que el rechazo sea idéntico en las dos.
fn reject_import(rejected: &mut Vec<serde_json::Value>, name: &str, error: &AppError) {
    debug!("Rule '{name}' rejected on import: {error}");
    rejected.push(serde_json::json!({
        "name": name,
        "reason": error.to_string(),
    }));
}

/// Importa reglas desde un JSON array.
///
/// Cada regla se inserta con UPSERT por `name`:
/// ```sql
/// INSERT INTO rules (...) VALUES (...)
/// ON CONFLICT (name) DO UPDATE SET ...
/// ```
///
/// La importación es **por entradas**: una entrada con un pipeline no admitido
/// (lo que no sea exactamente `waf` o `jail`) o con una especificación de
/// transformaciones irresoluble no se almacena, pero tampoco aborta la importación
/// entera ni se descarta en silencio —viaja de vuelta en `rejected` con su nombre y
/// el motivo—. La especificación se **canoniza** con
/// [`normalized_transformations`] antes de escribirla, igual que el peso con
/// [`normalized_weight`]. `imported` cuenta solo las que se almacenaron.
///
/// Después de importar, recarga la caché de reglas.
///
/// * **Parameters**
///   - `app_state`: Shared state (DB pool, cache).
///   - `payload`: JSON con array de reglas (`{rules: [...]}`).
/// * **Returns**
///   - `Result<impl IntoResponse, AppError>` – `{imported: N, rejected: [...]}`.
pub async fn import_handler(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<ImportPayload>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Importing {} rules", payload.rules.len());

    let mut imported = 0_usize;
    let mut rejected: Vec<serde_json::Value> = Vec::new();

    for rule in &payload.rules {
        let now = chrono::Utc::now();
        // Valida pipeline, transformaciones, peso y condiciones ANTES de tocar la
        // base de datos. Una entrada inválida se rechaza sin abortar el resto.
        let ValidatedImportRule {
            pipeline,
            transformations,
            weight,
            compiled,
        } = match validate_import_rule(rule) {
            Ok(validated) => validated,
            Err(error) => {
                reject_import(&mut rejected, &rule.name, &error);
                continue;
            },
        };

        // El UPSERT de la fila y el reemplazo de sus condiciones comparten
        // transacción: `RETURNING id` da el id real en INSERT y en DO UPDATE.
        let mut tx = app_state.pool.begin().await?;
        let rule_id: i32 = sqlx::query_scalar(IMPORT_RULE_SQL)
            .bind(&rule.name)
            .bind(rule.description.as_deref().unwrap_or(""))
            .bind(weight)
            .bind(rule.mode.as_deref().unwrap_or("log_only"))
            .bind(&pipeline)
            .bind(rule.allow.unwrap_or(true))
            .bind(rule.is_tor.unwrap_or(false))
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
            .bind(&transformations)
            .fetch_one(&mut *tx)
            .await?;
        Rule::replace_conditions_for_id(&mut tx, rule_id, compiled.as_deref()).await?;
        tx.commit().await?;
        imported += 1;
    }

    // Recargar la caché de reglas
    app_state.reload_rules().await?;
    debug!(
        "Rules cache reloaded after import: {imported} imported, {} rejected",
        rejected.len()
    );

    let response = serde_json::json!({
        "imported": imported,
        "rejected": rejected,
    });
    Ok(Json(response))
}

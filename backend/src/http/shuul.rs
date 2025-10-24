use crate::models::{AppState, EmptyResponse, NewRecord, Record};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header::HeaderMap},
    response::IntoResponse,
    routing,
};
use std::mem;
use std::sync::Arc;
use tracing::{debug, error};

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // <-- Ahora solo hay un punto de retorno
    let mut record = NewRecord::from_request(&headers, &app_state.maxmind_db);
    debug!("Captured record: {:?}", record);

    let mut final_status = StatusCode::OK;
    let mut final_message = "Ok";
    let mut rule_id: i32 = -1;
    let mut must_save = true;

    if let Ok(rules) = app_state.rules.lock() {
        for rule in rules.iter() {
            if rule.matches(&record) {
                rule_id = rule.id;
                debug!("Matched rule with rule: {:?}", rule);
                // Comprobación de ignorado
                if let Ok(ignored) = app_state.ignored.lock() {
                    for one in ignored.iter() {
                        if one.matches(&record) {
                            must_save = false;
                            break; // Se ignoró, salimos del bucle de ignorados
                        }
                    }
                }
                // Decidir la respuesta (Permitir o Denegar)
                if rule.allow {
                    final_status = StatusCode::OK;
                    final_message = "Ok";
                } else {
                    final_status = StatusCode::FORBIDDEN;
                    final_message = "Ko";
                }
                // Si encontramos una regla, no necesitamos seguir buscando
                break;
            }
        }
    }
    // Lógica de guardado de registro
    if must_save {
        debug!("Record is not ignored");
        if rule_id > -1 {
            // Solo guardamos si se encontró una regla y no fue ignorada
            record.rule_id = Some(rule_id); // Usa el ID de la regla coincidente si existe
            debug!("Store record with rule: {:?}", record);
            save_on_cache_or_db(&app_state, record).await;
        } else {
            // Guardar si no hay reglas que apliquen
            debug!("Store record without rule: {:?}", record);
            save_on_cache_or_db(&app_state, record).await;
        }
    } else {
        debug!("Record is ignored, skipping save record");
    }
    // Retorno unificado
    EmptyResponse::create(final_status, final_message)
}

async fn save_on_cache_or_db(app_state: &AppState, record: NewRecord) {
    if app_state.cache_enabled {
        debug!("Cache is enabled, saving record to cache");
        let mut records_to_save: Option<Vec<NewRecord>> = None;
        {
            if let Ok(mut cache_guard) = app_state.cache.lock() {
                cache_guard.push(record);
                debug!("Record saved to cache. Cache size: {}", cache_guard.len());
                if cache_guard.len() >= app_state.cache_size {
                    records_to_save = Some(mem::take(&mut *cache_guard));
                    debug!("Cache size reached limit, preparing to bulk save to database");
                    cache_guard.clear();
                }
            }
        }
        if let Some(records) = records_to_save {
            debug!(
                "Caching limit reached, saving {} records to database",
                records.len()
            );
            match Record::create_bulk(&app_state.pool, records).await {
                Ok(data) => debug!("Saved {} records from cache to database", data.len()),
                Err(e) => error!("Error saving records from cache to database: {:?}", e),
            }
        }
    } else {
        match Record::create(&app_state.pool, record.clone()).await {
            Ok(record) => debug!("Saved record to database: {:?}", record),
            Err(e) => error!("Error saving record to database: {:?}", e),
        }
    }
}

use axum::{
    routing,
    Router,
    response::IntoResponse,
    http::{
        StatusCode,
        header::HeaderMap,
    },
    extract::State,
};
use tracing::{
    debug,
};
use crate::models::{
    AppState,
    EmptyResponse,
    NewRecord,
    Record,
};
use std::sync::Arc;


pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::any(shuul))
}

pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse { // <-- Ahora solo hay un punto de retorno
    let mut record = NewRecord::from_request(
        &headers,
        &app_state.maxmind_db
    );
    debug!("Captured record: {:?}", record);

    let mut final_status = StatusCode::OK;
    let mut final_message = "Ok";
    let mut rule_matched = false;
    let mut must_save = true;

    if let Ok(rules) = app_state.rules.lock() {
        for rule in rules.iter() {
            if rule.matches(&record) {
                rule_matched = true;
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
    if rule_matched && must_save {
        debug!("Record is not ignored, saving record");
        // Solo guardamos si se encontró una regla y no fue ignorada
        record.rule_id = Some(record.rule_id.unwrap_or_default()); // Usa el ID de la regla coincidente si existe
        let record = Record::create(&app_state.pool, record).await;
        debug!("Created record: {:?}", record);
    } else if !rule_matched {
        // Guardar si no hay reglas que apliquen
        debug!("Created record without rule: {:?}", record);
        let record = Record::create(&app_state.pool, record).await;
        debug!("Created record: {:?}", record);
    } else {
        debug!("Record is ignored, skipping save record");
    }
    // Retorno unificado
    EmptyResponse::create(final_status, final_message)
}

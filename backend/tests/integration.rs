//! Tests de integración con base de datos `PostgreSQL` real.
//!
//! Requisitos:
//!   1. `PostgreSQL` corriendo (ver `docker-compose.test.yml`)
//!   2. Variable de entorno `DATABASE_URL` configurada
//!
//! Ejecución:
//!   ```bash
//!   podman compose -f docker-compose.test.yml up -d
//!   ./setup_test_db.sh
//!   DATABASE_URL=postgres://test:test@localhost:5433/test_db cargo test --test integration -- --nocapture
//!   ```

use sqlx::PgPool;
use std::env;

/// Obtiene la URL de la base de datos desde la variable de entorno.
/// Si no está configurada, los tests se ignoran.
fn get_test_db_url() -> Option<String> {
    env::var("DATABASE_URL").ok()
}

/// Crea un pool de conexiones para los tests.
async fn setup_pool() -> Option<PgPool> {
    let url = get_test_db_url()?;
    PgPool::connect(&url).await.ok()
}

// ──────────────────────────────────────────────
// Tests de conexión
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_db_connection() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    let row: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("Error ejecutando query de prueba");

    assert_eq!(row.0, 1);
}

// ──────────────────────────────────────────────
// Tests de migraciones (tablas existen)
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_rules_table_exists() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    let result: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'rules'")
            .fetch_one(&pool)
            .await;

    assert!(result.is_ok());
    let (count,) = result.unwrap();
    assert_eq!(count, 1, "La tabla 'rules' debe existir");
}

#[tokio::test]
async fn test_requests_table_exists() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'requests'",
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());
    let (count,) = result.unwrap();
    assert_eq!(count, 1, "La tabla 'requests' debe existir");
}

// ──────────────────────────────────────────────
// Tests CRUD de Rules
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_rule_crud() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    // Resetear sequence para evitar colisiones con datos seed
    sqlx::query("SELECT setval('rules_id_seq', (SELECT COALESCE(MAX(id), 0) + 1 FROM rules))")
        .execute(&pool)
        .await
        .expect("Error reseteando sequence");

    // CREATE (dejamos que PostgreSQL asigne el ID automáticamente)
    let rule_id: i32 = sqlx::query_scalar(
        "INSERT INTO rules (weight, allow, store, active) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(100i32)
    .bind(true)
    .bind(true)
    .bind(true)
    .fetch_one(&pool)
    .await
    .expect("Error creando regla");

    assert!(rule_id > 0);

    // READ
    let (weight, allow, store, active): (i32, bool, bool, bool) =
        sqlx::query_as("SELECT weight, allow, store, active FROM rules WHERE id = $1")
            .bind(rule_id)
            .fetch_one(&pool)
            .await
            .expect("Error leyendo regla");

    assert_eq!(weight, 100);
    assert!(allow);
    assert!(store);
    assert!(active);

    // UPDATE
    sqlx::query("UPDATE rules SET weight = $1, allow = $2 WHERE id = $3")
        .bind(200i32)
        .bind(false)
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error actualizando regla");

    let (updated_weight, updated_allow): (i32, bool) =
        sqlx::query_as("SELECT weight, allow FROM rules WHERE id = $1")
            .bind(rule_id)
            .fetch_one(&pool)
            .await
            .expect("Error leyendo regla actualizada");

    assert_eq!(updated_weight, 200);
    assert!(!updated_allow);

    // DELETE
    sqlx::query("DELETE FROM rules WHERE id = $1")
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error eliminando regla");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rules WHERE id = $1")
        .bind(rule_id)
        .fetch_one(&pool)
        .await
        .expect("Error verificando eliminación");

    assert_eq!(count, 0);
}

// ──────────────────────────────────────────────
// Tests CRUD de Requests
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_request_crud() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    // CREATE (solo columnas que existen en el schema)
    let request_id: i32 = sqlx::query_scalar(
        "INSERT INTO requests (ip_address, protocol, fqdn, path) VALUES ($1, $2, $3, $4) RETURNING id"
    )
    .bind("127.0.0.1")
    .bind("https")
    .bind("example.com")
    .bind("/test")
    .fetch_one(&pool)
    .await
    .expect("Error creando petición");

    assert!(request_id > 0);

    // READ
    let (ip, protocol): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT ip_address, protocol FROM requests WHERE id = $1")
            .bind(request_id)
            .fetch_one(&pool)
            .await
            .expect("Error leyendo petición");

    assert_eq!(ip.unwrap(), "127.0.0.1");
    assert_eq!(protocol.unwrap(), "https");

    // DELETE
    sqlx::query("DELETE FROM requests WHERE id = $1")
        .bind(request_id)
        .execute(&pool)
        .await
        .expect("Error eliminando petición");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requests WHERE id = $1")
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .expect("Error verificando eliminación");

    assert_eq!(count, 0);
}

// ──────────────────────────────────────────────
// Tests de Foreign Key
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_request_with_rule_fk() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    // Resetear sequence para evitar colisiones con datos seed
    sqlx::query("SELECT setval('rules_id_seq', (SELECT COALESCE(MAX(id), 0) + 1 FROM rules))")
        .execute(&pool)
        .await
        .expect("Error reseteando sequence");

    // Crear regla (ID asignado por sequence)
    let rule_id: i32 = sqlx::query_scalar(
        "INSERT INTO rules (weight, allow, store, active) VALUES (999, true, true, true) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .expect("Error creando regla");

    // Crear petición con rule_id
    let request_id: i32 = sqlx::query_scalar(
        "INSERT INTO requests (ip_address, rule_id) VALUES ($1, $2) RETURNING id",
    )
    .bind("192.168.1.1")
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .expect("Error creando petición con FK");

    // Verificar que el rule_id se guardó
    let (fk_rule_id,): (Option<i32>,) =
        sqlx::query_as("SELECT rule_id FROM requests WHERE id = $1")
            .bind(request_id)
            .fetch_one(&pool)
            .await
            .expect("Error leyendo FK");

    assert_eq!(fk_rule_id, Some(rule_id));

    // Eliminar regla — el rule_id debe quedar en NULL (ON DELETE SET NULL)
    sqlx::query("DELETE FROM rules WHERE id = $1")
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error eliminando regla");

    let (fk_after_delete,): (Option<i32>,) =
        sqlx::query_as("SELECT rule_id FROM requests WHERE id = $1")
            .bind(request_id)
            .fetch_one(&pool)
            .await
            .expect("Error leyendo FK tras eliminación");

    assert_eq!(
        fk_after_delete, None,
        "El rule_id debe ser NULL tras eliminar la regla"
    );

    // Limpiar
    sqlx::query("DELETE FROM requests WHERE id = $1")
        .bind(request_id)
        .execute(&pool)
        .await
        .ok();
}

// ──────────────────────────────────────────────
// Tests de índices
// ──────────────────────────────────────────────

#[tokio::test]
async fn test_indexes_exist() {
    let Some(pool) = setup_pool().await else {
        eprintln!("⚠️  DATABASE_URL no configurada, saltando test");
        return;
    };

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'public' ORDER BY indexname",
    )
    .fetch_all(&pool)
    .await
    .expect("Error consultando índices");

    let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

    // Verificar que existen los índices que creamos
    assert!(
        index_names
            .iter()
            .any(|n| n.contains("requests_created_at")),
        "Debe existir un índice en requests.created_at. Índices encontrados: {index_names:?}"
    );
    assert!(
        index_names.iter().any(|n| n.contains("requests_rule_id")),
        "Debe existir un índice en requests.rule_id. Índices encontrados: {index_names:?}"
    );
    assert!(
        index_names.iter().any(|n| n.contains("rules_active")),
        "Debe existir un índice en rules.active. Índices encontrados: {index_names:?}"
    );
}

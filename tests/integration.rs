//! Tests de integración con base de datos SQLite.
//!
//! Usa `sqlite::memory:` por defecto, o la variable de entorno `DATABASE_URL`
//! para apuntar a un archivo `.db`.
//!
//! Ejecución:
//!   ```bash
//!   cargo test --test integration -- --nocapture
//!   ```

use sqlx::SqlitePool;
use std::env;

fn get_test_db_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

async fn setup_pool() -> Option<SqlitePool> {
    let url = get_test_db_url();
    SqlitePool::connect(&url).await.ok()
}

#[tokio::test]
async fn test_db_connection() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let row: (i64,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("Error ejecutando query de prueba");

    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn test_users_table_exists() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users'",
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());
    let (count,) = result.unwrap();
    assert_eq!(count, 1, "La tabla 'users' debe existir");
}

#[tokio::test]
async fn test_rules_table_exists() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rules'",
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());
    let (count,) = result.unwrap();
    assert_eq!(count, 1, "La tabla 'rules' debe existir");
}

#[tokio::test]
async fn test_requests_table_exists() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='requests'",
    )
    .fetch_one(&pool)
    .await;

    assert!(result.is_ok());
    let (count,) = result.unwrap();
    assert_eq!(count, 1, "La tabla 'requests' debe existir");
}

#[tokio::test]
async fn test_user_crud() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let email = format!("test_{}@example.com", uuid::Uuid::new_v4());
    let username = format!("user_{}", uuid::Uuid::new_v4());
    let password = "test_password_123";
    let role = "admin";

    let hashed = bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("Error hasheando password");
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, email, hashed_password, role, active) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&username)
    .bind(&email)
    .bind(&hashed)
    .bind(role)
    .bind(true)
    .fetch_one(&pool)
    .await
    .expect("Error creando usuario");

    assert!(user_id > 0, "El ID del usuario debe ser positivo");

    let (db_email, db_role): (String, String) = sqlx::query_as(
        "SELECT email, role FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo usuario");

    assert_eq!(db_email, email);
    assert_eq!(db_role, role);

    let new_role = "user";
    sqlx::query("UPDATE users SET role = ? WHERE id = ?")
        .bind(new_role)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("Error actualizando usuario");

    let (updated_role,): (String,) = sqlx::query_as("SELECT role FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("Error leyendo rol actualizado");

    assert_eq!(updated_role, new_role);

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("Error eliminando usuario");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("Error verificando eliminacion");

    assert_eq!(count, 0, "El usuario debe haber sido eliminado");
}

#[tokio::test]
async fn test_rule_crud() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let rule_id: i64 = sqlx::query_scalar(
        "INSERT INTO rules (weight, allow, active) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(1i32)
    .bind(true)
    .bind(true)
    .fetch_one(&pool)
    .await
    .expect("Error creando regla");

    assert!(rule_id > 0);

    let (weight, allow, active): (i32, bool, bool) = sqlx::query_as(
        "SELECT weight, allow, active FROM rules WHERE id = ?",
    )
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo regla");

    assert_eq!(weight, 1);
    assert!(allow);
    assert!(active);

    sqlx::query("UPDATE rules SET weight = ?, allow = ? WHERE id = ?")
        .bind(10i32)
        .bind(false)
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error actualizando regla");

    let (updated_weight, updated_allow): (i32, bool) = sqlx::query_as(
        "SELECT weight, allow FROM rules WHERE id = ?",
    )
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo regla actualizada");

    assert_eq!(updated_weight, 10);
    assert!(!updated_allow);

    sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error eliminando regla");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rules WHERE id = ?")
        .bind(rule_id)
        .fetch_one(&pool)
        .await
        .expect("Error verificando eliminacion");

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_request_crud() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let request_id: i64 = sqlx::query_scalar(
        "INSERT INTO requests (ip_address, protocol, fqdn, path, method) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind("127.0.0.1")
    .bind("https")
    .bind("example.com")
    .bind("/test")
    .bind("GET")
    .fetch_one(&pool)
    .await
    .expect("Error creando peticion");

    assert!(request_id > 0);

    let (ip, method): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT ip_address, method FROM requests WHERE id = ?",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo peticion");

    assert_eq!(ip.unwrap(), "127.0.0.1");
    assert_eq!(method.unwrap(), "GET");

    sqlx::query("DELETE FROM requests WHERE id = ?")
        .bind(request_id)
        .execute(&pool)
        .await
        .expect("Error eliminando peticion");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requests WHERE id = ?")
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .expect("Error verificando eliminacion");

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_request_with_rule_fk() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let rule_id: i64 = sqlx::query_scalar(
        "INSERT INTO rules (weight, allow, active) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(1i32)
    .bind(true)
    .bind(true)
    .fetch_one(&pool)
    .await
    .expect("Error creando regla");

    let request_id: i64 = sqlx::query_scalar(
        "INSERT INTO requests (ip_address, rule_id) VALUES (?, ?) RETURNING id",
    )
    .bind("192.168.1.1")
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .expect("Error creando peticion con FK");

    let (fk_rule_id,): (Option<i32>,) = sqlx::query_as(
        "SELECT rule_id FROM requests WHERE id = ?",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo FK");

    assert_eq!(fk_rule_id, Some(rule_id));

    sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(rule_id)
        .execute(&pool)
        .await
        .expect("Error eliminando regla");

    let (fk_after_delete,): (Option<i32>,) = sqlx::query_as(
        "SELECT rule_id FROM requests WHERE id = ?",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .expect("Error leyendo FK tras eliminacion");

    assert_eq!(fk_after_delete, None, "El rule_id debe ser NULL tras eliminar la regla");

    sqlx::query("DELETE FROM requests WHERE id = ?")
        .bind(request_id)
        .execute(&pool)
        .await
        .ok();
}

#[tokio::test]
async fn test_indexes_exist() {
    let pool = match setup_pool().await {
        Some(p) => p,
        None => {
            eprintln!("No se pudo conectar a SQLite, saltando test");
            return;
        }
    };

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='rules' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("Error consultando indices");

    assert!(!indexes.is_empty(), "Deben existir indices en la tabla 'rules'");
    assert!(
        indexes.iter().any(|(n,)| n.contains("rules")),
        "Debe existir un indice relacionado con 'rules'. Indices encontrados: {indexes:?}"
    );
}
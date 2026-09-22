//! # Modelo de configuración global (Settings)
//!
//! Define [`Settings`] con los parámetros globales de la aplicación
//! almacenados en la tabla `settings` (key-value).
//!
//! Los valores se cargan y persisten mediante los métodos
//! [`Settings::load`] y [`Settings::save`].

use sqlx::{Row, SqlitePool};

use crate::models::error::AppError;

/// Configuración global de la aplicación.
///
/// Cada campo se corresponde con una clave en la tabla `settings`.
/// - `default_rule_mode`: modo por defecto para nuevas reglas
/// - `log_retention_days`: días de retención de logs de peticiones
/// - `log_all_requests`: nivel de log para peticiones
#[derive(Debug, Clone)]
pub struct Settings {
    pub default_rule_mode: String,
    pub log_retention_days: i32,
    pub log_all_requests: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_rule_mode: "log_only".to_string(),
            log_retention_days: 30,
            log_all_requests: "all".to_string(),
        }
    }
}

impl Settings {
    /// Carga la configuración desde la base de datos.
    ///
    /// Lee todas las claves de la tabla `settings` y construye un [`Settings`].
    /// Si una clave no existe, se usa el valor por defecto.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or if the stored values cannot be parsed.
    pub async fn load(pool: &SqlitePool) -> Result<Self, AppError> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(pool)
            .await?;

        let mut map: std::collections::HashMap<String, String> = rows
            .into_iter()
            .map(|row| {
                let key: String = row.get("key");
                let value: String = row.get("value");
                (key, value)
            })
            .collect();

        let default_rule_mode = map
            .remove("default_rule_mode")
            .unwrap_or_else(|| "log_only".to_string());

        let log_retention_days_raw = map.remove("log_retention_days").unwrap_or_default();
        let log_retention_days: i32 = log_retention_days_raw.parse().unwrap_or(30);

        let log_all_requests = map
            .remove("log_all_requests")
            .unwrap_or_else(|| "all".to_string());

        Ok(Self {
            default_rule_mode,
            log_retention_days,
            log_all_requests,
        })
    }

    /// Persiste la configuración en la base de datos (UPSERT).
    ///
    /// Cada campo de [`Settings`] se guarda como una fila independiente
    /// en la tabla `settings`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database upsert fails.
    pub async fn save(pool: &SqlitePool, settings: &Self) -> Result<(), AppError> {
        let pairs = vec![
            ("default_rule_mode", settings.default_rule_mode.clone()),
            (
                "log_retention_days",
                settings.log_retention_days.to_string(),
            ),
            ("log_all_requests", settings.log_all_requests.clone()),
        ];

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?) \
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(&value)
            .execute(pool)
            .await?;
        }

        Ok(())
    }
}

//! # Modelo de perfiles de rate limiting
//!
//! Define [`RateLimitProfile`], [`NewRateLimitProfile`], [`UpdateRateLimitProfile`]
//! y [`ReadRateLimitProfileParams`] para gestionar perfiles reutilizables de
//! limitación de tasa. Cada perfil configura los parámetros de rate limiting
//! (intentos, ventanas de tiempo, escalado de bans) y puede ser referenciado
//! desde múltiples reglas mediante `rate_limit_profile_id`.
//!
//! # Almacenamiento en SQLite
//!
//! Los campos `bantime_multipliers` y `fail_codes` son arrays de enteros que se
//! almacenan como texto JSON (e.g., `'[1,2,4,8]'`) porque SQLite no tiene un
//! tipo array nativo.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Error, Row, SqlitePool, query, sqlite::SqliteRow};

/// Analiza una columna de tipo `TEXT` con JSON array en un `Vec<i32>`.
///
/// Devuelve un vec vacío si el valor es `None` o no se puede parsear.
fn parse_json_array(val: Option<String>) -> Vec<i32> {
    val.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Perfil completo de rate limiting, incluyendo metadatos de persistencia.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitProfile {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub max_retry: i32,
    pub find_time_seconds: i32,
    pub ban_time_seconds: i32,
    pub bantime_increment: bool,
    pub bantime_multipliers: Vec<i32>,
    pub bantime_maxtime_seconds: i32,
    pub ban_count_decay_days: i32,
    pub fail_codes: Vec<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Dats necesarios para crear un nuevo perfil de rate limiting.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRateLimitProfile {
    pub name: String,
    pub description: Option<String>,
    pub max_retry: Option<i32>,
    pub find_time_seconds: Option<i32>,
    pub ban_time_seconds: Option<i32>,
    pub bantime_increment: Option<bool>,
    pub bantime_multipliers: Option<Vec<i32>>,
    pub bantime_maxtime_seconds: Option<i32>,
    pub ban_count_decay_days: Option<i32>,
    pub fail_codes: Option<Vec<i32>>,
}

/// Dats para actualizar un perfil de rate limiting existente.
/// Todos los campos excepto `id` son opcionales para permitir actualización parcial.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateRateLimitProfile {
    pub id: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub max_retry: Option<i32>,
    pub find_time_seconds: Option<i32>,
    pub ban_time_seconds: Option<i32>,
    pub bantime_increment: Option<bool>,
    pub bantime_multipliers: Option<Vec<i32>>,
    pub bantime_maxtime_seconds: Option<i32>,
    pub ban_count_decay_days: Option<i32>,
    pub fail_codes: Option<Vec<i32>>,
}

/// Parámetros de filtrado y paginación para listar perfiles de rate limiting.
#[derive(Debug, Deserialize)]
pub struct ReadRateLimitProfileParams {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}

use crate::constants::{DEFAULT_LIMIT, DEFAULT_PAGE};

impl RateLimitProfile {
    /// Construye un `RateLimitProfile` desde una fila de SQLite.
    fn from_row(row: SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            max_retry: row.get("max_retry"),
            find_time_seconds: row.get("find_time_seconds"),
            ban_time_seconds: row.get("ban_time_seconds"),
            bantime_increment: row.get("bantime_increment"),
            bantime_multipliers: parse_json_array(
                row.get::<Option<String>, _>("bantime_multipliers"),
            ),
            bantime_maxtime_seconds: row.get("bantime_maxtime_seconds"),
            ban_count_decay_days: row.get("ban_count_decay_days"),
            fail_codes: parse_json_array(row.get::<Option<String>, _>("fail_codes")),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Crea un nueo perfil de rate limiting en la ase de datos.
    pub async fn create(pool: &SqlitePool, profile: NewRateLimitProfile) -> Result<Self, Error> {
        let now = Utc::now();
        let sql = "INSERT INTO rate_limit_profiles (
            name, description, max_retry, find_time_seconds, ban_time_seconds,
            bantime_increment, bantime_multipliers, bantime_maxtime_seconds,
            ban_count_decay_days, fail_codes, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *";
        query(sql)
            .bind(&profile.name)
            .bind(profile.description.unwrap_or_default())
            .bind(profile.max_retry.unwrap_or(5))
            .bind(profile.find_time_seconds.unwrap_or(600))
            .bind(profile.ban_time_seconds.unwrap_or(3600))
            .bind(profile.bantime_increment.unwrap_or(false))
            .bind(
                serde_json::to_string(&profile.bantime_multipliers.unwrap_or(vec![1, 2, 4, 8]))
                    .unwrap_or_default(),
            )
            .bind(profile.bantime_maxtime_seconds.unwrap_or(604800))
            .bind(profile.ban_count_decay_days.unwrap_or(30))
            .bind(
                serde_json::to_string(&profile.fail_codes.unwrap_or(vec![401, 403, 404]))
                    .unwrap_or_default(),
            )
            .bind(now)
            .bind(now)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    /// Lee un perfil de rate limiting por su ID.
    pub async fn read(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "SELECT * FROM rate_limit_profiles WHERE id = ?";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    /// Devuelve el recuento total de perfiles (para el dashboard/info).
    pub async fn read_info(pool: &SqlitePool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM rate_limit_profiles"
        } else {
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|row: SqliteRow| row.get::<i64, _>(0))
            .fetch_one(pool)
            .await
    }

    /// Lee perfiles con filtro LIKE y paginación.
    pub async fn read_paged(
        pool: &SqlitePool,
        params: &ReadRateLimitProfileParams,
    ) -> Result<Vec<Self>, Error> {
        let filters = vec![("name", &params.name)];
        let active_filters: Vec<(&str, String)> = filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();

        let mut sql = "SELECT * FROM rate_limit_profiles WHERE 1=1".to_string();
        for (col, _) in &active_filters {
            sql.push_str(&format!(" AND {col} LIKE ?"));
        }
        let sort_by = params.sort_by.as_deref().unwrap_or("id");
        if [
            "id",
            "name",
            "description",
            "max_retry",
            "ban_time_seconds",
            "bantime_increment",
            "ban_count_decay_days",
            "find_time_seconds",
            "bantime_maxtime_seconds",
            "created_at",
            "updated_at",
        ]
        .contains(&sort_by)
        {
            if params.asc.unwrap_or(true) {
                sql.push_str(&format!(" ORDER BY {sort_by} ASC"));
            } else {
                sql.push_str(&format!(" ORDER BY {sort_by} DESC"));
            }
        }
        sql.push_str(" LIMIT ? OFFSET ?");

        let mut query = query(&sql);
        for (_, value) in active_filters {
            query = query.bind(value);
        }
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT) as i32;
        let page = params.page.unwrap_or(DEFAULT_PAGE).max(1);
        let offset = ((page - 1) as i32) * limit;
        query
            .bind(limit)
            .bind(offset)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    /// Cuenta perfiles con el mismo filtro que `read_paged`.
    pub async fn count_paged(
        pool: &SqlitePool,
        params: &ReadRateLimitProfileParams,
    ) -> Result<i64, Error> {
        let filters = vec![("name", &params.name)];
        let active_filters: Vec<(&str, String)> = filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();
        let mut sql = "SELECT COUNT(*) total FROM rate_limit_profiles WHERE 1=1".to_string();
        for (col, _) in &active_filters {
            sql.push_str(&format!(" AND {col} LIKE ?"));
        }
        let mut query = query(&sql);
        for (_, value) in active_filters {
            query = query.bind(value);
        }
        query
            .map(|row: SqliteRow| {
                let count: i64 = row.get("total");
                count
            })
            .fetch_one(pool)
            .await
    }

    /// Actualiza un perfil de rate limiting existente.
    pub async fn update(pool: &SqlitePool, profile: UpdateRateLimitProfile) -> Result<Self, Error> {
        let now = Utc::now();
        let sql = "UPDATE rate_limit_profiles SET
            name = COALESCE(?, name),
            description = COALESCE(?, description),
            max_retry = COALESCE(?, max_retry),
            find_time_seconds = COALESCE(?, find_time_seconds),
            ban_time_seconds = COALESCE(?, ban_time_seconds),
            bantime_increment = COALESCE(?, bantime_increment),
            bantime_multipliers = COALESCE(?, bantime_multipliers),
            bantime_maxtime_seconds = COALESCE(?, bantime_maxtime_seconds),
            ban_count_decay_days = COALESCE(?, ban_count_decay_days),
            fail_codes = COALESCE(?, fail_codes),
            updated_at = ?
            WHERE id = ?
            RETURNING *";
        query(sql)
            .bind(&profile.name)
            .bind(&profile.description)
            .bind(profile.max_retry)
            .bind(profile.find_time_seconds)
            .bind(profile.ban_time_seconds)
            .bind(profile.bantime_increment)
            .bind(
                profile
                    .bantime_multipliers
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_default()),
            )
            .bind(profile.bantime_maxtime_seconds)
            .bind(profile.ban_count_decay_days)
            .bind(
                profile
                    .fail_codes
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_default()),
            )
            .bind(now)
            .bind(profile.id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    /// Elimina un perfil de rate limiting y devuelve el perfil eliminado.
    pub async fn delete(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "DELETE FROM rate_limit_profiles WHERE id = ? RETURNING *";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
}

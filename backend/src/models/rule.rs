//! # Modelo de reglas de filtrado
//!
//! Define [`Rule`], [`NewRule`], [`UpdateRule`] y [`CacheRule`].
//! Las reglas determinan si una petición HTTP debe ser permitida,
//! denegada y/o almacenada en la base de datos.
//!
//! [`CacheRule`] envuelve una [`Rule`] con un [`Regex`] precompilado
//! para la coincidencia rápida de URIs en memoria.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::NewRequest;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{Error, Row, SqlitePool, query, sqlite::SqliteRow};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub weight: i32,
    pub mode: String,
    pub pipeline: String,
    pub allow: bool,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub rate_limit_profile_name: Option<String>,
    pub active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RuleInfoCounts {
    pub total: i64,
    pub active: i64,
}

#[derive(Debug, Clone)]
pub struct CacheRule {
    pub rule: Rule,
    pub ip_address: Option<Regex>,
    pub protocol: Option<Regex>,
    pub fqdn: Option<Regex>,
    pub path: Option<Regex>,
    pub query: Option<Regex>,
    pub city_name: Option<Regex>,
    pub country_name: Option<Regex>,
    pub country_code: Option<Regex>,
    pub user_agent: Option<Regex>,
    pub method: Option<Regex>,
    pub referer: Option<Regex>,
    pub content_type: Option<Regex>,
    pub accept_language: Option<Regex>,
    pub x_request_id: Option<Regex>,
}

impl CacheRule {
    /// Construye un `CacheRule` desde una fila de SQLite.
    fn from_row(row: SqliteRow) -> Self {
        let rule = Rule::from_row(row);
        Self::from_rule(rule)
    }

    /// Construye un `CacheRule` con los regex precompilados desde una [`Rule`].
    pub fn from_rule(rule: Rule) -> Self {
        Self {
            rule: rule.clone(),
            ip_address: rule
                .ip_address
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            protocol: rule
                .protocol
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            fqdn: rule
                .fqdn
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            path: rule
                .path
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            query: rule
                .query
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            city_name: rule
                .city_name
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            country_name: rule
                .country_name
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            country_code: rule
                .country_code
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            user_agent: rule
                .user_agent
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            method: rule
                .method
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            referer: rule
                .referer
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            content_type: rule
                .content_type
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            accept_language: rule
                .accept_language
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
            x_request_id: rule
                .x_request_id
                .as_ref()
                .filter(|r| !r.is_empty())
                .and_then(|r| Regex::new(r).ok()),
        }
    }

    /// Lee todas las reglas activas desde la base de datos.
    pub async fn read_all_active(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT * FROM rules WHERE active = 1 ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub fn matches(&self, request: &NewRequest) -> bool {
        let check_match = |rule_regex: Option<&Regex>, request_value: Option<&String>| -> bool {
            match (rule_regex, request_value) {
                (Some(regex), Some(value)) => {
                    // Si la regla esta definida Y el valor existe, DEBE coincidir.
                    regex.is_match(value)
                },
                // Si la regla no esta definida (None), la condicion se cumple por defecto (true).
                // Si la regla esta definida pero el valor de la solicitud es None,
                // asumimos que el valor no existe y la regla no se puede aplicar (true).
                _ => true,
            }
        };
        // Si CUALQUIERA de las comprobaciones devuelve 'false', el metodo devuelve 'false'.
        check_match(self.ip_address.as_ref(), request.ip_address.as_ref())
            && check_match(self.protocol.as_ref(), request.protocol.as_ref())
            && check_match(self.fqdn.as_ref(), request.fqdn.as_ref())
            && check_match(self.path.as_ref(), request.path.as_ref())
            && check_match(self.query.as_ref(), request.query.as_ref())
            && check_match(self.city_name.as_ref(), request.city_name.as_ref())
            && check_match(self.country_name.as_ref(), request.country_name.as_ref())
            && check_match(self.country_code.as_ref(), request.country_code.as_ref())
            && check_match(self.user_agent.as_ref(), request.user_agent.as_ref())
            && check_match(self.method.as_ref(), request.method.as_ref())
            && check_match(self.referer.as_ref(), request.referer.as_ref())
            && check_match(self.content_type.as_ref(), request.content_type.as_ref())
            && check_match(
                self.accept_language.as_ref(),
                request.accept_language.as_ref(),
            )
            && check_match(self.x_request_id.as_ref(), request.x_request_id.as_ref())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRule {
    pub name: String,
    pub description: Option<String>,
    pub weight: Option<i32>,
    pub mode: Option<String>,
    pub pipeline: Option<String>,
    pub allow: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateRule {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub weight: i32,
    pub mode: String,
    pub pipeline: String,
    pub allow: bool,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ReadRuleParams {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mode: Option<String>,
    pub pipeline: Option<String>,
    pub weight: Option<i32>,
    pub allow: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub rate_limit_profile_name: Option<String>,
    pub active: Option<bool>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}

impl From<Rule> for CacheRule {
    fn from(val: Rule) -> Self {
        CacheRule::from_rule(val)
    }
}

impl Rule {
    fn from_row(row: SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            weight: row.get("weight"),
            mode: row.get("mode"),
            pipeline: row.get("pipeline"),
            allow: row.get("allow"),
            ip_address: row.get("ip_address"),
            protocol: row.get("protocol"),
            fqdn: row.get("fqdn"),
            path: row.get("path"),
            query: row.get("query"),
            city_name: row.get("city_name"),
            country_name: row.get("country_name"),
            country_code: row.get("country_code"),
            user_agent: row.get("user_agent"),
            method: row.get("method"),
            referer: row.get("referer"),
            content_type: row.get("content_type"),
            accept_language: row.get("accept_language"),
            x_request_id: row.get("x_request_id"),
            rate_limit_profile_id: row.try_get("rate_limit_profile_id").ok().flatten(),
            rate_limit_profile_name: row.try_get("rate_limit_profile_name").ok().flatten(),
            active: row.get("active"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn create(pool: &SqlitePool, rule: NewRule) -> Result<Self, Error> {
        let sql = "INSERT INTO rules (name, description, weight, mode, pipeline, allow,
            ip_address, protocol, fqdn, path, query, city_name, country_name,
            country_code, user_agent, method, referer, content_type,
            accept_language, x_request_id, rate_limit_profile_id,
            active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(&rule.name)
            .bind(rule.description.unwrap_or_default())
            .bind(rule.weight.unwrap_or(100))
            .bind(rule.mode.unwrap_or_else(|| "log_only".to_string()))
            .bind(rule.pipeline.unwrap_or_else(|| "waf".to_string()))
            .bind(rule.allow.unwrap_or(true))
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.user_agent)
            .bind(rule.method)
            .bind(rule.referer)
            .bind(rule.content_type)
            .bind(rule.accept_language)
            .bind(rule.x_request_id)
            .bind(rule.rate_limit_profile_id)
            .bind(rule.active.unwrap_or(true))
            .bind(now)
            .bind(now)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_info(pool: &SqlitePool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM rules"
        } else if info == "active" {
            "SELECT count(*) FROM rules WHERE active = 1"
        } else {
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|row: SqliteRow| row.get(0))
            .fetch_one(pool)
            .await
    }

    pub async fn read_info_all(pool: &SqlitePool) -> Result<RuleInfoCounts, Error> {
        let sql = "SELECT
            (SELECT count(*) FROM rules) as total,
            (SELECT count(*) FROM rules WHERE active = 1) as active";
        query(sql)
            .map(|row: SqliteRow| RuleInfoCounts {
                total: row.get("total"),
                active: row.get("active"),
            })
            .fetch_one(pool)
            .await
    }

    pub async fn update(pool: &SqlitePool, rule: UpdateRule) -> Result<Self, Error> {
        let sql = "UPDATE rules SET
                name = ?,
                description = ?,
                weight = ?,
                mode = ?,
                pipeline = ?,
                allow = ?,
                ip_address = ?,
                protocol = ?,
                fqdn = ?,
                path = ?,
                query = ?,
                city_name = ?,
                country_name = ?,
                country_code = ?,
                user_agent = ?,
                method = ?,
                referer = ?,
                content_type = ?,
                accept_language = ?,
                x_request_id = ?,
                rate_limit_profile_id = ?,
                active = ?,
                updated_at = ?
            WHERE id = ?
            RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(&rule.name)
            .bind(&rule.description)
            .bind(rule.weight)
            .bind(&rule.mode)
            .bind(&rule.pipeline)
            .bind(rule.allow)
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.user_agent)
            .bind(rule.method)
            .bind(rule.referer)
            .bind(rule.content_type)
            .bind(rule.accept_language)
            .bind(rule.x_request_id)
            .bind(rule.rate_limit_profile_id)
            .bind(rule.active)
            .bind(now)
            .bind(rule.id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn count_paged(pool: &SqlitePool, params: &ReadRuleParams) -> Result<i64, Error> {
        let like_filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query),
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
            ("name", &params.name),
            ("description", &params.description),
            ("mode", &params.mode),
        ];
        let active_like_filters: Vec<(&str, String)> = like_filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();
        let mut sql = "SELECT COUNT(*) total FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE 1=1".to_string();
        for (col, _) in &active_like_filters {
            sql.push_str(&format!(" AND {col} LIKE ?"));
        }
        // Boolean exact-match filters
        if params.allow.is_some() {
            sql.push_str(" AND allow = ?");
        }
        if params.pipeline.is_some() {
            sql.push_str(" AND pipeline = ?");
        }
        if params.active.is_some() {
            sql.push_str(" AND active = ?");
        }
        // Integer exact-match filter
        if params.weight.is_some() {
            sql.push_str(" AND weight = ?");
        }
        // Joined table filter (rate_limit_profile_name)
        if params.rate_limit_profile_name.is_some() {
            sql.push_str(" AND rate_limit_profiles.name LIKE ?");
        }
        let mut query = query(&sql);
        for (_, value) in active_like_filters {
            query = query.bind(value);
        }
        if let Some(val) = params.allow {
            query = query.bind(val);
        }
        if let Some(ref val) = params.pipeline {
            query = query.bind(val);
        }
        if let Some(val) = params.active {
            query = query.bind(val);
        }
        if let Some(val) = params.weight {
            query = query.bind(val);
        }
        if let Some(ref val) = params.rate_limit_profile_name {
            query = query.bind(val);
        }
        query
            .map(|row: SqliteRow| {
                let count: i64 = row.get("total");
                count
            })
            .fetch_one(pool)
            .await
    }

    pub async fn read_paged(
        pool: &SqlitePool,
        params: &ReadRuleParams,
    ) -> Result<Vec<Self>, Error> {
        let like_filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query),
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
            ("name", &params.name),
            ("description", &params.description),
            ("mode", &params.mode),
        ];
        let active_like_filters: Vec<(&str, String)> = like_filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();
        let mut sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE 1=1".to_string();
        for (col, _) in &active_like_filters {
            sql.push_str(&format!(" AND {col} LIKE ?"));
        }
        // Boolean exact-match filters
        if params.allow.is_some() {
            sql.push_str(" AND allow = ?");
        }
        if params.pipeline.is_some() {
            sql.push_str(" AND pipeline = ?");
        }
        if params.active.is_some() {
            sql.push_str(" AND active = ?");
        }
        // Integer exact-match filter
        if params.weight.is_some() {
            sql.push_str(" AND weight = ?");
        }
        // Joined table filter (rate_limit_profile_name)
        if let Some(ref _val) = params.rate_limit_profile_name {
            sql.push_str(" AND rate_limit_profiles.name LIKE ?");
        }
        let sort_by = params.sort_by.as_deref().unwrap_or("id");
        if [
            "id",
            "name",
            "description",
            "mode",
            "pipeline",
            "weight",
            "allow",
            "active",
            "ip_address",
            "protocol",
            "fqdn",
            "path",
            "city_name",
            "country_name",
            "country_code",
            "rate_limit_profile_name",
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
        for (_, value) in active_like_filters {
            query = query.bind(value);
        }
        if let Some(val) = params.allow {
            query = query.bind(val);
        }
        if let Some(ref val) = params.pipeline {
            query = query.bind(val);
        }
        if let Some(val) = params.active {
            query = query.bind(val);
        }
        if let Some(val) = params.weight {
            query = query.bind(val);
        }
        if let Some(ref val) = params.rate_limit_profile_name {
            query = query.bind(val);
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

    pub async fn read_all(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub async fn read(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE rules.id = ?";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn delete(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "DELETE FROM rules WHERE id = ? RETURNING *";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
}

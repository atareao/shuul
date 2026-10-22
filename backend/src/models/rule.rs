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
use std::fmt::Write;
use tracing::debug;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub weight: i32,
    pub mode: String,
    pub pipeline: String,
    pub allow: bool,
    pub is_tor: Option<bool>,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RuleInfoCounts {
    pub total: i64,
    pub active: i64,
}

#[derive(Debug, Clone)]
pub struct CacheRule {
    pub rule: Rule,
    pub is_tor: Option<bool>,
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
    /// Construye un `CacheRule` desde una fila de `SQLite`.
    #[allow(clippy::needless_pass_by_value)]
    fn from_row(row: SqliteRow) -> Self {
        let rule = Rule::from_row(&row);
        Self::from_rule(&rule)
    }

    /// Construye un `CacheRule` con los regex precompilados desde una [`Rule`].
    #[must_use]
    pub fn from_rule(rule: &Rule) -> Self {
        Self {
            rule: rule.clone(),
            is_tor: rule.is_tor,
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
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_all_active(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT * FROM rules WHERE active = 1 ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    /// Check if this rule matches a request.
    #[must_use]
    pub fn matches(&self, request: &NewRequest) -> bool {
        let check_tor = |rule_tor: Option<bool>, request_tor: Option<bool>| -> bool {
            // If rule has is_tor = Some(true), only match when request is Tor.
            // If rule has is_tor = None or Some(false), the filter is neutral.
            if rule_tor == Some(true) {
                request_tor == Some(true)
            } else {
                true
            }
        };
        // Si CUALQUIERA de las comprobaciones devuelve 'false', el metodo devuelve 'false'.
        self.check_match(
            "ip_address",
            self.ip_address.as_ref(),
            request.ip_address.as_ref(),
        ) && self.check_match(
            "protocol",
            self.protocol.as_ref(),
            request.protocol.as_ref(),
        ) && self.check_match("fqdn", self.fqdn.as_ref(), request.fqdn.as_ref())
            && self.check_match("path", self.path.as_ref(), request.path.as_ref())
            && self.check_match("query", self.query.as_ref(), request.query.as_ref())
            && self.check_match(
                "city_name",
                self.city_name.as_ref(),
                request.city_name.as_ref(),
            )
            && self.check_match(
                "country_name",
                self.country_name.as_ref(),
                request.country_name.as_ref(),
            )
            && self.check_match(
                "country_code",
                self.country_code.as_ref(),
                request.country_code.as_ref(),
            )
            && self.check_match(
                "user_agent",
                self.user_agent.as_ref(),
                request.user_agent.as_ref(),
            )
            && self.check_match("method", self.method.as_ref(), request.method.as_ref())
            && self.check_match("referer", self.referer.as_ref(), request.referer.as_ref())
            && self.check_match(
                "content_type",
                self.content_type.as_ref(),
                request.content_type.as_ref(),
            )
            && self.check_match(
                "accept_language",
                self.accept_language.as_ref(),
                request.accept_language.as_ref(),
            )
            && self.check_match(
                "x_request_id",
                self.x_request_id.as_ref(),
                request.x_request_id.as_ref(),
            )
            && check_tor(self.is_tor, request.is_tor)
    }

    /// Evalúa un filtro de la regla contra el valor de la petición.
    ///
    /// Es el **único** punto de decisión del matcheo: `matches()` lo invoca una
    /// vez por cada uno de los 14 filtros. El contrato de los tres brazos es:
    ///
    /// - `(Some(regex), Some(valor))` → decide el regex: `true` si casa, `false` si no;
    /// - `(Some(_), None)` → la regla **restringe** el campo y la petición **no** lo
    ///   trae: la condición NO se cumple (`false`) y se emite un `debug!` que nombra
    ///   el campo y la regla, de modo que el descarte no sea mudo;
    /// - `(None, _)` → la regla **no** restringe el campo: es **neutral** y se cumple
    ///   traiga la petición valor o no.
    fn check_match(&self, field: &str, filter: Option<&Regex>, value: Option<&String>) -> bool {
        match (filter, value) {
            (Some(regex), Some(v)) => regex.is_match(v),
            (Some(_), None) => {
                debug!(
                    field = field,
                    rule_id = self.rule.id,
                    rule_name = %self.rule.name,
                    "regla descartada: la petición no trae el campo restringido"
                );
                false
            },
            (None, _) => true,
        }
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
    pub is_tor: Option<bool>,
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
    pub is_tor: Option<bool>,
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
        Self::from_rule(&val)
    }
}

impl Rule {
    fn from_row(row: &SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            weight: row.get("weight"),
            mode: row.get("mode"),
            pipeline: row.get("pipeline"),
            allow: row.get("allow"),
            is_tor: row.get("is_tor"),
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

    /// Creates a new rule in the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn create(pool: &SqlitePool, rule: NewRule) -> Result<Self, Error> {
        let sql = "INSERT INTO rules (name, description, weight, mode, pipeline, allow,
            is_tor, ip_address, protocol, fqdn, path, query, city_name, country_name,
            country_code, user_agent, method, referer, content_type,
            accept_language, x_request_id, rate_limit_profile_id,
            active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(&rule.name)
            .bind(rule.description.unwrap_or_default())
            .bind(rule.weight.unwrap_or(100))
            .bind(rule.mode.unwrap_or_else(|| "log_only".to_string()))
            .bind(rule.pipeline.unwrap_or_else(|| "waf".to_string()))
            .bind(rule.allow.unwrap_or(true))
            .bind(rule.is_tor.unwrap_or(false))
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
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }

    /// Reads rule info (total or active count).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
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

    /// Reads all rule info counts.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
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

    /// Updates an existing rule.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn update(pool: &SqlitePool, rule: UpdateRule) -> Result<Self, Error> {
        let sql = "UPDATE rules SET
                name = ?,
                description = ?,
                weight = ?,
                mode = ?,
                pipeline = ?,
                allow = ?,
                is_tor = ?,
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
            .bind(rule.is_tor)
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
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }

    /// Counts rules with the given filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    #[allow(clippy::cast_possible_wrap)]
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
            let _ = write!(sql, " AND {col} LIKE ?");
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

    /// Reads rules with pagination and filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    #[allow(clippy::cast_possible_wrap)]
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
            let _ = write!(sql, " AND {col} LIKE ?");
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
                let _ = write!(sql, " ORDER BY {sort_by} ASC");
            } else {
                let _ = write!(sql, " ORDER BY {sort_by} DESC");
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
            .map(|row| Self::from_row(&row))
            .fetch_all(pool)
            .await
    }

    /// Reads all rules.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_all(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id ORDER BY weight ASC";
        query(sql)
            .map(|row| Self::from_row(&row))
            .fetch_all(pool)
            .await
    }

    /// Reads a rule by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE rules.id = ?";
        query(sql)
            .bind(id)
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }

    /// Deletes a rule by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn delete(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "DELETE FROM rules WHERE id = ? RETURNING *";
        query(sql)
            .bind(id)
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NewRequest;
    use chrono::Utc;
    use std::sync::{Arc, Mutex};

    fn make_request(is_tor: Option<bool>) -> NewRequest {
        NewRequest {
            ip_address: Some("8.8.8.8".to_string()),
            protocol: None,
            fqdn: None,
            path: None,
            query: None,
            city_name: None,
            country_name: None,
            country_code: None,
            user_agent: None,
            method: None,
            referer: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            is_tor,
            rule_id: None,
            created_at: Utc::now(),
        }
    }

    fn make_rule(is_tor: Option<bool>) -> CacheRule {
        let rule = Rule {
            id: 0,
            name: "test".to_string(),
            description: String::new(),
            weight: 100,
            mode: "enforce".to_string(),
            pipeline: "waf".to_string(),
            allow: false,
            is_tor,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: None,
            query: None,
            city_name: None,
            country_name: None,
            country_code: None,
            user_agent: None,
            method: None,
            referer: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            rate_limit_profile_id: None,
            rate_limit_profile_name: None,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        CacheRule::from_rule(&rule)
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = Some(true) → matches
    #[test]
    fn test_matches_tor_filter_match() {
        let rule = make_rule(Some(true));
        let request = make_request(Some(true));
        assert!(rule.matches(&request));
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = Some(false) → no match
    #[test]
    fn test_matches_tor_filter_no_match() {
        let rule = make_rule(Some(true));
        let request = make_request(Some(false));
        assert!(!rule.matches(&request));
    }

    /// Escenario: rule.is_tor = None, request.is_tor = Some(true) → matches (neutral)
    #[test]
    fn test_matches_tor_filter_neutral() {
        let rule = make_rule(None);
        let request = make_request(Some(true));
        assert!(rule.matches(&request));
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = None → no match (unknown = not Tor)
    #[test]
    fn test_matches_tor_filter_lazy_unknown() {
        let rule = make_rule(Some(true));
        let request = make_request(None);
        assert!(!rule.matches(&request));
    }

    /// Escenario: CacheRule::from_rule() copies is_tor field
    #[test]
    fn test_cache_rule_from_rule_copies_is_tor() {
        let rule = make_rule(Some(true));
        assert_eq!(rule.is_tor, Some(true));

        let rule = make_rule(None);
        assert_eq!(rule.is_tor, None);

        let rule = make_rule(Some(false));
        assert_eq!(rule.is_tor, Some(false));
    }

    // ── missing-header-fields: matcher (RED) ──

    /// Construye una `CacheRule` desde la base de `make_rule` y aplica los
    /// filtros que indique `configure` antes de compilar los regex.
    fn make_rule_with(configure: impl FnOnce(&mut Rule)) -> CacheRule {
        let mut cache = make_rule(None);
        configure(&mut cache.rule);
        CacheRule::from_rule(&cache.rule)
    }

    /// Construye un `NewRequest` desde la base de `make_request` y aplica los
    /// campos que indique `configure`.
    fn make_request_with(configure: impl FnOnce(&mut NewRequest)) -> NewRequest {
        let mut request = make_request(None);
        configure(&mut request);
        request
    }

    /// Escenario: Restricted field absent makes the rule not match.
    /// Regla con regex de `user_agent` y petición sin `user_agent` → NO casa.
    ///
    /// RED: hoy `check_match` devuelve `true` en `(Some(_), None)`, así que el
    /// test falla en aserción (no en compilación).
    #[test]
    fn test_matches_restricted_field_absent_does_not_match() {
        let rule = make_rule_with(|r| r.user_agent = Some("curl".to_string()));
        let request = make_request(None); // user_agent ausente

        assert!(
            !rule.matches(&request),
            "una regla que restringe user_agent no puede casar una petición sin esa cabecera"
        );
    }

    /// Escenario: Restricted field present and matching.
    /// Guardián: decide el regex; pasa ya y debe seguir pasando tras el GREEN.
    #[test]
    fn test_matches_restricted_field_present_and_matching() {
        let rule = make_rule_with(|r| r.user_agent = Some(r"curl/8\.5".to_string()));
        let request = make_request_with(|q| q.user_agent = Some("curl/8.5".to_string()));

        assert!(
            rule.matches(&request),
            "el regex debe casar cuando el valor está presente y coincide"
        );
    }

    /// Escenario: Restricted field present and not matching.
    /// Guardián: con el valor presente decide el regex; pasa ya y debe seguir
    /// pasando tras el GREEN.
    #[test]
    fn test_matches_restricted_field_present_and_not_matching() {
        let rule = make_rule_with(|r| r.user_agent = Some(r"curl/8\.5".to_string()));
        let request = make_request_with(|q| q.user_agent = Some("Mozilla/5.0".to_string()));

        assert!(
            !rule.matches(&request),
            "un regex que no casa con el valor presente → la regla NO casa"
        );
    }

    /// Escenario: One absent field discards a rule that matches on the others.
    /// `path` casa pero `x_request_id` está ausente → la regla NO casa.
    ///
    /// RED: hoy el campo ausente es neutral y la regla casa → falla en aserción.
    #[test]
    fn test_matches_absent_field_discards_rule_matching_others() {
        let rule = make_rule_with(|r| {
            r.path = Some(r"^/admin$".to_string());
            r.x_request_id = Some(r"^req-".to_string());
        });
        let request = make_request_with(|q| q.path = Some("/admin".to_string()));

        assert!(
            !rule.matches(&request),
            "un único campo restringido y ausente descarta la regla aunque los demás casen"
        );
    }

    /// Escenario: An unrestricted field is neutral.
    /// Guardián: la regla sólo restringe `path`; los campos que no restringe
    /// (`user_agent`, `referer`, `x_request_id`, ausentes) son neutrales → casa.
    /// Pasa ya y vigila que el GREEN no toque la neutralidad de `(None, _)`.
    #[test]
    fn test_matches_unrestricted_fields_are_neutral() {
        let rule = make_rule_with(|r| r.path = Some(r"^/admin$".to_string()));
        let request = make_request_with(|q| q.path = Some("/admin".to_string()));

        assert!(
            rule.matches(&request),
            "los campos que la regla no restringe deben ser neutrales aunque falten"
        );
    }

    /// Escenario: A present but empty header counts as absent.
    /// `from_request` colapsa la cabecera vacía a `None` y la regla NO casa.
    ///
    /// RED: el colapso a `None` ya pasa, pero la regla casa por el brazo
    /// `(Some(_), None)` → la segunda aserción falla.
    #[test]
    fn test_matches_present_but_empty_header_counts_as_absent() {
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::REFERER, http::HeaderValue::from_static(""));
        let request = NewRequest::from_request(&headers, None, None);

        assert!(
            request.referer.is_none(),
            "una cabecera presente pero vacía debe colapsar a None"
        );

        let rule = make_rule_with(|r| r.referer = Some("https://example.com".to_string()));
        assert!(
            !rule.matches(&request),
            "una cabecera vacía cuenta como ausente → la regla NO casa"
        );
    }

    // ── captura de logs (copia privada del patrón de `journald_reader`) ──

    #[derive(Clone, Default)]
    struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Ejecuta `f` con un subscriber a nivel DEBUG que graba cada evento y
    /// devuelve lo escrito. Es el único modo de observar el `debug!` del
    /// matcher.
    fn capture_debug<F: FnOnce()>(f: F) -> String {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let writer = CapturedLog(Arc::clone(&sink));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || CapturedLog(Arc::clone(&writer.0)))
            .finish();

        tracing::subscriber::with_default(subscriber, f);

        let bytes = sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        String::from_utf8_lossy(&bytes).to_string()
    }

    /// Escenario: Restricted field absent makes the rule not match (diagnóstico).
    /// Un match descartado por campo ausente emite un `debug!` que nombra el
    /// campo y la regla.
    ///
    /// RED: hoy no se emite nada → falla en aserción. No se fija el texto
    /// literal del mensaje: sólo que nombre el campo y algo que identifique la
    /// regla (su nombre o su id).
    #[test]
    fn test_matches_absent_field_emits_debug_diagnostic() {
        let rule = make_rule_with(|r| {
            r.id = 4242;
            r.name = "regla-diagnostico".to_string();
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request(None);

        let log = capture_debug(|| {
            let _ = rule.matches(&request);
        });

        assert!(
            log.contains("user_agent"),
            "el diagnóstico debe nombrar el campo ausente; log capturado: {log:?}"
        );
        assert!(
            log.contains("regla-diagnostico") || log.contains("4242"),
            "el diagnóstico debe identificar la regla (nombre o id); log capturado: {log:?}"
        );
    }

    /// Escenario: A legitimate regex no-match is not diagnosed as an absent field.
    /// Con el valor presente y un regex que no casa NO se emite el `debug!` de
    /// campo ausente.
    ///
    /// Guardián: hoy pasa trivialmente (no hay diagnóstico alguno) y debe
    /// seguir pasando tras el GREEN.
    #[test]
    fn test_matches_legitimate_regex_no_match_emits_no_diagnostic() {
        let rule = make_rule_with(|r| {
            r.id = 4242;
            r.name = "regla-diagnostico".to_string();
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request_with(|q| q.user_agent = Some("Mozilla/5.0".to_string()));

        let log = capture_debug(|| {
            let _ = rule.matches(&request);
        });

        assert!(
            !log.contains("user_agent"),
            "no debe diagnosticar campo ausente cuando el valor está presente; log: {log:?}"
        );
        assert!(
            !log.contains("regla-diagnostico"),
            "no debe nombrar la regla en un no-match legítimo por regex; log: {log:?}"
        );
    }
}

use crate::models::request::NewRequest;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{
    Error, Row,
    postgres::{PgPool, PgRow},
    query,
};
use std::convert::Into;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    pub id: i32,
    pub weight: i32,
    pub allow: bool,
    pub store: bool,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
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
}

impl CacheRule {
    fn from_row(row: PgRow) -> Self {
        let rule = Rule::from_row(row);
        Self::from_rule(rule)
    }
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
        }
    }

    pub async fn read_all_active(pool: &PgPool) -> Result<Vec<CacheRule>, Error> {
        let sql = "SELECT * FROM rules WHERE active = TRUE ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub fn matches(&self, request: &NewRequest) -> bool {
        let check_match = |rule_regex: Option<&Regex>, request_value: Option<&String>| -> bool {
            match (rule_regex, request_value) {
                (Some(regex), Some(value)) => {
                    // Si la regla está definida Y el valor existe, DEBE coincidir.
                    regex.is_match(value)
                }
                // Si la regla no está definida (None), la condición se cumple por defecto (true).
                // Si la regla está definida pero el valor de la solicitud es None,
                // asumimos que el valor no existe y la regla no se puede aplicar (true).
                _ => true,
            }
        };
        // Si CUALQUIERA de las comprobaciones devuelve 'false', el método devuelve 'false'.
        check_match(self.ip_address.as_ref(), request.ip_address.as_ref())
            && check_match(self.protocol.as_ref(), request.protocol.as_ref())
            && check_match(self.fqdn.as_ref(), request.fqdn.as_ref())
            && check_match(self.path.as_ref(), request.path.as_ref())
            && check_match(self.query.as_ref(), request.query.as_ref())
            && check_match(self.city_name.as_ref(), request.city_name.as_ref())
            && check_match(self.country_name.as_ref(), request.country_name.as_ref())
            && check_match(self.country_code.as_ref(), request.country_code.as_ref())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRule {
    pub weight: i32,
    pub allow: bool,
    pub store: bool,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateRule {
    pub id: i32,
    pub weight: i32,
    pub allow: bool,
    pub store: bool,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub active: bool,
}
#[derive(Debug, Deserialize)]
pub struct ReadRuleParams {
    pub id: Option<i32>,
    pub weight: Option<i32>,
    pub allow: Option<bool>,
    pub store: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub active: Option<bool>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}
use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;

impl Into<CacheRule> for Rule {
    fn into(self) -> CacheRule {
        CacheRule::from_rule(self)
    }
}

impl Rule {
    fn from_row(row: PgRow) -> Self {
        Self {
            id: row.get("id"),
            weight: row.get("weight"),
            allow: row.get("allow"),
            store: row.get("store"),
            ip_address: row.get("ip_address"),
            protocol: row.get("protocol"),
            fqdn: row.get("fqdn"),
            path: row.get("path"),
            query: row.get("query"),
            city_name: row.get("city_name"),
            country_name: row.get("country_name"),
            country_code: row.get("country_code"),
            active: row.get("active"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn create(pool: &PgPool, rule: NewRule) -> Result<Rule, Error> {
        let sql = "INSERT INTO rules (weight, allow, store,
            ip_address, protocol, fqdn, path, query, city_name, country_name,
            country_code, active, created_at, updated_at) VALUES ($1, $2, $3,
            $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.weight)
            .bind(rule.allow)
            .bind(rule.store)
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.active)
            .bind(now)
            .bind(now)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_info(pool: &PgPool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM rules"
        } else if info == "active" {
            "SELECT count(*) FROM rules WHERE active = true"
        } else {
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|cp_row: PgRow| cp_row.get(0))
            .fetch_one(pool)
            .await
    }

    pub async fn update(pool: &PgPool, rule: UpdateRule) -> Result<Rule, Error> {
        let sql = "UPDATE rules set
                weight = $1,
                allow = $2,
                store = $3,
                ip_address = $4,
                protocol = $5,
                fqdn = $6,
                path = $7,
                query = $8,
                city_name = $9,
                country_name = $10,
                country_code = $11,
                active = $12,
                updated_at = $13
            WHERE id = $14
            RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.weight)
            .bind(rule.allow)
            .bind(rule.store)
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.active)
            .bind(now)
            .bind(rule.id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn count_paged(pool: &PgPool, params: &ReadRuleParams) -> Result<i64, Error> {
        let filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query), // Mapea 'query_for_request' a 'query'
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
        ];
        let active_filters: Vec<(&str, String)> = filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.to_string())))
            .collect();
        let mut sql = "SELECT COUNT(*) total FROM rules WHERE 1=1".to_string();
        for (i, (col, _)) in active_filters.iter().enumerate() {
            let param_index = i + 1;
            sql.push_str(&format!(" AND {} LIKE ${}", col, param_index));
        }
        let mut query = query(&sql);
        for (_, value) in active_filters {
            query = query.bind(value);
        }
        query
            .map(|row: PgRow| {
                let count: i64 = row.get("total");
                count
            })
            .fetch_one(pool)
            .await
    }

    pub async fn read_paged(pool: &PgPool, params: &ReadRuleParams) -> Result<Vec<Rule>, Error> {
        let filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query), // Mapea 'query_for_request' a 'query'
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
        ];
        let active_filters: Vec<(&str, String)> = filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.to_string())))
            .collect();
        let mut sql = "SELECT * FROM rules WHERE 1=1".to_string();
        for (i, (col, _)) in active_filters.iter().enumerate() {
            let param_index = i + 1;
            sql.push_str(&format!(" AND {} LIKE ${}", col, param_index));
        }
        let limit_index = active_filters.len() + 1;
        let offset_index = limit_index + 1;
        if let Some(sort_by) = params.sort_by.as_ref()
            && [
                "ip_address",
                "protocol",
                "fqdn",
                "path",
                "city_name",
                "country_name",
                "country_code",
            ]
            .contains(&sort_by.as_str())
        {
            if params.asc.unwrap_or(true) {
                sql.push_str(&format!(" ORDER BY {} ASC", sort_by));
            } else {
                sql.push_str(&format!(" ORDER BY {} DESC", sort_by));
            }
        }
        sql.push_str(&format!(" LIMIT ${} OFFSET ${}", limit_index, offset_index));
        let mut query = query(&sql);
        for (_, value) in active_filters {
            query = query.bind(value);
        }
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT) as i32;
        let offset = ((params.page.unwrap_or(DEFAULT_PAGE) - 1) as i32) * limit;
        query
            .bind(limit)
            .bind(offset)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    pub async fn read(pool: &PgPool, id: i32) -> Result<Rule, Error> {
        let sql = "SELECT * FROM rules WHERE id = $1";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_all_active(pool: &PgPool) -> Result<Vec<Rule>, Error> {
        let sql = "SELECT * FROM rules WHERE active = TRUE ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub async fn delete(pool: &PgPool, id: i32) -> Result<Rule, Error> {
        let sql = "DELETE FROM rules WHERE id = $1 RETURNING *";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
}

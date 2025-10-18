use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use sqlx::{postgres::{PgPool, PgRow}, query, Row, Error};
use regex::Regex;
use tracing::debug;

use crate::models::NewRecord;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule{
    pub id: i32,
    pub weight: i32,
    pub allow: bool,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRule{
    pub weight: i32,
    pub allow: bool,
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
pub struct UpdateRule{
    pub id: i32,
    pub weight: i32,
    pub allow: bool,
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
use crate::constants::DEFAULT_PAGE;
use crate::constants::DEFAULT_LIMIT;

impl Rule{
    fn from_row(row: PgRow) -> Self{
        Self{
            id: row.get("id"),
            weight: row.get("weight"),
            allow: row.get("allow"),
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

    pub fn matches(&self, record: &NewRecord) -> bool {
        debug!("Matching rule: {:?} for record: {:?}", self, record);
        if let Some(regex) = &self.ip_address &&
                let Some(value) = record.ip_address.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.protocol && 
                let Some(value) = record.protocol.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.fqdn &&
                let Some(value) = record.fqdn.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.path &&
                let Some(value) = record.path.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.query &&
                let Some(value) = record.query.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.city_name &&
                let Some(value) = record.city_name.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.country_name &&
                let Some(value) = record.country_name.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        if let Some(regex) = &self.country_code &&
                let Some(value) = record.country_code.as_ref() &&
                !Regex::new(regex).unwrap().is_match(value) {
            return false;
        }
        true
    }

    pub async fn create( pool: &PgPool, rule: NewRule) -> Result<Rule, Error> {

        let sql = "INSERT INTO rules (weight, allow, ip_address,
            protocol, fqdn, path, query, city_name, country_name, country_code,
            active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, $12, $13) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.weight)
            .bind(rule.allow)
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

    pub async fn update(pool: &PgPool, rule: UpdateRule) -> Result<Rule, Error>{
        let sql = "UPDATE rules set
                weight = $1,
                allow = $2,
                ip_address = $3,
                protocol = $4,
                fqdn = $5,
                path = $6,
                query = $7,
                city_name = $8,
                country_name = $9,
                country_code = $10,
                active = $11,
                updated_at = $12
            WHERE id = $13
            RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.weight)
            .bind(rule.allow)
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

    pub async fn count_paged(
        pool: &PgPool,
        params: &ReadRuleParams,

    ) -> Result<i64, Error> {
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

    pub async fn read_paged(
        pool: &PgPool,
        params: &ReadRuleParams,
    ) -> Result<Vec<Rule>, Error> {
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
        let sort_by = params.sort_by.as_deref().unwrap_or("ip_address");
        if ["ip_address", "protocol", "fqdn", "path", "city_name",
                "country_name", "country_code"].contains(&sort_by) {
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

    pub async fn read_all(pool: &PgPool) -> Result<Vec<Rule>, Error> {
        let sql = "SELECT * FROM rules";
        query(sql)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    pub async fn read_all_active(pool: &PgPool) -> Result<Vec<Rule>, Error> {
        let sql = "SELECT * FROM rules WHERE active = TRUE ORDER BY weight ASC";
        query(sql)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
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

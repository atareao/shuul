use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{
    Error, Row,
    postgres::{PgPool, PgRow},
    query,
};
use tracing::debug;

use crate::models::NewRecord;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ignored {
    pub id: i32,
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
pub struct NewIgnored {
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
pub struct UpdateIgnored {
    pub id: i32,
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
pub struct ReadIgnoredParams {
    pub id: Option<i32>,
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

impl Ignored {
    fn from_row(row: PgRow) -> Self {
        Self {
            id: row.get("id"),
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
        debug!("Matching ignored: {:?} for record: {:?}", self, record);
        if let Some(regex) = &self.ip_address
            && let Some(value) = record.ip_address.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.protocol
            && let Some(value) = record.protocol.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.fqdn
            && let Some(value) = record.fqdn.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.path
            && let Some(value) = record.path.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.query
            && let Some(value) = record.query.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.city_name
            && let Some(value) = record.city_name.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.country_name
            && let Some(value) = record.country_name.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        if let Some(regex) = &self.country_code
            && let Some(value) = record.country_code.as_ref()
            && !Regex::new(regex).unwrap().is_match(value)
        {
            return false;
        }
        true
    }

    pub async fn create(pool: &PgPool, ignored: NewIgnored) -> Result<Ignored, Error> {
        let sql = "INSERT INTO ignored (ip_address, protocol, fqdn,
            path, query, city_name, country_name, country_code, active,
            created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
        $10, $11) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(ignored.ip_address)
            .bind(ignored.protocol)
            .bind(ignored.fqdn)
            .bind(ignored.path)
            .bind(ignored.query)
            .bind(ignored.city_name)
            .bind(ignored.country_name)
            .bind(ignored.country_code)
            .bind(ignored.active)
            .bind(now)
            .bind(now)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_info(pool: &PgPool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM ignored"
        }else if info == "active" {
            "SELECT count(*) FROM ignored WHERE active = true"
        }else{
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|cp_row: PgRow| cp_row.get(0))
            .fetch_one(pool)
            .await
    }

    pub async fn update(pool: &PgPool, ignored: UpdateIgnored) -> Result<Ignored, Error> {
        let sql = "UPDATE ignored set
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
            .bind(ignored.ip_address)
            .bind(ignored.protocol)
            .bind(ignored.fqdn)
            .bind(ignored.path)
            .bind(ignored.query)
            .bind(ignored.city_name)
            .bind(ignored.country_name)
            .bind(ignored.country_code)
            .bind(ignored.active)
            .bind(now)
            .bind(ignored.id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn count_paged(pool: &PgPool, params: &ReadIgnoredParams) -> Result<i64, Error> {
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
        let mut sql = "SELECT COUNT(*) total FROM ignored WHERE 1=1".to_string();
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

    pub async fn read_paged(pool: &PgPool, params: &ReadIgnoredParams) -> Result<Vec<Ignored>, Error> {
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
        let mut sql = "SELECT * FROM ignored WHERE 1=1".to_string();
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

    pub async fn read(pool: &PgPool, id: i32) -> Result<Ignored, Error> {
        let sql = "SELECT * FROM ignored WHERE id = $1";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_all_active(pool: &PgPool) -> Result<Vec<Ignored>, Error> {
        let sql = "SELECT * FROM ignored WHERE active = TRUE ORDER BY weight ASC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub async fn delete(pool: &PgPool, id: i32) -> Result<Ignored, Error> {
        let sql = "DELETE FROM ignored WHERE id = $1 RETURNING *";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
}

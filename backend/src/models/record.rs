use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use sqlx::{postgres::{PgPool, PgRow}, query, Row, Error};
use http::Uri;
use maxminddb::Reader;
use tracing::debug;

use crate::models::IPData;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Record{
    id: i32,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub rule_id: Option<i32>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRecord{
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub rule_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ReadRecordParams {
    pub id: Option<i32>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}
use crate::constants::DEFAULT_PAGE;
use crate::constants::DEFAULT_LIMIT;

impl NewRecord{
    pub fn from_request(headers: &http::HeaderMap, maxmind_db: &Reader<Vec<u8>>) -> Self {
        let method = headers.get("x-forwarded-method")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("method from proxy: {:?}", method);
        let protocol = headers.get("x-forwarded-proto")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("protocol from proxy: {:?}", protocol);
        let host = headers.get("x-forwarded-host")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("host from proxy: {:?}", host);
        let uri = headers.get("x-forwarded-uri")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("")
            .parse::<Uri>()
            .unwrap_or_default();
        debug!("uri from proxy: {:?}", uri);
        let ip = headers.get("x-forwarded-for")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("ip from proxy: {:?}", ip);
        let ip_data = IPData::complete(maxmind_db, ip);
        debug!("ip data: {:?}", &ip_data);
        let ip_address = if ip.is_empty() { None } else { Some(ip.to_string()) };
        let protocol = if protocol.is_empty() { None } else { Some(protocol.to_string()) };
        let fqdn = if host.is_empty() { None } else { Some(host.to_string()) };
        let path = if uri.path().is_empty() { None } else { Some(uri.path().to_string()) };
        let query = uri.query().and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
        let city_name = ip_data.city_name.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
        let country_name = ip_data.country_name.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
        let country_code = ip_data.country_code.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
        NewRecord {
            ip_address,
            protocol,
            fqdn,
            path,
            query,
            city_name,
            country_name,
            country_code,
            rule_id: None,
        }
    }
}

impl Record{
    fn from_row(row: PgRow) -> Self{
        Self{
            id: row.get("id"),
            ip_address: row.get("ip_address"),
            protocol: row.get("protocol"),
            fqdn: row.get("fqdn"),
            path: row.get("path"),
            query: row.get("query"),
            city_name: row.get("city_name"),
            country_name: row.get("country_name"),
            country_code: row.get("country_code"),
            rule_id: row.get("rule_id"),
            created_at: row.get("created_at"),
        }
    }

    pub async fn create( pool: &PgPool, record: NewRecord) -> Result<Record, Error> {

        let sql = "INSERT INTO records (ip_address, protocol, fqdn, path, city_name, country_name, country_code, rule_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(record.ip_address)
            .bind(record.protocol)
            .bind(record.fqdn)
            .bind(record.path)
            .bind(record.city_name)
            .bind(record.country_name)
            .bind(record.country_code)
            .bind(record.rule_id)
            .bind(now)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read(pool: &PgPool, id: i32) -> Result<Record, Error> {
        let sql = "SELECT * FROM records WHERE id = $1";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_info(pool: &PgPool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM records"
        }else if info == "filtered" {
            "SELECT count(*) FROM records WHERE rule_id IS NOT NULL"
        }else{
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|cp_row: PgRow| cp_row.get(0))
            .fetch_one(pool)
            .await
    }

    pub async fn count_paged(
        pool: &PgPool,
        params: &ReadRecordParams,

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
        let mut sql = "SELECT COUNT(*) total FROM records WHERE 1=1".to_string();
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
        params: &ReadRecordParams,
    ) -> Result<Vec<Record>, Error> {
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
        let mut sql = "SELECT * FROM records WHERE 1=1".to_string();
        for (i, (col, _)) in active_filters.iter().enumerate() {
            let param_index = i + 1;
            sql.push_str(&format!(" AND {} LIKE ${}", col, param_index));
        }
        let limit_index = active_filters.len() + 1;
        let offset_index = limit_index + 1;
        let sort_by = params.sort_by.as_deref().unwrap_or("created_at");
        if ["created_at", "ip_address", "protocol", "fqdn", "path",
                "city_name", "country_name", "country_code"].contains(&sort_by) {
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

    pub async fn delete_before(pool: &PgPool, days: i32) -> Result<Vec<Record>, Error> {
        let sql = "DELETE FROM records WHERE created_at > $1 RETURNING *";
        let now = Utc::now()
            .checked_sub_signed(chrono::Duration::days(days.into())).unwrap();
        query(sql)
            .bind(now)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    pub async fn group_by_country(pool: &PgPool) -> Result<Vec<(String, i64)>, Error> {
        let sql = "SELECT country_name, COUNT(*) as count FROM records GROUP BY country_name";
        query(sql)
            .map(|row: PgRow| {
                let country_name: String = row.get("country_name");
                let count: i64 = row.get("count");
                (country_name, count)
            })
            .fetch_all(pool)
            .await
    }

    pub async fn group_by_city(pool: &PgPool) -> Result<Vec<(String, i64)>, Error> {
        let sql = "SELECT city_name, COUNT(*) as count FROM records GROUP BY city_name";
        query(sql)
            .map(|row: PgRow| {
                let country_name: String = row.get("city_name");
                let count: i64 = row.get("count");
                (country_name, count)
            })
            .fetch_all(pool)
            .await
    }

    pub async fn count_all(pool: &PgPool) -> Result<i32, Error> {
        let sql = "SELECT COUNT(*) as count FROM records";
        let row: (i32,) = query(sql)
            .map(|row: PgRow| {
                let count: i32 = row.get("count");
                (count,)
            })
            .fetch_one(pool)
            .await?;
        Ok(row.0)
    }
}

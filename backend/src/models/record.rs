use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use sqlx::{postgres::{PgPool, PgRow}, query, Row, Error};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Record{
    id: i32,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    rule_id: i32,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRecord{
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub rule_id: i32,
}

impl Record{
    fn from_row(row: PgRow) -> Self{
        Self{
            id: row.get("id"),
            ip_address: row.get("ip_address"),
            protocol: row.get("protocol"),
            fqdn: row.get("fqdn"),
            path: row.get("path"),
            city_name: row.get("city_name"),
            country_name: row.get("country_name"),
            country_code: row.get("country_code"),
            rule_id: row.get("rule_id"),
            created_at: row.get("created_at"),
        }
    }
}


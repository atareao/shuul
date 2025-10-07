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

    pub async fn create( pool: &PgPool, record: Record) -> Result<Record, Error> {

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
    pub async fn read_by_ip(pool: &PgPool, ip_address: &str) -> Result<Record, Error> {
        let sql = "SELECT * FROM records WHERE ip = $1";
        query(sql)
            .bind(ip_address)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
    pub async fn read_by_fqdn(pool: &PgPool, fqdn: &str) -> Result<Record, Error> {
        let sql = "SELECT * FROM records WHERE fqdn = $1";
        query(sql)
            .bind(fqdn)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
    pub async fn read_all(pool: &PgPool) -> Result<Vec<Record>, Error> {
        let sql = "SELECT * FROM records";
        query(sql)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    pub async fn select_by_country(pool: &PgPool) -> Result<Vec<(String, i64)>, Error> {
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

    pub async fn select_by_city(pool: &PgPool) -> Result<Vec<(String, i64)>, Error> {
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

}


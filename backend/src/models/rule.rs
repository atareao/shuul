use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use sqlx::{postgres::{PgPool, PgRow}, query, Row, Error};
use regex::Regex;

use crate::models::NewRecord;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule{
    pub id: i32,
    pub norder: i32,
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
    pub norder: i32,
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

impl Rule{
    fn from_row(row: PgRow) -> Self{
        Self{
            id: row.get("id"),
            norder: row.get("norder"),
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
        if let Some(regex) = &self.ip_address &&
                let Some(value) = record.ip_address.as_ref() &&
                Regex::new(regex).unwrap().is_match(value) {
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

    pub async fn create( pool: &PgPool, rule: Rule) -> Result<Rule, Error> {

        let sql = "INSERT INTO rules (norder, allow, ip_address,
            protocol, fqdn, path, qury, city_name, country_name, country_code,
            active, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, $12, $13) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.norder)
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

    pub async fn update(pool: &PgPool, rule: Rule) -> Result<Rule, Error>{
        let sql = "UPDATE rules set
                norder = $1,
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
            .bind(rule.norder)
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
        let sql = "SELECT * FROM rules WHERE active = TRUE ORDER BY norder ASC";
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

use chrono::{DateTime, Utc};
use http::Uri;
use maxminddb::Reader;
use serde::{Deserialize, Serialize};
use sqlx::{
    Error, Row, FromRow,
    postgres::{PgPool, PgRow},
    query,
    query_as,
};
use tracing::debug;

use crate::models::IPData;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct Request {
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
pub struct NewRequest {
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

#[derive(Debug, Deserialize)]
pub struct ReadRequestParams {
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
use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;

impl NewRequest {
    pub fn from_request(headers: &http::HeaderMap, maxmind_db: &Reader<Vec<u8>>) -> Self {
        let method = headers
            .get("x-forwarded-method")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("method from proxy: {:?}", method);
        let protocol = headers
            .get("x-forwarded-proto")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("protocol from proxy: {:?}", protocol);
        let host = headers
            .get("x-forwarded-host")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("host from proxy: {:?}", host);
        let uri = headers
            .get("x-forwarded-uri")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("")
            .parse::<Uri>()
            .unwrap_or_default();
        debug!("uri from proxy: {:?}", uri);
        let ip = headers
            .get("x-forwarded-for")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        debug!("ip from proxy: {:?}", ip);
        let ip_data = IPData::complete(maxmind_db, ip);
        debug!("ip data: {:?}", &ip_data);
        let ip_address = if ip.is_empty() {
            None
        } else {
            Some(ip.to_string())
        };
        let protocol = if protocol.is_empty() {
            None
        } else {
            Some(protocol.to_string())
        };
        let fqdn = if host.is_empty() {
            None
        } else {
            Some(host.to_string())
        };
        let path = if uri.path().is_empty() {
            None
        } else {
            Some(uri.path().to_string())
        };
        let query = uri.query().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });
        debug!("query from proxy: {:?}", query);
        let city_name = ip_data.city_name.and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });
        let country_name = ip_data.country_name.and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });
        let country_code = ip_data.country_code.and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });
        NewRequest {
            ip_address,
            protocol,
            fqdn,
            path,
            query,
            city_name,
            country_name,
            country_code,
            rule_id: None,
            created_at: Utc::now(),
        }
    }
}

impl Request {
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
            rule_id: row.get("rule_id"),
            created_at: row.get("created_at"),
        }
    }

    pub async fn create(pool: &PgPool, request: NewRequest) -> Result<Request, Error> {
        let sql = "INSERT INTO requests (ip_address, protocol, fqdn, path, query, city_name, country_name, country_code, rule_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *";
        query(sql)
            .bind(request.ip_address)
            .bind(request.protocol)
            .bind(request.fqdn)
            .bind(request.path)
            .bind(request.query)
            .bind(request.city_name)
            .bind(request.country_name)
            .bind(request.country_code)
            .bind(request.rule_id)
            .bind(request.created_at)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read(pool: &PgPool, id: i32) -> Result<Request, Error> {
        let sql = "SELECT * FROM requests WHERE id = $1";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_info(pool: &PgPool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM requests"
        } else if info == "filtered" {
            "SELECT count(*) FROM requests WHERE rule_id IS NOT NULL"
        } else {
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|cp_row: PgRow| cp_row.get(0))
            .fetch_one(pool)
            .await
    }

    pub async fn create_bulk(pool: &PgPool, requests: Vec<NewRequest>) -> Result<Vec<Request>, Error> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let num_columns = 10; // Número de columnas a insertar: ip_address, protocol, ..., created_at
        let mut placeholders = String::new();
        //let mut all_bindings = Vec::new(); // Vector para almacenar todos los valores a enlazar
        for (i, _request) in requests.iter().enumerate() {
            let start_index = i * num_columns + 1;
            // Genera ($1, $2, $3, ... $10), ($11, $12, ... $20), etc.
            placeholders.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                start_index,
                start_index + 1,
                start_index + 2,
                start_index + 3,
                start_index + 4,
                start_index + 5,
                start_index + 6,
                start_index + 7,
                start_index + 8,
                start_index + 9
            ));
            if i < requests.len() - 1 {
                placeholders.push_str(", ");
            }
        }
        let base_sql = "INSERT INTO requests (ip_address, protocol,
        fqdn, path, query, city_name, country_name, country_code, rule_id, created_at) VALUES ";
        let full_sql = format!("{} {} RETURNING *", base_sql, placeholders);

        // 2. Ejecutar la consulta con Transaction para el binding
        let mut transaction = pool.begin().await?;

        // Necesitamos usar `query_as` o `query` para el binding dinámico.
        let mut query_builder = query_as::<_, Request>(&full_sql);

        for request in &requests {
            query_builder = query_builder
                .bind(&request.ip_address)
                .bind(&request.protocol)
                .bind(&request.fqdn)
                .bind(&request.path)
                .bind(&request.query)
                .bind(&request.city_name)
                .bind(&request.country_name)
                .bind(&request.country_code)
                .bind(request.rule_id)
                .bind(&request.created_at);
        }

        let created_requests = query_builder.fetch_all(&mut *transaction).await?;

        transaction.commit().await?;

        Ok(created_requests)
    }

    pub async fn count_paged(pool: &PgPool, params: &ReadRequestParams) -> Result<i64, Error> {
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
        let mut sql = "SELECT COUNT(*) total FROM requests WHERE 1=1".to_string();
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
        params: &ReadRequestParams,
    ) -> Result<Vec<Request>, Error> {
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
        let mut sql = "SELECT * FROM requests WHERE 1=1".to_string();
        for (i, (col, _)) in active_filters.iter().enumerate() {
            let param_index = i + 1;
            sql.push_str(&format!(" AND {} LIKE ${}", col, param_index));
        }
        let limit_index = active_filters.len() + 1;
        let offset_index = limit_index + 1;
        let sort_by = params.sort_by.as_deref().unwrap_or("created_at");
        if [
            "created_at",
            "ip_address",
            "protocol",
            "fqdn",
            "path",
            "query",
            "city_name",
            "country_name",
            "country_code",
        ]
        .contains(&sort_by)
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

    pub async fn delete_before(pool: &PgPool, days: i32) -> Result<Vec<Request>, Error> {
        let sql = "DELETE FROM requests WHERE created_at > $1 RETURNING *";
        let now = Utc::now()
            .checked_sub_signed(chrono::Duration::days(days.into()))
            .unwrap();
        query(sql)
            .bind(now)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }

    pub async fn group_by_country(pool: &PgPool) -> Result<Vec<(String, i64)>, Error> {
        let sql = "SELECT country_name, COUNT(*) as count FROM requests GROUP BY country_name";
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
        let sql = "SELECT city_name, COUNT(*) as count FROM requests GROUP BY city_name";
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
        let sql = "SELECT COUNT(*) as count FROM requests";
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

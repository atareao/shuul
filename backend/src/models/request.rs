use chrono::{DateTime, Utc};
use http::Uri;
use maxminddb::Reader;
use serde::{Deserialize, Serialize};
use sqlx::{
    Error, FromRow, Row,
    postgres::{PgPool, PgRow},
    query, query_as,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub x: String, // Usamos String si la fecha viene como "YYYY-MM-DD"
    pub y: i64,    // La cuenta de peticiones
}

// Estructura principal para cada país (la serie completa)
// Nota: La columna 'data' de tu DB es un array JSON.
#[derive(Debug, Serialize, FromRow, Deserialize)]
pub struct TimeSeries {
    pub id: String,
    #[sqlx(json)] // Indica a sqlx que deserialice este campo desde JSONB
    pub data: Vec<TimeSeriesPoint>,
}

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;

impl NewRequest {
    pub fn from_request(headers: &http::HeaderMap, maxmind_db: &Reader<Vec<u8>>) -> Self {
        let protocol = headers
            .get("x-forwarded-proto")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        let host = headers
            .get("x-forwarded-host")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        let uri = headers
            .get("x-forwarded-uri")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("")
            .parse::<Uri>()
            .unwrap_or_default();
        let ip = headers
            .get("x-forwarded-for")
            .map(|s| s.to_str())
            .and_then(|result| result.ok())
            .unwrap_or("");
        let ip_data = IPData::complete(maxmind_db, ip);
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

    pub async fn create_bulk(
        pool: &PgPool,
        requests: Vec<NewRequest>,
    ) -> Result<Vec<Request>, Error> {
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
                .bind(request.created_at);
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

    pub async fn top_rules(pool: &PgPool) -> Result<Vec<(String, i32, f32)>, Error> {
        debug!("Computing top rules");
        let sql = r#"SELECT
    CASE
        WHEN ranking <= 10 THEN rule_id::text
        ELSE 'other'
    END AS rule,
    SUM(total_requests)::integer AS count,
    -- Calculate the percentage of each group relative to the total
    ROUND(
        (SUM(total_requests) * 100.0 / (SELECT COUNT(*) FROM requests))::numeric,
        1
    )::float4 AS percentage
FROM
(
    -- Subquery to count requests by country and assign a ranking
    SELECT
        rule_id,
        COUNT(*) AS total_requests,
        -- Assign a ranking number to identify the Top 10
        ROW_NUMBER() OVER (ORDER BY COUNT(*) DESC) AS ranking
    FROM
        requests
    WHERE
        rule_id IS NOT NULL
    GROUP BY
        rule_id
) AS ranked_requests
GROUP BY
    rule
ORDER BY
    count DESC;
    "#;
        query(sql)
            .map(|row: PgRow| {
                let country: String = row.get("rule");
                let count: i32 = row.get("count");
                let percentage: f32 = row.get("percentage");
                (country, count, percentage)
            })
            .fetch_all(pool)
            .await
    }
    pub async fn top_countries(pool: &PgPool) -> Result<Vec<(String, i32, f32)>, Error> {
        debug!("Computing top countries");
        let sql = r#"SELECT
    CASE
        WHEN ranking <= 10 THEN country_name
        ELSE 'other'
    END AS country,
    SUM(total_requests)::integer AS count,
    -- Calculate the percentage of each group relative to the total
    ROUND(
        (SUM(total_requests) * 100.0 / (SELECT COUNT(*) FROM requests))::numeric,
        1
    )::float4 AS percentage
FROM
(
    -- Subquery to count requests by country and assign a ranking
    SELECT
        country_name,
        COUNT(*) AS total_requests,
        -- Assign a ranking number to identify the Top 10
        ROW_NUMBER() OVER (ORDER BY COUNT(*) DESC) AS ranking
    FROM
        requests
    GROUP BY
        country_name
) AS ranked_requests
GROUP BY
    country
ORDER BY
    count DESC;
    "#;
        query(sql)
            .map(|row: PgRow| {
                let country: String = row.get("country");
                let count: i32 = row.get("count");
                let percentage: f32 = row.get("percentage");
                (country, count, percentage)
            })
            .fetch_all(pool)
            .await
    }
    // request.rs -> impl Request -> evolution

    pub async fn evolution(pool: &PgPool, unit: &str, last: i32) -> Result<Vec<TimeSeries>, Error> {
        debug!("Evolution params - unit: {}, last: {}", unit, last);

        // ... (Validación de unit_group y determinación de x_output - esto está correcto) ...
        let unit_group = match unit {
            "hour" => "hour",
            "day" => "day",
            _ => return Err(Error::RowNotFound),
        };
        let sql = format!(
            r#"WITH ranked_countries AS (
            -- CTE 1: Calcula el ranking total de peticiones en el periodo
            SELECT
                country_code,
                -- Usamos ROW_NUMBER() para un Top 10 estricto
                ROW_NUMBER() OVER (ORDER BY COUNT(*) DESC) AS ranking
            FROM
                requests
            WHERE
                created_at >= NOW() - INTERVAL '1 {unit_group}' * $1 
                AND country_code IS NOT NULL
            GROUP BY
                country_code
        ),
        -- NUEVO CTE 3: Consolidar y sumar las peticiones para 'other' o códigos reales
        aggregated_series AS (
            SELECT
                CASE
                    -- Aplica la etiqueta 'other' a todos los países fuera del Top 10
                    WHEN rp.ranking <= 10 THEN gr.country_code
                    ELSE 'other'
                END AS series_id, -- El ID de la serie (país o 'other')
                gr.date_group,
                -- SUMA las peticiones para cada (series_id, date_group)
                SUM(gr.num_requests) AS total_requests_for_point
            FROM
                ranked_countries AS rp
            JOIN (
                -- CTE 2: Cuenta las peticiones por país y unidad de tiempo
                SELECT
                    country_code,
                    DATE_TRUNC('{unit_group}', created_at) AS date_group,
                    COUNT(*)::integer AS num_requests
                FROM
                    requests
                WHERE
                    created_at >= NOW() - INTERVAL '1 {unit_group}' * $1 
                    AND country_code IS NOT NULL
                GROUP BY
                    country_code,
                    date_group
            ) AS gr
            ON rp.country_code = gr.country_code
            GROUP BY
                series_id,
                gr.date_group -- AGREGAMOS POR SERIE Y UNIDAD DE TIEMPO
        )
        SELECT
            series_id AS id,
            -- Agregación final: Crea el array JSON de puntos
            jsonb_agg(
                jsonb_build_object(
                    'x', (COALESCE(TO_CHAR(date_group, 'YYYY-MM-DD\"T\"HH24:MI:SS'), '1970-01-01T00:00:00') || 'Z')::text,
                    'y', total_requests_for_point::integer
                )
                ORDER BY date_group
            ) AS data
        FROM
            aggregated_series
        GROUP BY
            series_id
        ORDER BY
            series_id;
    "#
        );
        sqlx::query_as::<_, TimeSeries>(&sql)
            .bind(last)
            .fetch_all(pool)
            .await
    }
}

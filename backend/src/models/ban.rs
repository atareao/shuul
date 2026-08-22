//! # Modelo de bans persistidos
//!
//! Representa bans HTTP activos o históricos almacenados en PostgreSQL.

use chrono::{DateTime, Utc};
use sqlx::{
    Error, Row,
    postgres::{PgPool, PgRow},
    query,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ban {
    pub id: i32,
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub jail_name: String,
    pub banned_at: DateTime<Utc>,
    pub ban_duration_seconds: i32,
    pub escalation_level: i32,
    pub reason: Option<String>,
    pub expired: bool,
    pub created_at: DateTime<Utc>,
    pub ban_count_decay_days: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewBan {
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub jail_name: String,
    pub banned_at: DateTime<Utc>,
    pub ban_duration_seconds: i32,
    pub escalation_level: i32,
    pub reason: Option<String>,
}

impl Ban {
    fn from_row(row: PgRow) -> Self {
        Self {
            id: row.get("id"),
            ip_address: row.get("ip_address"),
            rule_id: row.get("rule_id"),
            jail_name: row.get("jail_name"),
            banned_at: row.get("banned_at"),
            ban_duration_seconds: row.get("ban_duration_seconds"),
            escalation_level: row.get("escalation_level"),
            reason: row.get("reason"),
            expired: row.get("expired"),
            created_at: row.get("created_at"),
            ban_count_decay_days: row.try_get("ban_count_decay_days").ok(),
        }
    }

    pub async fn create(pool: &PgPool, ban: NewBan) -> Result<Self, Error> {
        let sql = "INSERT INTO bans (
                ip_address, rule_id, jail_name, banned_at, ban_duration_seconds,
                escalation_level, reason, expired, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE, NOW())
            RETURNING *";
        query(sql)
            .bind(ban.ip_address)
            .bind(ban.rule_id)
            .bind(ban.jail_name)
            .bind(ban.banned_at)
            .bind(ban.ban_duration_seconds)
            .bind(ban.escalation_level)
            .bind(ban.reason)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }

    pub async fn read_active(pool: &PgPool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT bans.*, rules.ban_count_decay_days
            FROM bans
            LEFT JOIN rules ON rules.id = bans.rule_id
            WHERE expired = FALSE
              AND banned_at + (ban_duration_seconds * INTERVAL '1 second') > NOW()
            ORDER BY banned_at DESC";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub async fn active_count(pool: &PgPool) -> Result<i64, Error> {
        let sql = "SELECT COUNT(*) AS total FROM bans
            WHERE expired = FALSE
              AND banned_at + (ban_duration_seconds * INTERVAL '1 second') > NOW()";
        query(sql)
            .map(|row: PgRow| row.get("total"))
            .fetch_one(pool)
            .await
    }

    pub async fn expire_elapsed(pool: &PgPool) -> Result<Vec<Self>, Error> {
        let sql = "UPDATE bans
            SET expired = TRUE
            WHERE expired = FALSE
              AND banned_at + (ban_duration_seconds * INTERVAL '1 second') <= NOW()
            RETURNING *";
        query(sql).map(Self::from_row).fetch_all(pool).await
    }

    pub async fn expire_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, Error> {
        let sql = "UPDATE bans
            SET expired = TRUE
            WHERE id = $1 AND expired = FALSE
            RETURNING *";
        query(sql)
            .bind(id)
            .map(Self::from_row)
            .fetch_optional(pool)
            .await
    }

    pub async fn expire_by_ip_rule(
        pool: &PgPool,
        ip_address: &str,
        rule_id: Option<i32>,
    ) -> Result<Vec<Self>, Error> {
        let sql = "UPDATE bans
            SET expired = TRUE
            WHERE ip_address = $1
              AND rule_id IS NOT DISTINCT FROM $2
              AND expired = FALSE
            RETURNING *";
        query(sql)
            .bind(ip_address)
            .bind(rule_id)
            .map(Self::from_row)
            .fetch_all(pool)
            .await
    }
}

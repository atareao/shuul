//! # Ban Manager
//!
//! Manages active IP bans with escalation and decay.
//! Bans are enforced at the HTTP level — no firewall backend needed.
//!
//! The core [`BanManager`] is purely synchronous and in-memory.
//! Database persistence methods are provided as async associated functions
//! to be called by HTTP handlers after the mutex lock/release cycle.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Information about an active ban.
#[derive(Debug, Clone)]
pub struct BanInfo {
    /// When the ban was issued
    pub banned_at: Instant,
    /// Duration of the ban in seconds
    pub ban_duration_seconds: i64,
    /// Current escalation level (0 = first offense)
    pub escalation_level: u32,
    /// ID of the rule that triggered the ban
    pub rule_id: Option<i32>,
    /// Human-readable reason
    pub reason: String,
}

impl BanInfo {
    /// Returns true if this ban has expired.
    pub fn is_expired(&self) -> bool {
        let elapsed = Instant::now().duration_since(self.banned_at);
        elapsed > Duration::from_secs(self.ban_duration_seconds as u64)
    }

    /// Returns the remaining duration as a human-friendly string.
    pub fn time_remaining(&self) -> Duration {
        let elapsed = Instant::now().duration_since(self.banned_at);
        let total = Duration::from_secs(self.ban_duration_seconds as u64);
        if elapsed > total {
            Duration::from_secs(0)
        } else {
            total.checked_sub(elapsed).unwrap()
        }
    }
}

/// Manages all active bans, with escalation and decay logic.
#[derive(Debug, Clone)]
pub struct BanManager {
    /// Active bans keyed by IP address
    bans: HashMap<IpAddr, Vec<BanInfo>>,
    /// Per-IP escalation counters (decays over time)
    escalation_counts: HashMap<IpAddr, (u32, Instant)>,
    /// Default ban duration for new bans
    default_ban_duration: i64,
    /// Whether to escalate repeat offenses
    bantime_increment: bool,
    /// Multipliers for escalation (e.g., [1, 2, 4, 8])
    bantime_multipliers: Vec<u32>,
    /// Maximum ban duration
    bantime_maxtime: i64,
    /// Days after which escalation counter resets
    ban_count_decay_days: i64,
}

impl BanManager {
    /// Create a new `BanManager` with default settings.
    #[must_use]
    pub fn new(
        default_ban_duration: i64,
        bantime_increment: bool,
        bantime_multipliers: Vec<u32>,
        bantime_maxtime: i64,
        ban_count_decay_days: i64,
    ) -> Self {
        Self {
            bans: HashMap::new(),
            escalation_counts: HashMap::new(),
            default_ban_duration,
            bantime_increment,
            bantime_multipliers,
            bantime_maxtime,
            ban_count_decay_days,
        }
    }

    /// Check if an IP is currently banned.
    /// Returns the first active ban info, or None.
    #[must_use]
    pub fn is_banned(&self, ip: &IpAddr) -> Option<&BanInfo> {
        self.bans
            .get(ip)
            .and_then(|ban_list| ban_list.iter().find(|ban| !ban.is_expired()))
    }

    /// Ban an IP address.
    ///
    /// If `ban_duration_override` is `Some`, it is used as the ban duration
    /// instead of the calculated escalation-based duration.
    ///
    /// Returns a reference to the new [`BanInfo`].
    ///
    /// NOTE: This method only operates on in-memory state. To persist the ban
    /// to the database, call [`BanManager::persist_ban`] after the mutex is released.
    pub fn ban(
        &mut self,
        ip: IpAddr,
        rule_id: Option<i32>,
        reason: String,
        ban_duration_override: Option<i64>,
    ) -> &BanInfo {
        let escalation_level = self.get_escalation_level(&ip);
        let duration =
            ban_duration_override.unwrap_or_else(|| self.calculate_ban_duration(escalation_level));

        let ban_info = BanInfo {
            banned_at: Instant::now(),
            ban_duration_seconds: duration,
            escalation_level,
            rule_id,
            reason,
        };

        self.bans.entry(ip).or_default().push(ban_info);
        self.increment_escalation(&ip);

        // Return reference to the last added ban
        self.bans.get(&ip).unwrap().last().unwrap()
    }

    /// Unban an IP for a specific rule. Returns true if anything was removed.
    ///
    /// NOTE: This method only operates on in-memory state. To persist the unban
    /// to the database, call [`BanManager::remove_from_db`] after the mutex is released.
    pub fn unban(&mut self, ip: &IpAddr, rule_id: Option<i32>) -> bool {
        if let Some(ban_list) = self.bans.get_mut(ip) {
            let before = ban_list.len();
            ban_list.retain(|b| b.rule_id != rule_id && !b.is_expired());
            let removed = before - ban_list.len();
            if ban_list.is_empty() {
                self.bans.remove(ip);
            }
            removed > 0
        } else {
            false
        }
    }

    /// Unban all entries for an IP.
    #[allow(dead_code)]
    pub fn unban_all(&mut self, ip: &IpAddr) -> bool {
        self.bans.remove(ip).is_some()
    }

    /// Remove all expired bans in memory.
    ///
    /// NOTE: To clean up expired bans in the database, call
    /// [`BanManager::cleanup_expired_db`] separately.
    pub fn cleanup_expired(&mut self) {
        self.bans.retain(|_, ban_list| {
            ban_list.retain(|b| !b.is_expired());
            !ban_list.is_empty()
        });
        // Decay escalation counters
        let decay_duration = Duration::from_secs(self.ban_count_decay_days as u64 * 86400);
        self.escalation_counts
            .retain(|_, (_, last_ban)| Instant::now().duration_since(*last_ban) <= decay_duration);
    }

    /// Get all active (non-expired) bans.
    #[must_use]
    pub fn active_bans(&self) -> Vec<(IpAddr, &BanInfo)> {
        let mut result = Vec::new();
        for (ip, ban_list) in &self.bans {
            for ban in ban_list {
                if !ban.is_expired() {
                    result.push((*ip, ban));
                }
            }
        }
        result
    }

    /// Number of active bans.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active_bans().len()
    }

    /// Calculate ban duration based on escalation level.
    fn calculate_ban_duration(&self, escalation_level: u32) -> i64 {
        if !self.bantime_increment || escalation_level == 0 {
            return self.default_ban_duration;
        }

        let multiplier_idx = (escalation_level as usize).min(self.bantime_multipliers.len() - 1);
        let multiplier = i64::from(self.bantime_multipliers[multiplier_idx]);
        let duration = self.default_ban_duration * multiplier;

        duration.min(self.bantime_maxtime)
    }

    /// Get the current escalation level for an IP.
    fn get_escalation_level(&self, ip: &IpAddr) -> u32 {
        self.escalation_counts
            .get(ip)
            .map_or(0, |(level, _)| *level)
    }

    /// Increment the escalation counter for an IP.
    fn increment_escalation(&mut self, ip: &IpAddr) {
        let entry = self
            .escalation_counts
            .entry(*ip)
            .or_insert((0, Instant::now()));
        entry.0 += 1;
        entry.1 = Instant::now();
    }
}

// ---------------------------------------------------------------------------
// Database persistence layer
// ---------------------------------------------------------------------------
//
// These are async associated functions (not self-methods) that operate directly
// on the database. Call them from HTTP handlers AFTER the mutex lock/release
// on the in-memory BanManager.
//
// The BanManager itself remains purely synchronous — it has no DB awareness.

impl BanManager {
    /// Load all active (non-expired) bans from the database into a new
    /// `BanManager` with sensible defaults.
    ///
    /// This is useful on application startup to restore ban state from
    /// the previous session.
    pub async fn load_from_db(
        pool: &sqlx::PgPool,
    ) -> Result<(Self, Vec<(IpAddr, Instant)>), sqlx::Error> {
        use chrono::{DateTime, Utc};
        use sqlx::Row;

        let rows = sqlx::query(
            "SELECT ip_address, rule_id, reason, banned_at, ban_duration_seconds, escalation_level \
             FROM bans WHERE expired = FALSE",
        )
        .fetch_all(pool)
        .await?;

        let mut manager = Self::new(
            3600,             // default_ban_duration
            true,             // bantime_increment
            vec![1, 2, 4, 8], // bantime_multipliers
            604800,           // bantime_maxtime (7 days)
            30,               // ban_count_decay_days
        );

        let mut loaded: Vec<(IpAddr, Instant)> = Vec::with_capacity(rows.len());

        for row in &rows {
            let ip_str: String = row.get("ip_address");
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                let banned_at_db: DateTime<Utc> = row.get("banned_at");
                let ban_duration_seconds: i64 = row.get("ban_duration_seconds");
                let escalation_level: i32 = row.get("escalation_level");
                let rule_id: Option<i32> = row.get("rule_id");
                let reason: String = row.get("reason");

                // Approximate Instant from DB DateTime<Utc>
                let now = Instant::now();
                let now_dt = Utc::now();
                let elapsed_secs = (now_dt - banned_at_db).num_seconds().max(0) as u64;
                let banned_at = now.checked_sub(Duration::from_secs(elapsed_secs)).unwrap();

                let ban_info = BanInfo {
                    banned_at,
                    ban_duration_seconds,
                    escalation_level: escalation_level as u32,
                    rule_id,
                    reason,
                };

                manager.bans.entry(ip).or_default().push(ban_info);
                loaded.push((ip, banned_at));
            }
        }

        Ok((manager, loaded))
    }

    /// Persist a ban to the database.
    ///
    /// Call this AFTER `BanManager::ban()` to ensure the ban is durable.
    pub async fn persist_ban(
        pool: &sqlx::PgPool,
        ip: IpAddr,
        rule_id: Option<i32>,
        reason: &str,
        ban_duration_seconds: i64,
        escalation_level: u32,
    ) -> Result<(), sqlx::Error> {
        use chrono::Utc;

        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, FALSE, $7)"
        )
        .bind(ip.to_string())
        .bind(rule_id)
        .bind(reason)
        .bind(Utc::now())
        .bind(ban_duration_seconds)
        .bind(escalation_level as i32)
        .bind(Utc::now())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Mark a ban as expired in the database (soft-delete).
    ///
    /// Call this AFTER `BanManager::unban()` to keep the DB consistent.
    pub async fn remove_from_db(
        pool: &sqlx::PgPool,
        ip: &IpAddr,
        rule_id: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        if let Some(rid) = rule_id {
            sqlx::query(
                "UPDATE bans SET expired = TRUE WHERE ip_address = $1 AND rule_id = $2 AND expired = FALSE"
            )
            .bind(ip.to_string())
            .bind(rid)
            .execute(pool)
            .await?;
        } else {
            sqlx::query("UPDATE bans SET expired = TRUE WHERE ip_address = $1 AND expired = FALSE")
                .bind(ip.to_string())
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    /// Mark all expired (past their `ban_duration_seconds`) bans as expired in the DB.
    ///
    /// Call this periodically (e.g. via a cron-like task) to keep the DB clean.
    pub async fn cleanup_expired_db(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE bans SET expired = TRUE \
             WHERE expired = FALSE \
             AND banned_at + (ban_duration_seconds * INTERVAL '1 second') < NOW()",
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ban_and_check() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        assert!(bm.is_banned(&ip).is_none());
        bm.ban(ip, None, "test ban".to_string(), None);
        assert!(bm.is_banned(&ip).is_some());
    }

    #[test]
    fn test_unban() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        bm.ban(ip, Some(1), "test".to_string(), None);
        assert!(bm.unban(&ip, Some(1)));
        assert!(bm.is_banned(&ip).is_none());
    }

    #[test]
    fn test_escalation() {
        let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First offense: 3600s (multiplier 1)
        let ban1 = bm.ban(ip, None, "1st".to_string(), None);
        assert_eq!(ban1.ban_duration_seconds, 3600);

        // Second offense: 7200s (multiplier 2)
        let ban2 = bm.ban(ip, None, "2nd".to_string(), None);
        assert_eq!(ban2.ban_duration_seconds, 7200);

        // Third offense: 14400s (multiplier 4)
        let ban3 = bm.ban(ip, None, "3rd".to_string(), None);
        assert_eq!(ban3.ban_duration_seconds, 14400);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut bm = BanManager::new(1, false, vec![1], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        bm.ban(ip, None, "short ban".to_string(), None);
        assert_eq!(bm.active_count(), 1);

        std::thread::sleep(Duration::from_millis(1100));
        bm.cleanup_expired();
        assert_eq!(bm.active_count(), 0);
    }
}

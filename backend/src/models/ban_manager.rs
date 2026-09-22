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
    #[allow(clippy::cast_sign_loss)]
    pub fn is_expired(&self) -> bool {
        let elapsed = Instant::now().duration_since(self.banned_at);
        elapsed > Duration::from_secs(self.ban_duration_seconds as u64)
    }

    /// Returns the remaining duration as a human-friendly string.
    #[allow(clippy::cast_sign_loss)]
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

    /// Check if an IP is currently banned for a specific rule.
    /// Returns true if there is at least one active (non-expired) ban
    /// whose `rule_id` matches the given `rule_id`.
    #[must_use]
    #[allow(dead_code)]
    pub fn is_banned_for_rule(&self, ip: &IpAddr, rule_id: Option<i32>) -> bool {
        self.bans.get(ip).is_some_and(|ban_list| {
            ban_list
                .iter()
                .any(|b| !b.is_expired() && b.rule_id == rule_id)
        })
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
    /// Bans an IP address.
    ///
    /// # Panics
    ///
    /// Panics if the IP is not found in the bans map after insertion,
    /// which should never happen under normal circumstances.
    #[must_use]
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

        let bans = self.bans.entry(ip).or_default();

        let target_idx = if let Some(rid) = rule_id {
            // If an active (non-expired) ban with the same rule_id exists, replace it
            if let Some(pos) = bans
                .iter()
                .position(|b| b.rule_id == Some(rid) && !b.is_expired())
            {
                bans[pos] = ban_info;
                pos
            } else {
                bans.push(ban_info);
                bans.len() - 1
            }
        } else {
            bans.push(ban_info);
            bans.len() - 1
        };

        // `bans` mutable borrow ends here (NLL) before increment_escalation
        self.increment_escalation(&ip);

        // Return reference to the ban we just added/replaced
        &self.bans.get(&ip).unwrap()[target_idx]
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

    /// Get all bans (active and expired) for a specific IP.
    #[must_use]
    #[allow(dead_code)]
    pub fn bans_for_ip(&self, ip: &IpAddr) -> &[BanInfo] {
        self.bans.get(ip).map_or(&[], |v| v.as_slice())
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
        #[allow(clippy::cast_sign_loss)]
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
            .or_insert_with(|| (0, Instant::now()));
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
    /// Loads bans from the database, deduplicating by `(ip_address, rule_id)`.
    ///
    /// If multiple active (non-expired) rows exist for the same IP + `rule_id`
    /// pair, only the most recent row (by `banned_at`) is loaded. This handles
    /// the case where `persist_ban` did an UPDATE (leaving a single row) or
    /// where legacy code may have inserted duplicates.
    ///
    /// # Panics
    ///
    /// Panics if the ban duration calculation overflows, which should
    /// not happen with valid database values.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    #[allow(clippy::cast_sign_loss, clippy::too_many_lines)]
    pub async fn load_from_db(
        pool: &sqlx::SqlitePool,
    ) -> Result<(Self, Vec<(IpAddr, Instant)>), sqlx::Error> {
        use chrono::{DateTime, Utc};
        use sqlx::Row;

        // Deduplicate by (ip_address, rule_id): for each pair, keep only the
        // most recent row (by banned_at) using ROW_NUMBER().
        let rows = sqlx::query(
            "SELECT ip_address, rule_id, reason, banned_at, ban_duration_seconds, \
                    escalation_level \
             FROM bans \
             WHERE expired = 0 \
               AND rowid IN ( \
                 SELECT rowid FROM ( \
                   SELECT rowid, \
                          ROW_NUMBER() OVER ( \
                            PARTITION BY ip_address, COALESCE(rule_id, 0) \
                            ORDER BY banned_at DESC \
                          ) AS rn \
                   FROM bans \
                   WHERE expired = 0 \
                 ) WHERE rn = 1 \
               )",
        )
        .fetch_all(pool)
        .await?;

        // Query the rate_limit_profile with the highest bantime_maxtime_seconds
        // among all active bans' referenced rules. This provides the BanManager
        // with the most restrictive profile as the global baseline.
        // INNER JOINs ensure we only consider bans whose rule → profile chain
        // is intact; if a rule or profile has been deleted, that ban is
        // ignored for baseline selection and defaults are used.
        let profile_row: Option<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT rlp.bantime_multipliers, rlp.bantime_maxtime_seconds, \
                    rlp.ban_count_decay_days, rlp.bantime_increment \
             FROM bans b \
             INNER JOIN rules r ON b.rule_id = r.id \
             INNER JOIN rate_limit_profiles rlp ON r.rate_limit_profile_id = rlp.id \
             WHERE b.expired = 0 \
             ORDER BY rlp.bantime_maxtime_seconds DESC \
             LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        let (bantime_multipliers, bantime_maxtime, ban_count_decay_days, bantime_increment) =
            profile_row.map_or_else(
                || {
                    (
                        vec![1, 2, 4, 8], // default multipliers
                        604_800,          // default maxtime (7 days)
                        30,               // default decay_days
                        true,             // default bantime_increment
                    )
                },
                |row| {
                    let multipliers_str: String = row.get("bantime_multipliers");
                    let multipliers: Vec<u32> =
                        serde_json::from_str(&multipliers_str).unwrap_or_else(|_| vec![1, 2, 4, 8]);
                    let maxtime: i64 = row.get("bantime_maxtime_seconds");
                    let decay_days: i64 = row.get("ban_count_decay_days");
                    let increment: bool = row.get("bantime_increment");
                    (multipliers, maxtime, decay_days, increment)
                },
            );

        let mut manager = Self::new(
            3600, // default_ban_duration
            bantime_increment,
            bantime_multipliers,
            bantime_maxtime,
            ban_count_decay_days,
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

                // Restore escalation_counts with the maximum historical level per IP
                let entry = manager
                    .escalation_counts
                    .entry(ip)
                    .or_insert_with(|| (0, Instant::now()));
                if (escalation_level as u32) > entry.0 {
                    entry.0 = escalation_level as u32;
                }

                loaded.push((ip, banned_at));
            }
        }

        // Mark legacy duplicates as expired in DB so they get cleaned up
        // by the periodic cleanup task. Only the most recent ban per
        // (ip_address, COALESCE(rule_id, 0)) is kept as active.
        sqlx::query(
            "UPDATE bans SET expired = 1 \
             WHERE expired = 0 \
               AND rowid NOT IN ( \
                 SELECT MIN(rowid) FROM ( \
                   SELECT rowid, ROW_NUMBER() OVER ( \
                     PARTITION BY ip_address, COALESCE(rule_id, 0) \
                     ORDER BY banned_at DESC \
                   ) AS rn FROM bans WHERE expired = 0 \
                 ) WHERE rn = 1 \
               )",
        )
        .execute(pool)
        .await?;

        Ok((manager, loaded))
    }

    /// Persist a ban to the database.
    ///
    /// If a ban already exists for the same `(ip_address, rule_id)` pair and is
    /// not expired, it is **updated** in-place (escalation, reason, duration).
    /// Otherwise a new row is inserted.
    ///
    /// When `rule_id` is `None`, a new row is always inserted (no UNIQUE-like
    /// deduplication for NULL rule IDs).
    ///
    /// Call this AFTER `BanManager::ban()` to ensure the ban is durable.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    #[allow(clippy::cast_possible_wrap)]
    pub async fn persist_ban(
        pool: &sqlx::SqlitePool,
        ip: IpAddr,
        rule_id: Option<i32>,
        reason: &str,
        ban_duration_seconds: i64,
        escalation_level: u32,
    ) -> Result<(), sqlx::Error> {
        use chrono::Utc;
        let now = Utc::now();

        if let Some(rid) = rule_id {
            // Try to update existing active ban first (same IP + rule_id, not expired)
            let affected = sqlx::query(
                "UPDATE bans SET reason = ?, banned_at = ?, ban_duration_seconds = ?, \
                 escalation_level = ?, expired = 0 \
                 WHERE ip_address = ? AND rule_id = ? AND expired = 0",
            )
            .bind(reason)
            .bind(now)
            .bind(ban_duration_seconds)
            .bind(escalation_level as i32)
            .bind(ip.to_string())
            .bind(rid)
            .execute(pool)
            .await?
            .rows_affected();

            if affected == 0 {
                // No existing active ban, insert new row
                sqlx::query(
                    "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
                     ban_duration_seconds, escalation_level, expired, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
                )
                .bind(ip.to_string())
                .bind(rid)
                .bind(reason)
                .bind(now)
                .bind(ban_duration_seconds)
                .bind(escalation_level as i32)
                .bind(now)
                .execute(pool)
                .await?;
            }
        } else {
            // rule_id=None: always insert (no UNIQUE constraint for NULL)
            sqlx::query(
                "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
                 ban_duration_seconds, escalation_level, expired, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
            )
            .bind(ip.to_string())
            .bind(rule_id)
            .bind(reason)
            .bind(now)
            .bind(ban_duration_seconds)
            .bind(escalation_level as i32)
            .bind(now)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Mark a ban as expired in the database (soft-delete).
    ///
    /// Call this AFTER `BanManager::unban()` to keep the DB consistent.
    /// Removes a ban from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database delete fails.
    pub async fn remove_from_db(
        pool: &sqlx::SqlitePool,
        ip: &IpAddr,
        rule_id: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        if let Some(rid) = rule_id {
            sqlx::query(
                "UPDATE bans SET expired = 1 WHERE ip_address = ? AND rule_id = ? AND expired = 0",
            )
            .bind(ip.to_string())
            .bind(rid)
            .execute(pool)
            .await?;
        } else {
            sqlx::query("UPDATE bans SET expired = 1 WHERE ip_address = ? AND expired = 0")
                .bind(ip.to_string())
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    /// Mark all expired (past their `ban_duration_seconds`) bans as expired in the DB.
    ///
    /// Call this periodically (e.g. via a cron-like task) to keep the DB clean.
    #[allow(dead_code)]
    /// Cleans up expired bans from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database delete fails.
    pub async fn cleanup_expired_db(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE bans SET expired = 1 \
             WHERE expired = 0 \
             AND CAST(strftime('%s', banned_at) AS INTEGER) + ban_duration_seconds \
                 < CAST(strftime('%s', 'now') AS INTEGER)",
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
        let _ = bm.ban(ip, None, "test ban".to_string(), None);
        assert!(bm.is_banned(&ip).is_some());
    }

    #[test]
    fn test_unban() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        let _ = bm.ban(ip, Some(1), "test".to_string(), None);
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

        let _ = bm.ban(ip, None, "short ban".to_string(), None);
        assert_eq!(bm.active_count(), 1);

        std::thread::sleep(Duration::from_millis(1100));
        bm.cleanup_expired();
        assert_eq!(bm.active_count(), 0);
    }

    #[test]
    fn test_ban_replaces_existing_active_same_rule_id() {
        let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First ban with rule_id=Some(1)
        let ban1 = bm.ban(ip, Some(1), "first".to_string(), None);
        assert_eq!(ban1.escalation_level, 0);
        assert_eq!(ban1.ban_duration_seconds, 3600);

        // Second ban with SAME rule_id=Some(1) — should REPLACE, not append
        let ban2 = bm.ban(ip, Some(1), "second".to_string(), None);
        assert_eq!(ban2.escalation_level, 1);
        assert_eq!(ban2.ban_duration_seconds, 7200);
        // ban2 borrow ends here (NLL) — now we can immutably borrow bm

        let bans = bm.bans_for_ip(&ip);
        assert_eq!(
            bans.len(),
            1,
            "should have only 1 ban (replaced, not appended)"
        );
    }

    #[test]
    fn test_ban_adds_new_for_different_rule_id() {
        let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First ban with rule_id=Some(1)
        let ban1 = bm.ban(ip, Some(1), "first".to_string(), None);
        assert_eq!(ban1.escalation_level, 0);
        // ban1 borrow ends here (NLL)

        // Second ban with DIFFERENT rule_id=Some(2) — should APPEND
        let _ban2 = bm.ban(ip, Some(2), "second".to_string(), None);

        let bans = bm.bans_for_ip(&ip);
        assert_eq!(bans.len(), 2, "should have 2 bans (different rule_ids)");
        assert_eq!(bans[0].rule_id, Some(1), "first ban rule_id unchanged");
        assert_eq!(bans[1].rule_id, Some(2), "second ban has new rule_id");
    }

    #[test]
    fn test_ban_adds_new_for_none_rule_id() {
        let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First ban with rule_id=None
        let _ban1 = bm.ban(ip, None, "first".to_string(), None);

        // Second ban with rule_id=None — should APPEND (None never replaces)
        let _ban2 = bm.ban(ip, None, "second".to_string(), None);

        let bans = bm.bans_for_ip(&ip);
        assert_eq!(bans.len(), 2, "rule_id=None should never replace");
        assert!(bans[0].rule_id.is_none());
        assert!(bans[1].rule_id.is_none());
    }

    #[test]
    fn test_ban_adds_new_when_existing_expired() {
        let mut bm = BanManager::new(1, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First ban with rule_id=Some(1), duration=1s
        let ban1 = bm.ban(ip, Some(1), "first".to_string(), None);
        assert_eq!(ban1.ban_duration_seconds, 1);
        // ban1 borrow ends here (NLL)

        // Wait for it to expire
        std::thread::sleep(Duration::from_millis(1100));

        // Second ban with SAME rule_id — first is expired, so APPEND
        let _ban2 = bm.ban(ip, Some(1), "second".to_string(), None);

        let bans = bm.bans_for_ip(&ip);
        assert_eq!(
            bans.len(),
            2,
            "expired ban should not be replaced, only active ones"
        );
        assert!(bans[0].is_expired(), "first ban should be expired");
        assert!(!bans[1].is_expired(), "second ban should be active");
    }

    // ------------------------------------------------------------------
    // is_banned_for_rule tests
    // ------------------------------------------------------------------

    #[test]
    fn test_is_banned_for_rule_matching_rule_id_returns_true() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let _ = bm.ban(ip, Some(1), "test".to_string(), None);
        assert!(bm.is_banned_for_rule(&ip, Some(1)));
    }

    #[test]
    fn test_is_banned_for_rule_different_rule_id_returns_false() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let _ = bm.ban(ip, Some(1), "test".to_string(), None);
        assert!(!bm.is_banned_for_rule(&ip, Some(2)));
    }

    #[test]
    fn test_is_banned_for_rule_none_rule_id_matches_none() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let _ = bm.ban(ip, None, "test".to_string(), None);
        assert!(bm.is_banned_for_rule(&ip, None));
    }

    #[test]
    fn test_is_banned_for_rule_none_rule_id_does_not_match_some() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let _ = bm.ban(ip, None, "test".to_string(), None);
        assert!(!bm.is_banned_for_rule(&ip, Some(1)));
    }

    #[test]
    fn test_is_banned_for_rule_unbanned_ip_returns_false() {
        let bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert!(!bm.is_banned_for_rule(&ip, Some(1)));
    }

    #[test]
    fn test_is_banned_for_rule_expired_ban_returns_false() {
        let mut bm = BanManager::new(1, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let _ = bm.ban(ip, Some(1), "test".to_string(), None);
        // Wait for ban to expire
        std::thread::sleep(Duration::from_millis(1100));
        assert!(!bm.is_banned_for_rule(&ip, Some(1)));
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::str::FromStr;

    async fn create_pool_and_schema() -> sqlx::SqlitePool {
        let opts = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();

        // Disable FK enforcement so tests can insert ban rows without
        // creating the full referenced Rule/Profile chain.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rate_limit_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                max_retry INTEGER NOT NULL DEFAULT 5,
                find_time_seconds INTEGER NOT NULL DEFAULT 600,
                ban_time_seconds INTEGER NOT NULL DEFAULT 3600,
                bantime_increment INTEGER NOT NULL DEFAULT 0,
                bantime_multipliers TEXT NOT NULL DEFAULT '[1,2,4,8]',
                bantime_maxtime_seconds INTEGER NOT NULL DEFAULT 604800,
                ban_count_decay_days INTEGER NOT NULL DEFAULT 30,
                fail_codes TEXT NOT NULL DEFAULT '[401,403,404]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                weight INTEGER NOT NULL DEFAULT 100,
                mode TEXT NOT NULL DEFAULT 'log_only',
                pipeline TEXT NOT NULL DEFAULT 'waf',
                allow INTEGER NOT NULL DEFAULT 1,
                ip_address TEXT, protocol TEXT, fqdn TEXT, path TEXT, query TEXT,
                city_name TEXT, country_name TEXT, country_code TEXT,
                user_agent TEXT, method TEXT, referer TEXT, content_type TEXT,
                accept_language TEXT, x_request_id TEXT,
                rate_limit_profile_id INTEGER REFERENCES rate_limit_profiles(id) ON DELETE SET NULL,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS bans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ip_address TEXT NOT NULL,
                rule_id INTEGER REFERENCES rules(id) ON DELETE SET NULL,
                reason TEXT NOT NULL DEFAULT '',
                banned_at TEXT NOT NULL,
                ban_duration_seconds INTEGER NOT NULL,
                escalation_level INTEGER NOT NULL DEFAULT 0,
                expired INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    // ------------------------------------------------------------------
    // persist_ban tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_persist_ban_inserts_new() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        BanManager::persist_ban(&pool, ip, Some(1), "test reason", 3600, 0)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "should insert exactly one row");
    }

    #[tokio::test]
    async fn test_persist_ban_updates_existing() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let now = chrono::Utc::now();

        // Insert a row manually (simulating a previous ban)
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("original")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Now persist_ban with same IP + rule_id — should UPDATE, not INSERT
        BanManager::persist_ban(&pool, ip, Some(1), "updated reason", 7200, 1)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bans WHERE ip_address = ? AND rule_id = ? AND expired = 0",
        )
        .bind(ip.to_string())
        .bind(1i32)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            count, 1,
            "should still have exactly one active row (UPDATE, not insert)"
        );

        // Verify the row was actually updated
        let reason: String = sqlx::query_scalar(
            "SELECT reason FROM bans WHERE ip_address = ? AND rule_id = ? AND expired = 0",
        )
        .bind(ip.to_string())
        .bind(1i32)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(reason, "updated reason", "reason should reflect the update");
    }

    #[tokio::test]
    async fn test_persist_ban_inserts_new_for_none_rule_id() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        // persist_ban with rule_id=None twice — each should INSERT
        BanManager::persist_ban(&pool, ip, None, "first", 3600, 0)
            .await
            .unwrap();
        BanManager::persist_ban(&pool, ip, None, "second", 7200, 1)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2, "rule_id=None should always insert new rows");
    }

    // ------------------------------------------------------------------
    // load_from_db deduplication tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_load_from_db_deduplicates() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let earlier = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();

        // Insert older row
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("old")
        .bind(earlier)
        .bind(3600i64)
        .bind(0i32)
        .bind(earlier)
        .execute(&pool)
        .await
        .unwrap();

        // Insert newer row
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("new")
        .bind(later)
        .bind(7200i64)
        .bind(1i32)
        .bind(later)
        .execute(&pool)
        .await
        .unwrap();

        // load_from_db should return only 1 ban (the most recent)
        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();
        let bans = manager.bans_for_ip(&ip);
        assert_eq!(
            bans.len(),
            1,
            "should deduplicate to 1 ban per (ip, rule_id)"
        );
        assert_eq!(bans[0].reason, "new", "should keep the most recent ban");
    }

    #[tokio::test]
    async fn test_load_from_db_keeps_different_rule_ids() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let now = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();

        // Insert row for rule_id=1
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("waf-rule")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Insert row for rule_id=2
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(2i32))
        .bind("jail-rule")
        .bind(now)
        .bind(7200i64)
        .bind(1i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // load_from_db should return both bans (different rule_ids)
        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();
        let bans = manager.bans_for_ip(&ip);
        assert_eq!(
            bans.len(),
            2,
            "should keep both bans with different rule_ids"
        );

        let rule_ids: Vec<Option<i32>> = bans.iter().map(|b| b.rule_id).collect();
        assert!(rule_ids.contains(&Some(1)), "should contain rule_id=1");
        assert!(rule_ids.contains(&Some(2)), "should contain rule_id=2");
    }

    // ------------------------------------------------------------------
    // load_from_db escalation_counts restoration tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_load_from_db_restores_escalation_counts() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let now = chrono::Utc::now();

        // Insert 2 bans for the same IP with different rule_ids and escalation_levels
        // rule_id=1 with escalation_level=5
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("rule1")
        .bind(now)
        .bind(3600i64)
        .bind(5i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // rule_id=2 with escalation_level=3
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(2i32))
        .bind("rule2")
        .bind(now)
        .bind(3600i64)
        .bind(3i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        let escalation = manager.escalation_counts.get(&ip);
        assert!(
            escalation.is_some(),
            "escalation_counts should contain the IP"
        );
        assert_eq!(
            escalation.unwrap().0,
            5,
            "should restore the maximum escalation_level (5 > 3)"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_empty_db() {
        let pool = create_pool_and_schema().await;

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        assert!(
            manager.escalation_counts.is_empty(),
            "escalation_counts should be empty when no bans exist"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_single_ban() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let now = chrono::Utc::now();

        // Insert 1 ban with escalation_level=7
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("single")
        .bind(now)
        .bind(3600i64)
        .bind(7i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        let escalation = manager.escalation_counts.get(&ip);
        assert!(
            escalation.is_some(),
            "escalation_counts should contain the IP"
        );
        assert_eq!(
            escalation.unwrap().0,
            7,
            "should restore the escalation_level for a single ban"
        );
    }

    // ------------------------------------------------------------------
    // load_from_db profile multipliers tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_load_from_db_uses_profile_multipliers() {
        let pool = create_pool_and_schema().await;
        let now = chrono::Utc::now();

        // Insert Path Scanning profile: bantime_multipliers=[1,2,4,8,16,32], maxtime=86400
        sqlx::query(
            "INSERT INTO rate_limit_profiles (id, name, description, max_retry, find_time_seconds, \
             ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, \
             ban_count_decay_days, fail_codes, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(3i32)
        .bind("Path Scanning")
        .bind("Test profile")
        .bind(20i32)
        .bind(60i32)
        .bind(300i32)
        .bind(1i32)
        .bind("[1,2,4,8,16,32]")
        .bind(86400i32)
        .bind(30i32)
        .bind("[403,404]")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Insert rule referencing the profile
        sqlx::query(
            "INSERT INTO rules (id, name, description, weight, mode, pipeline, allow, \
             rate_limit_profile_id, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1i32)
        .bind("test-rule")
        .bind("")
        .bind(100i32)
        .bind("enforce")
        .bind("jail")
        .bind(0i32)
        .bind(Some(3i32))
        .bind(1i32)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Insert ban referencing rule_id=1
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("test ban")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        assert_eq!(
            manager.bantime_multipliers,
            vec![1, 2, 4, 8, 16, 32],
            "should use multipliers from Path Scanning profile"
        );
        assert_eq!(
            manager.bantime_maxtime, 86400,
            "should use maxtime from Path Scanning profile"
        );
        assert_eq!(
            manager.ban_count_decay_days, 30,
            "should use decay_days from Path Scanning profile"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_defaults_when_profile_missing() {
        let pool = create_pool_and_schema().await;
        let now = chrono::Utc::now();

        // Insert ban referencing rule_id=999 which does NOT exist
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(999i32))
        .bind("orphan ban")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        assert_eq!(
            manager.bantime_multipliers,
            vec![1, 2, 4, 8],
            "should use default multipliers when profile missing"
        );
        assert_eq!(
            manager.bantime_maxtime, 604_800,
            "should use default maxtime when profile missing"
        );
        assert_eq!(
            manager.ban_count_decay_days, 30,
            "should use default decay_days when profile missing"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_uses_highest_maxtime_across_profiles() {
        let pool = create_pool_and_schema().await;
        let now = chrono::Utc::now();

        // Profile A: bantime_maxtime=86400 (Path Scanning)
        sqlx::query(
            "INSERT INTO rate_limit_profiles (id, name, description, max_retry, find_time_seconds, \
             ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, \
             ban_count_decay_days, fail_codes, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1i32)
        .bind("Profile A")
        .bind("")
        .bind(20i32)
        .bind(60i32)
        .bind(300i32)
        .bind(1i32)
        .bind("[1,2,4,8]")
        .bind(86400i32)
        .bind(30i32)
        .bind("[403,404]")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Profile B: bantime_maxtime=604800 (Auth Brute Force — higher)
        sqlx::query(
            "INSERT INTO rate_limit_profiles (id, name, description, max_retry, find_time_seconds, \
             ban_time_seconds, bantime_increment, bantime_multipliers, bantime_maxtime_seconds, \
             ban_count_decay_days, fail_codes, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(2i32)
        .bind("Profile B")
        .bind("")
        .bind(5i32)
        .bind(300i32)
        .bind(900i32)
        .bind(1i32)
        .bind("[1,2,4,8]")
        .bind(604_800i32)
        .bind(30i32)
        .bind("[401]")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Rule A referencing profile A
        sqlx::query(
            "INSERT INTO rules (id, name, description, weight, mode, pipeline, allow, \
             rate_limit_profile_id, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1i32)
        .bind("rule-a")
        .bind("")
        .bind(100i32)
        .bind("enforce")
        .bind("jail")
        .bind(0i32)
        .bind(Some(1i32))
        .bind(1i32)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Rule B referencing profile B
        sqlx::query(
            "INSERT INTO rules (id, name, description, weight, mode, pipeline, allow, \
             rate_limit_profile_id, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(2i32)
        .bind("rule-b")
        .bind("")
        .bind(100i32)
        .bind("enforce")
        .bind("jail")
        .bind(0i32)
        .bind(Some(2i32))
        .bind(1i32)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Ban for IP 1.2.3.4 with rule_id=1 (Profile A, maxtime=86400)
        let ip1: IpAddr = "1.2.3.4".parse().unwrap();
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip1.to_string())
        .bind(Some(1i32))
        .bind("ban for rule A")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Ban for IP 5.6.7.8 with rule_id=2 (Profile B, maxtime=604800)
        let ip2: IpAddr = "5.6.7.8".parse().unwrap();
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip2.to_string())
        .bind(Some(2i32))
        .bind("ban for rule B")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        // The manager should use Profile B's values (highest maxtime)
        assert_eq!(
            manager.bantime_maxtime, 604_800,
            "should use highest bantime_maxtime across all referenced profiles"
        );
        // bantime_multipliers should also come from the profile with highest maxtime
        assert_eq!(
            manager.bantime_multipliers,
            vec![1, 2, 4, 8],
            "multipliers should come from the profile with highest maxtime"
        );
    }

    // ------------------------------------------------------------------
    // load_from_db duplicate cleanup tests (Task 4)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_load_from_db_marks_duplicates_expired() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        let earliest = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let middle = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let latest = Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap();

        // Insert 3 duplicate rows for same IP + rule_id=1, different banned_at
        for (banned_at, reason) in [(earliest, "oldest"), (middle, "middle"), (latest, "newest")] {
            sqlx::query(
                "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
                 ban_duration_seconds, escalation_level, expired, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
            )
            .bind(ip.to_string())
            .bind(Some(1i32))
            .bind(reason)
            .bind(banned_at)
            .bind(3600i64)
            .bind(0i32)
            .bind(banned_at)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Act: load_from_db should deduplicate and clean up DB
        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        // Assert: only 1 ban loaded in memory (the most recent)
        let bans = manager.bans_for_ip(&ip);
        assert_eq!(bans.len(), 1, "should load only 1 ban per (ip, rule_id)");
        assert_eq!(bans[0].reason, "newest", "should keep the most recent ban");

        // Assert: DB has only 1 active (expired=0) row
        let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            active_count, 1,
            "only 1 row should remain active in DB after cleanup"
        );

        // Assert: DB has 2 expired (expired=1) rows
        let expired_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            expired_count, 2,
            "2 duplicate rows should be marked as expired=1"
        );

        // Assert: the remaining active row is the newest one
        let active_reason: String = sqlx::query_scalar("SELECT reason FROM bans WHERE expired = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            active_reason, "newest",
            "active row should be the newest ban"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_does_not_affect_unique_bans() {
        let pool = create_pool_and_schema().await;
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let now = Utc::now();

        // Insert 1 single ban for IP + rule_id
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip.to_string())
        .bind(Some(1i32))
        .bind("single-ban")
        .bind(now)
        .bind(3600i64)
        .bind(0i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Act: load_from_db
        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        // Assert: 1 ban loaded in memory
        let bans = manager.bans_for_ip(&ip);
        assert_eq!(bans.len(), 1, "should load the single ban");
        assert_eq!(bans[0].reason, "single-ban");

        // Assert: DB still has 1 active row, 0 expired rows
        let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            active_count, 1,
            "unique ban should remain active (expired=0)"
        );

        let expired_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bans WHERE expired = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            expired_count, 0,
            "no rows should be marked as expired for a unique ban"
        );
    }

    #[tokio::test]
    async fn test_load_from_db_multiple_ips() {
        let pool = create_pool_and_schema().await;
        let ip1: IpAddr = "1.2.3.4".parse().unwrap();
        let ip2: IpAddr = "5.6.7.8".parse().unwrap();
        let now = chrono::Utc::now();

        // Insert ban for IP 1.2.3.4 with escalation_level=5
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip1.to_string())
        .bind(Some(1i32))
        .bind("first-ip")
        .bind(now)
        .bind(3600i64)
        .bind(5i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        // Insert ban for IP 5.6.7.8 with escalation_level=3
        sqlx::query(
            "INSERT INTO bans (ip_address, rule_id, reason, banned_at, \
             ban_duration_seconds, escalation_level, expired, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(ip2.to_string())
        .bind(Some(2i32))
        .bind("second-ip")
        .bind(now)
        .bind(3600i64)
        .bind(3i32)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let (manager, _) = BanManager::load_from_db(&pool).await.unwrap();

        let escalation1 = manager.escalation_counts.get(&ip1);
        assert!(
            escalation1.is_some(),
            "escalation_counts should contain IP 1.2.3.4"
        );
        assert_eq!(
            escalation1.unwrap().0,
            5,
            "IP 1.2.3.4 should have escalation_level 5"
        );

        let escalation2 = manager.escalation_counts.get(&ip2);
        assert!(
            escalation2.is_some(),
            "escalation_counts should contain IP 5.6.7.8"
        );
        assert_eq!(
            escalation2.unwrap().0,
            3,
            "IP 5.6.7.8 should have escalation_level 3"
        );
    }
}

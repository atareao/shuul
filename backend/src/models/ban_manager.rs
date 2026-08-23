//! # Ban Manager
//!
//! Manages active IP bans with escalation and decay.
//! Bans are enforced at the HTTP level — no firewall backend needed.

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
            total - elapsed
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
    /// Create a new BanManager with default settings.
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
    pub fn is_banned(&self, ip: &IpAddr) -> Option<&BanInfo> {
        self.bans.get(ip).and_then(|ban_list| {
            ban_list.iter().find(|ban| !ban.is_expired())
        })
    }

    /// Ban an IP address.
    ///
    /// If `ban_duration_override` is `Some`, it is used as the ban duration
    /// instead of the calculated escalation-based duration.
    pub fn ban(&mut self, ip: IpAddr, rule_id: Option<i32>, reason: String, ban_duration_override: Option<i64>) -> &BanInfo {
        let escalation_level = self.get_escalation_level(&ip);
        let duration = ban_duration_override.unwrap_or_else(|| self.calculate_ban_duration(escalation_level));

        let ban_info = BanInfo {
            banned_at: Instant::now(),
            ban_duration_seconds: duration,
            escalation_level,
            rule_id,
            reason,
        };

        self.bans.entry(ip).or_default().push(ban_info.clone());
        self.increment_escalation(&ip);

        // Return reference to the last added ban
        self.bans.get(&ip).unwrap().last().unwrap()
    }

    /// Unban an IP for a specific rule. Returns true if anything was removed.
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

    /// Remove all expired bans.
    pub fn cleanup_expired(&mut self) {
        self.bans.retain(|_, ban_list| {
            ban_list.retain(|b| !b.is_expired());
            !ban_list.is_empty()
        });
        // Decay escalation counters
        let decay_duration = Duration::from_secs(self.ban_count_decay_days as u64 * 86400);
        self.escalation_counts.retain(|_, (_, last_ban)| {
            Instant::now().duration_since(*last_ban) <= decay_duration
        });
    }

    /// Get all active (non-expired) bans.
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
    pub fn active_count(&self) -> usize {
        self.active_bans().len()
    }

    /// Calculate ban duration based on escalation level.
    fn calculate_ban_duration(&self, escalation_level: u32) -> i64 {
        if !self.bantime_increment || escalation_level == 0 {
            return self.default_ban_duration;
        }

        let multiplier_idx = (escalation_level as usize).min(self.bantime_multipliers.len() - 1);
        let multiplier = self.bantime_multipliers[multiplier_idx] as i64;
        let duration = self.default_ban_duration * multiplier;

        duration.min(self.bantime_maxtime)
    }

    /// Get the current escalation level for an IP.
    fn get_escalation_level(&self, ip: &IpAddr) -> u32 {
        self.escalation_counts
            .get(ip)
            .map(|(level, _)| *level)
            .unwrap_or(0)
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
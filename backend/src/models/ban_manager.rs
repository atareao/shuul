//! # Ban Manager
//!
//! Manages active IP bans with escalation and decay.
//! Bans are enforced at the HTTP level — no firewall backend needed.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BanSettings {
    pub ban_time_seconds: i64,
    pub bantime_increment: bool,
    pub bantime_multipliers: Vec<u32>,
    pub bantime_maxtime_seconds: i64,
    pub ban_count_decay_days: i64,
}

impl Default for BanSettings {
    fn default() -> Self {
        Self {
            ban_time_seconds: 3600,
            bantime_increment: false,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604_800,
            ban_count_decay_days: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct BanScope {
    ip: IpAddr,
    rule_id: Option<i32>,
}

/// Information about an active ban.
#[derive(Debug, Clone)]
pub struct BanInfo {
    /// When the ban was issued
    pub banned_at: DateTime<Utc>,
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
        self.expires_at() <= Utc::now()
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.banned_at + ChronoDuration::seconds(self.ban_duration_seconds)
    }

    /// Returns the remaining duration as a human-friendly string.
    #[allow(dead_code)]
    pub fn time_remaining(&self) -> Duration {
        let remaining = self.expires_at() - Utc::now();
        if remaining.num_seconds() <= 0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(remaining.num_seconds() as u64)
        }
    }
}

#[derive(Debug, Clone)]
struct EscalationState {
    level: u32,
    last_ban: DateTime<Utc>,
    decay_days: i64,
}

/// Manages all active bans, with escalation and decay logic.
#[derive(Debug, Clone)]
pub struct BanManager {
    /// Active bans keyed by IP address
    bans: HashMap<IpAddr, Vec<BanInfo>>,
    /// Per-IP/rule escalation counters (decays over time)
    escalation_counts: HashMap<BanScope, EscalationState>,
}

impl Default for BanManager {
    fn default() -> Self {
        Self {
            bans: HashMap::new(),
            escalation_counts: HashMap::new(),
        }
    }
}

impl BanManager {
    /// Create a new BanManager with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an IP is currently banned.
    /// Returns the first active ban info, or None.
    pub fn is_banned(&self, ip: &IpAddr) -> Option<&BanInfo> {
        self.bans
            .get(ip)
            .and_then(|ban_list| ban_list.iter().find(|ban| !ban.is_expired()))
    }

    /// Ban an IP address.
    ///
    /// If `ban_duration_override` is `Some`, it is used as the ban duration
    /// instead of the calculated escalation-based duration.
    pub fn restore(&mut self, ip: IpAddr, ban_info: BanInfo, decay_days: i64) {
        let scope = BanScope {
            ip,
            rule_id: ban_info.rule_id,
        };
        self.escalation_counts
            .entry(scope)
            .and_modify(|state| {
                state.level = state.level.max(ban_info.escalation_level + 1);
                state.last_ban = state.last_ban.max(ban_info.banned_at);
            })
            .or_insert(EscalationState {
                level: ban_info.escalation_level + 1,
                last_ban: ban_info.banned_at,
                decay_days,
            });
        self.bans.entry(ip).or_default().push(ban_info);
    }

    pub fn ban(
        &mut self,
        ip: IpAddr,
        rule_id: Option<i32>,
        reason: String,
        settings: &BanSettings,
        ban_duration_override: Option<i64>,
    ) -> &BanInfo {
        let scope = BanScope { ip, rule_id };
        let escalation_level = self.get_escalation_level(scope, settings);
        let duration = ban_duration_override
            .unwrap_or_else(|| self.calculate_ban_duration(escalation_level, settings));

        let ban_info = BanInfo {
            banned_at: Utc::now(),
            ban_duration_seconds: duration,
            escalation_level,
            rule_id,
            reason,
        };

        self.bans.entry(ip).or_default().push(ban_info.clone());
        self.increment_escalation(scope, settings);

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
        self.escalation_counts.retain(|_, state| {
            let expires_at = state.last_ban + ChronoDuration::days(state.decay_days);
            Utc::now() <= expires_at
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
    fn calculate_ban_duration(&self, escalation_level: u32, settings: &BanSettings) -> i64 {
        if !settings.bantime_increment || escalation_level == 0 {
            return settings.ban_time_seconds;
        }

        let multiplier_idx =
            (escalation_level as usize).min(settings.bantime_multipliers.len() - 1);
        let multiplier = settings.bantime_multipliers[multiplier_idx] as i64;
        let duration = settings.ban_time_seconds * multiplier;

        duration.min(settings.bantime_maxtime_seconds)
    }

    /// Get the current escalation level for an IP.
    fn get_escalation_level(&self, scope: BanScope, settings: &BanSettings) -> u32 {
        self.escalation_counts
            .get(&scope)
            .and_then(|state| {
                let expires_at =
                    state.last_ban + ChronoDuration::days(settings.ban_count_decay_days);
                (Utc::now() <= expires_at).then_some(state.level)
            })
            .unwrap_or(0)
    }

    /// Increment the escalation counter for an IP.
    fn increment_escalation(&mut self, scope: BanScope, settings: &BanSettings) {
        let entry = self
            .escalation_counts
            .entry(scope)
            .or_insert(EscalationState {
                level: 0,
                last_ban: Utc::now(),
                decay_days: settings.ban_count_decay_days,
            });

        let expires_at = entry.last_ban + ChronoDuration::days(entry.decay_days);
        if Utc::now() > expires_at {
            entry.level = 0;
        }

        entry.level += 1;
        entry.last_ban = Utc::now();
        entry.decay_days = settings.ban_count_decay_days;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ban_and_check() {
        let mut bm = BanManager::new();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        assert!(bm.is_banned(&ip).is_none());
        bm.ban(
            ip,
            None,
            "test ban".to_string(),
            &BanSettings::default(),
            None,
        );
        assert!(bm.is_banned(&ip).is_some());
    }

    #[test]
    fn test_unban() {
        let mut bm = BanManager::new();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        bm.ban(
            ip,
            Some(1),
            "test".to_string(),
            &BanSettings::default(),
            None,
        );
        assert!(bm.unban(&ip, Some(1)));
        assert!(bm.is_banned(&ip).is_none());
    }

    #[test]
    fn test_escalation() {
        let mut bm = BanManager::new();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let settings = BanSettings {
            ban_time_seconds: 3600,
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 86400,
            ban_count_decay_days: 30,
        };

        // First offense: 3600s (multiplier 1)
        let ban1 = bm.ban(ip, None, "1st".to_string(), &settings, None).clone();
        assert_eq!(ban1.ban_duration_seconds, 3600);

        // Second offense: 7200s (multiplier 2)
        let ban2 = bm.ban(ip, None, "2nd".to_string(), &settings, None).clone();
        assert_eq!(ban2.ban_duration_seconds, 7200);

        // Third offense: 14400s (multiplier 4)
        let ban3 = bm.ban(ip, None, "3rd".to_string(), &settings, None).clone();
        assert_eq!(ban3.ban_duration_seconds, 14400);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut bm = BanManager::new();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        let settings = BanSettings {
            ban_time_seconds: 1,
            ..BanSettings::default()
        };
        bm.ban(ip, None, "short ban".to_string(), &settings, None);
        assert_eq!(bm.active_count(), 1);

        std::thread::sleep(Duration::from_millis(1100));
        bm.cleanup_expired();
        assert_eq!(bm.active_count(), 0);
    }

    #[test]
    fn test_escalation_is_scoped_per_rule() {
        let mut bm = BanManager::new();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let settings = BanSettings {
            ban_time_seconds: 60,
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4],
            bantime_maxtime_seconds: 240,
            ban_count_decay_days: 30,
        };

        let rule_one_first = bm
            .ban(ip, Some(1), "r1".to_string(), &settings, None)
            .clone();
        let rule_two_first = bm
            .ban(ip, Some(2), "r2".to_string(), &settings, None)
            .clone();
        let rule_one_second = bm
            .ban(ip, Some(1), "r1 again".to_string(), &settings, None)
            .clone();

        assert_eq!(rule_one_first.escalation_level, 0);
        assert_eq!(rule_two_first.escalation_level, 0);
        assert_eq!(rule_one_second.escalation_level, 1);
        assert_eq!(rule_one_second.ban_duration_seconds, 120);
    }
}

//! Integration tests for rate limiting and ban functionality.

use backend::models::{BanManager, BanSettings, CircularTimestamps, RateLimiter};
use std::net::IpAddr;

#[tokio::test]
async fn test_rate_limiter_integration() {
    let mut rl = RateLimiter::new(3, 10);
    let ip: IpAddr = "10.0.0.1".parse().unwrap();

    // First two requests should not trigger ban
    assert!(!rl.record(ip));
    assert!(!rl.record(ip));

    // Third request within window should trigger
    assert!(rl.record(ip));
}

#[tokio::test]
async fn test_ban_manager_escalation() {
    let mut bm = BanManager::new();
    let ip: IpAddr = "10.0.0.2".parse().unwrap();
    let settings = BanSettings {
        ban_time_seconds: 3600,
        bantime_increment: true,
        bantime_multipliers: vec![1, 2, 4, 8],
        bantime_maxtime_seconds: 604800,
        ban_count_decay_days: 30,
    };

    // First ban: 3600s
    let b1 = bm.ban(ip, Some(1), "test".to_string(), &settings, None);
    assert_eq!(b1.escalation_level, 0);
    assert_eq!(b1.ban_duration_seconds, 3600);

    // Second ban: 7200s (2x)
    let b2 = bm.ban(ip, Some(1), "test".to_string(), &settings, None);
    assert_eq!(b2.escalation_level, 1);
    assert_eq!(b2.ban_duration_seconds, 7200);

    // Third ban: 14400s (4x)
    let b3 = bm.ban(ip, Some(1), "test".to_string(), &settings, None);
    assert_eq!(b3.escalation_level, 2);
    assert_eq!(b3.ban_duration_seconds, 14400);
}

#[tokio::test]
async fn test_ban_unban_cycle() {
    let mut bm = BanManager::new();
    let ip: IpAddr = "10.0.0.3".parse().unwrap();

    assert!(bm.is_banned(&ip).is_none());
    bm.ban(
        ip,
        Some(1),
        "test".to_string(),
        &BanSettings::default(),
        None,
    );
    assert!(bm.is_banned(&ip).is_some());
    bm.unban(&ip, Some(1));
    assert!(bm.is_banned(&ip).is_none());
}

#[tokio::test]
async fn test_circular_timestamps_wraparound() {
    let mut ring = CircularTimestamps::new(3);
    let now = std::time::Instant::now();

    // Fill buffer
    ring.push(now);
    ring.push(now + std::time::Duration::from_secs(1));
    ring.push(now + std::time::Duration::from_secs(2));
    assert!(ring.threshold_reached(std::time::Duration::from_secs(10)));

    // Push more (wraparound)
    ring.push(now + std::time::Duration::from_secs(100));
    ring.push(now + std::time::Duration::from_secs(101));
    ring.push(now + std::time::Duration::from_secs(102));
    assert!(ring.threshold_reached(std::time::Duration::from_secs(10)));
}

#[tokio::test]
async fn test_ban_manager_scopes_escalation_by_rule() {
    let mut bm = BanManager::new();
    let ip: IpAddr = "10.0.0.4".parse().unwrap();
    let settings = BanSettings {
        ban_time_seconds: 60,
        bantime_increment: true,
        bantime_multipliers: vec![1, 2, 4],
        bantime_maxtime_seconds: 240,
        ban_count_decay_days: 30,
    };

    assert_eq!(
        bm.ban(ip, Some(1), "rule-1".to_string(), &settings, None)
            .escalation_level,
        0
    );
    assert_eq!(
        bm.ban(ip, Some(2), "rule-2".to_string(), &settings, None)
            .escalation_level,
        0
    );
    assert_eq!(
        bm.ban(ip, Some(1), "rule-1 again".to_string(), &settings, None)
            .ban_duration_seconds,
        120
    );
}

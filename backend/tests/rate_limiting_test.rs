//! Integration tests for rate limiting and ban functionality.

use std::net::IpAddr;
use backend::models::{BanManager, CircularTimestamps, RateLimiter};

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
    let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 604800, 30);
    let ip: IpAddr = "10.0.0.2".parse().unwrap();

    // First ban: 3600s
    let b1 = bm.ban(ip, Some(1), "test".to_string());
    assert_eq!(b1.escalation_level, 0);
    assert_eq!(b1.ban_duration_seconds, 3600);

    // Second ban: 7200s (2x)
    let b2 = bm.ban(ip, Some(1), "test".to_string());
    assert_eq!(b2.escalation_level, 1);
    assert_eq!(b2.ban_duration_seconds, 7200);

    // Third ban: 14400s (4x)
    let b3 = bm.ban(ip, Some(1), "test".to_string());
    assert_eq!(b3.escalation_level, 2);
    assert_eq!(b3.ban_duration_seconds, 14400);
}

#[tokio::test]
async fn test_ban_unban_cycle() {
    let mut bm = BanManager::new(3600, false, vec![1], 3600, 30);
    let ip: IpAddr = "10.0.0.3".parse().unwrap();

    assert!(bm.is_banned(&ip).is_none());
    bm.ban(ip, Some(1), "test".to_string());
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
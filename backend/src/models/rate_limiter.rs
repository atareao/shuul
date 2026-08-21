//! # Rate Limiter
//!
//! Circular ring buffer for tracking request timestamps per IP,
//! and a rate limiter that checks thresholds against sliding windows.
//!
//! Inspired by fail2ban-rs's `CircularTimestamps`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// A fixed-size circular buffer of timestamps for a single IP.
///
/// Stores only the last `capacity` timestamps. `threshold_reached()`
/// returns true if the buffer is full AND the oldest timestamp falls
/// within `find_time` seconds of the newest.
#[derive(Debug, Clone)]
pub struct CircularTimestamps {
    timestamps: Vec<Instant>,
    capacity: usize,
    head: usize,
    count: usize,
}

impl CircularTimestamps {
    /// Create a new ring buffer that tracks up to `capacity` timestamps.
    pub fn new(capacity: usize) -> Self {
        let now = Instant::now();
        Self {
            timestamps: vec![now; capacity],
            capacity,
            head: 0,
            count: 0,
        }
    }

    /// Push a new timestamp, overwriting the oldest if at capacity.
    pub fn push(&mut self, now: Instant) {
        self.timestamps[self.head] = now;
        self.head = (self.head + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    /// Returns `true` if the buffer has reached capacity AND the oldest
    /// timestamp is within `find_time` seconds of the newest.
    ///
    /// This means the IP has exceeded `max_retry` within the sliding window.
    pub fn threshold_reached(&self, find_time: Duration) -> bool {
        if self.count < self.capacity {
            return false;
        }
        // The oldest entry is at head (next to be overwritten)
        let oldest = self.timestamps[self.head];
        let newest = self.timestamps[(self.head + self.capacity - 1) % self.capacity];
        newest.duration_since(oldest) <= find_time
    }

    /// Number of timestamps stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Per-rule rate limiter. Maps IP addresses to circular timestamp buffers.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Per-IP ring buffers
    ip_buffers: HashMap<IpAddr, CircularTimestamps>,
    /// Maximum retries before ban
    max_retry: u32,
    /// Sliding window duration in seconds
    find_time_seconds: i64,
}

impl RateLimiter {
    /// Create a new rate limiter with the given threshold.
    pub fn new(max_retry: u32, find_time_seconds: i64) -> Self {
        Self {
            ip_buffers: HashMap::new(),
            max_retry,
            find_time_seconds,
        }
    }

    /// Record a request from `ip`. Returns `true` if the threshold is reached
    /// (IP should be banned).
    pub fn record(&mut self, ip: IpAddr) -> bool {
        let capacity = self.max_retry as usize;
        let buffer = self
            .ip_buffers
            .entry(ip)
            .or_insert_with(|| CircularTimestamps::new(capacity));
        buffer.push(Instant::now());
        buffer.threshold_reached(Duration::from_secs(self.find_time_seconds as u64))
    }

    /// Remove expired entries to prevent memory leaks.
    /// Entries whose newest timestamp is older than `find_time` are removed.
    pub fn cleanup_expired(&mut self) {
        let find_time = Duration::from_secs(self.find_time_seconds as u64);
        self.ip_buffers.retain(|_, buffer| {
            if buffer.is_empty() {
                return false;
            }
            // Keep if the newest entry is still within the window
            let newest_idx = (buffer.head + buffer.capacity - 1) % buffer.capacity;
            let newest = buffer.timestamps[newest_idx];
            Instant::now().duration_since(newest) <= find_time
        });
    }

    /// Remove a specific IP from the rate limiter (e.g., after unban).
    pub fn remove_ip(&mut self, ip: &IpAddr) {
        self.ip_buffers.remove(ip);
    }

    /// Number of tracked IPs.
    pub fn len(&self) -> usize {
        self.ip_buffers.len()
    }

    /// Whether no IPs are tracked.
    pub fn is_empty(&self) -> bool {
        self.ip_buffers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_circular_timestamps_below_threshold() {
        let mut ring = CircularTimestamps::new(3);
        let now = Instant::now();
        ring.push(now);
        ring.push(now + Duration::from_secs(1));
        // Only 2 entries, capacity is 3
        assert!(!ring.threshold_reached(Duration::from_secs(10)));
    }

    #[test]
    fn test_circular_timestamps_at_threshold() {
        let mut ring = CircularTimestamps::new(3);
        let now = Instant::now();
        ring.push(now);
        ring.push(now + Duration::from_secs(1));
        ring.push(now + Duration::from_secs(2));
        // 3 entries, all within 10s window
        assert!(ring.threshold_reached(Duration::from_secs(10)));
    }

    #[test]
    fn test_circular_timestamps_outside_window() {
        let mut ring = CircularTimestamps::new(3);
        let now = Instant::now();
        ring.push(now);
        ring.push(now + Duration::from_secs(100));
        ring.push(now + Duration::from_secs(200));
        // 3 entries, but spread beyond 10s window
        assert!(!ring.threshold_reached(Duration::from_secs(10)));
    }

    #[test]
    fn test_rate_limiter_record() {
        let mut rl = RateLimiter::new(3, 10);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        assert!(!rl.record(ip)); // 1st
        assert!(!rl.record(ip)); // 2nd
        assert!(rl.record(ip));  // 3rd → threshold reached
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let mut rl = RateLimiter::new(3, 1); // 1 second window
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        rl.record(ip);
        assert_eq!(rl.len(), 1);

        // After cleanup with very short window, should be removed
        std::thread::sleep(Duration::from_millis(1100));
        rl.cleanup_expired();
        assert_eq!(rl.len(), 0);
    }
}
//! # Rate Limiter
//!
//! Circular ring buffer for tracking request timestamps per IP,
//! and a rate limiter that checks thresholds against sliding windows.
//!
//! Inspired by fail2ban-rs's `CircularTimestamps`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant, SystemTime};

use sqlx::SqlitePool;

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
    #[must_use]
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
    #[must_use]
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
    #[allow(dead_code)]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
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
    find_time_seconds: i32,
}

impl RateLimiter {
    /// Create a new rate limiter with the given threshold.
    #[must_use]
    pub fn new(max_retry: u32, find_time_seconds: i32) -> Self {
        Self {
            ip_buffers: HashMap::new(),
            max_retry,
            find_time_seconds,
        }
    }

    /// Record a request from `ip`. Returns `true` if the threshold is reached
    /// (IP should be banned).
    #[allow(clippy::cast_sign_loss)]
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
    #[allow(clippy::cast_sign_loss)]
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
    #[allow(dead_code)]
    pub fn remove_ip(&mut self, ip: &IpAddr) {
        self.ip_buffers.remove(ip);
    }

    /// Number of tracked IPs.
    #[allow(dead_code)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.ip_buffers.len()
    }

    /// Whether no IPs are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ip_buffers.is_empty()
    }

    /// Persist all ring buffers to `SQLite` for this profile.
    ///
    /// Serializes each [`CircularTimestamps`] as a JSON array of epoch-millis values
    /// and UPSERTs into `rate_limiter_state`. Skips if no IPs are tracked.
    ///
    /// # Errors
    ///
    /// Returns `sqlx::Error` if the database query fails.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    pub async fn save(&self, pool: &SqlitePool, profile_id: i32) -> Result<(), sqlx::Error> {
        if self.ip_buffers.is_empty() {
            return Ok(());
        }

        let mut tx = pool.begin().await?;

        for (ip, buffer) in &self.ip_buffers {
            // Extract valid timestamps in chronological order (head → wrap)
            let epoch_millis: Vec<i64> = (0..buffer.count)
                .map(|i| {
                    let idx = (buffer.head + i) % buffer.capacity;
                    instant_to_epoch_millis(&buffer.timestamps[idx])
                })
                .collect();

            let timestamps_json =
                serde_json::to_string(&epoch_millis).unwrap_or_else(|_| "[]".to_string());

            let ip_str = ip.to_string();
            let now_epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let now_epoch: i64 = now_epoch.try_into().unwrap_or(0);

            sqlx::query(
                "INSERT INTO rate_limiter_state \
                 (profile_id, ip_address, timestamps, head, count, capacity, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(profile_id, ip_address) DO UPDATE SET \
                 timestamps = excluded.timestamps, \
                 head = excluded.head, \
                 count = excluded.count, \
                 capacity = excluded.capacity, \
                 updated_at = excluded.updated_at",
            )
            .bind(profile_id)
            .bind(&ip_str)
            .bind(&timestamps_json)
            .bind(0i32) // head is always 0 because we serialize in chronological order
            .bind(i32::try_from(epoch_millis.len()).unwrap_or(0))
            .bind(i32::try_from(buffer.capacity).unwrap_or(0))
            .bind(now_epoch.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await
    }

    /// Load a persisted [`RateLimiter`] from `SQLite`.
    ///
    /// Deserializes ring buffers from `rate_limiter_state`, filtering out
    /// timestamps that have fallen outside the `find_time_seconds` window.
    /// Returns an empty [`RateLimiter`] if no data exists for `profile_id`.
    ///
    /// # Errors
    ///
    /// Returns `sqlx::Error` if the database query fails.
    #[allow(
        dead_code,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless,
        clippy::cast_possible_wrap
    )]
    pub async fn load(
        pool: &SqlitePool,
        profile_id: i32,
        max_retry: u32,
        find_time_seconds: i32,
    ) -> Result<Self, sqlx::Error> {
        let rows: Vec<(String, String, i32, i32)> = sqlx::query_as(
            "SELECT ip_address, timestamps, count, capacity \
             FROM rate_limiter_state WHERE profile_id = ?",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await?;

        let now_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let window_ms = (find_time_seconds as i64) * 1000;

        let mut ip_buffers: HashMap<IpAddr, CircularTimestamps> = HashMap::new();

        for (ip_str, timestamps_json, _stored_count, capacity) in rows {
            let epoch_millis: Vec<i64> = serde_json::from_str(&timestamps_json).unwrap_or_default();

            let ip: IpAddr = match ip_str.parse() {
                Ok(ip) => ip,
                Err(_) => continue,
            };

            // Filter out expired timestamps
            let valid_ts: Vec<i64> = epoch_millis
                .into_iter()
                .filter(|ts| now_epoch - ts < window_ms)
                .collect();

            if valid_ts.is_empty() {
                continue;
            }

            let cap = capacity.max(1) as usize;
            let mut buffer = CircularTimestamps::new(cap);
            for ts in valid_ts {
                buffer.push(epoch_millis_to_instant(ts));
            }

            ip_buffers.insert(ip, buffer);
        }

        Ok(Self {
            ip_buffers,
            max_retry,
            find_time_seconds,
        })
    }
}

/// Convert an [`Instant`] to epoch milliseconds for serialization.
///
/// Uses [`SystemTime::now()`] as a reference point to convert the monotonic
/// [`Instant`] into a wall-clock timestamp.
#[allow(dead_code, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn instant_to_epoch_millis(instant: &Instant) -> i64 {
    let now_instant = Instant::now();
    let now_system = SystemTime::now();
    let elapsed = now_instant.duration_since(*instant);
    let then_system = now_system
        .checked_sub(elapsed)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let millis = then_system
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.try_into().unwrap_or(i64::MAX)
}

/// Convert epoch milliseconds back to an [`Instant`].
///
/// Uses [`SystemTime::now()`] as a reference to reconstruct the monotonic
/// [`Instant`] from a wall-clock epoch-millis timestamp.
#[allow(clippy::cast_sign_loss)]
fn epoch_millis_to_instant(epoch_millis: i64) -> Instant {
    let now_system = SystemTime::now();
    let now_instant = Instant::now();
    let millis = u64::try_from(epoch_millis).unwrap_or(u64::MAX);
    let then_system = SystemTime::UNIX_EPOCH + Duration::from_millis(millis);
    let elapsed = now_system.duration_since(then_system).unwrap_or_default();
    now_instant.checked_sub(elapsed).unwrap_or(now_instant)
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
        assert!(rl.record(ip)); // 3rd → threshold reached
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

/// Integration tests for the `rate_limiter_state` migration.
///
/// Verifies that the SQL migration creates the `rate_limiter_state` table
/// with the expected schema.
#[cfg(test)]
mod db_tests {
    use super::*;
    use crate::models::CacheRule;
    use crate::models::RateLimitProfile;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Create an in-memory SQLite pool for testing.
    async fn create_pool() -> sqlx::SqlitePool {
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_migration_creates_rate_limiter_state_table() {
        let pool = create_pool().await;

        // Apply ALL migrations from the migrations directory
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let migrations_path = manifest_dir.join("migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_path)
            .await
            .expect("Failed to load migrations");
        migrator.run(&pool).await.expect("Failed to run migrations");

        // Verify the rate_limiter_state table exists
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='rate_limiter_state'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            count, 1,
            "rate_limiter_state table must exist after migrations"
        );
    }

    /// Helper: apply migrations to an in-memory pool.
    async fn apply_migrations(pool: &sqlx::SqlitePool) {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let migrations_path = manifest_dir.join("migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_path)
            .await
            .expect("Failed to load migrations");
        migrator.run(pool).await.expect("Failed to run migrations");
    }

    // ------------------------------------------------------------------
    //  RED tests for RateLimiter::save and RateLimiter::load
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_save_and_load_persists_ring_buffers() {
        // GIVEN a RateLimiter with max_retry=20, find_time=60s
        // AND records for IP 1.2.3.4 with 15 timestamps
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        let mut rl = RateLimiter::new(20, 60);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        for _ in 0..15 {
            rl.record(ip);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // WHEN save(pool, 3) → then load(pool, 3, 20, 60)
        rl.save(&pool, 3).await.unwrap();
        let loaded = RateLimiter::load(&pool, 3, 20, 60).await.unwrap();

        // THEN the loaded RateLimiter has the 15 timestamps for 1.2.3.4
        let buffer = loaded
            .ip_buffers
            .get(&ip)
            .expect("IP 1.2.3.4 should be in the loaded rate limiter");
        assert_eq!(
            buffer.len(),
            15,
            "Should have restored 15 timestamps for 1.2.3.4"
        );
    }

    #[tokio::test]
    async fn test_empty_rate_limiter_does_not_persist() {
        // GIVEN a new empty RateLimiter
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        let rl = RateLimiter::new(20, 60);

        // WHEN save(pool, 3)
        rl.save(&pool, 3).await.unwrap();

        // THEN no rows are inserted into rate_limiter_state
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limiter_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "Empty RateLimiter should not persist any rows");
    }

    #[tokio::test]
    async fn test_load_with_no_data_returns_empty() {
        // GIVEN the rate_limiter_state table is empty
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        // WHEN load(pool, 999, 20, 60)
        let loaded: RateLimiter = RateLimiter::load(&pool, 999, 20, 60).await.unwrap();

        // THEN the RateLimiter is empty (no IPs tracked)
        assert!(
            loaded.ip_buffers.is_empty(),
            "RateLimiter should be empty when no data exists for profile_id 999"
        );
    }

    #[tokio::test]
    async fn test_expired_timestamps_filtered_on_load() {
        // GIVEN rate_limiter_state has timestamps for IP 1.2.3.4,
        // some within and some outside the 60s window
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        let now = SystemTime::now();
        let epoch = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 10 recent timestamps (within 60s window): 0s to 45s ago
        let recent: Vec<i64> = (0..10)
            .map(|i| epoch - i * 5_000) // 5s apart, max 45s ago → within 60s
            .collect();

        // 5 old timestamps (outside 60s window): 120s+ ago
        let old: Vec<i64> = (0..5)
            .map(|i| epoch - 120_000 - i * 10_000) // 120s+ ago → outside 60s
            .collect();

        // Insert all 15 timestamps as a single entry (head=0, count=15, capacity=20)
        let all_ts: Vec<i64> = recent.iter().chain(old.iter()).copied().collect();
        let timestamps_json = serde_json::to_string(&all_ts).unwrap();

        sqlx::query(
            "INSERT INTO rate_limiter_state \
             (profile_id, ip_address, timestamps, head, count, capacity, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(3i32)
        .bind("1.2.3.4")
        .bind(&timestamps_json)
        .bind(0i32) // head
        .bind(all_ts.len() as i32) // count
        .bind(20i32) // capacity
        .bind(epoch.to_string())
        .execute(&pool)
        .await
        .unwrap();

        // WHEN load(pool, 3, 20, 60)
        let loaded = RateLimiter::load(&pool, 3, 20, 60).await.unwrap();

        // THEN only the within-window timestamps are loaded
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let buffer = loaded
            .ip_buffers
            .get(&ip)
            .expect("IP 1.2.3.4 should be in the loaded rate limiter");
        assert_eq!(
            buffer.len(),
            10,
            "Should have 10 non-expired timestamps for 1.2.3.4"
        );
    }

    // ------------------------------------------------------------------
    //  RED test for background task persistence (Task 7)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_background_task_persists_rate_limiters_periodically() {
        // GIVEN a RateLimiter for profile_id=3 with data for IP 1.2.3.4
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        let mut rl1 = RateLimiter::new(20, 60);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        for _ in 0..15 {
            rl1.record(ip);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // AND another RateLimiter for profile_id=5 with data for IP 5.6.7.8
        let mut rl2 = RateLimiter::new(10, 30);
        let ip2: IpAddr = "5.6.7.8".parse().unwrap();
        for _ in 0..5 {
            rl2.record(ip2);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // AND the rate_limiters are stored in a Mutex<HashMap> (simulating AppState)
        let rate_limiters: Mutex<HashMap<i32, RateLimiter>> = {
            let mut map = HashMap::new();
            map.insert(3, rl1);
            map.insert(5, rl2);
            Mutex::new(map)
        };

        // WHEN the background task logic executes (one iteration, without loop)
        {
            let locked = rate_limiters.lock().unwrap();
            let snapshots: Vec<(i32, RateLimiter)> =
                locked.iter().map(|(id, rl)| (*id, rl.clone())).collect();
            drop(locked); // release lock before .await

            for (profile_id, rl) in snapshots {
                rl.save(&pool, profile_id).await.unwrap();
            }
        }

        // THEN data is persisted in rate_limiter_state for both profile_ids
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limiter_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2, "Should have rows for both profile_ids");

        // AND profile_id=3 has 15 timestamps for 1.2.3.4
        let row_count_3: i64 = sqlx::query_scalar(
            "SELECT CAST(count AS INTEGER) FROM rate_limiter_state \
             WHERE profile_id = ? AND ip_address = ?",
        )
        .bind(3i32)
        .bind("1.2.3.4")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            row_count_3, 15,
            "profile_id=3 should have 15 timestamps for 1.2.3.4"
        );

        // AND profile_id=5 has 5 timestamps for 5.6.7.8
        let row_count_5: i64 = sqlx::query_scalar(
            "SELECT CAST(count AS INTEGER) FROM rate_limiter_state \
             WHERE profile_id = ? AND ip_address = ?",
        )
        .bind(5i32)
        .bind("5.6.7.8")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            row_count_5, 5,
            "profile_id=5 should have 5 timestamps for 5.6.7.8"
        );
    }

    // ------------------------------------------------------------------
    //  RED test for startup loading of persisted rate limiters (Task 8)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_startup_loads_rate_limiters_for_active_jail_rules() {
        // GIVEN an in-memory SQLite pool with migrations applied
        let pool = create_pool().await;
        apply_migrations(&pool).await;

        // AND rate limit profiles exist for profile_id=3 (Path Scanning)
        // and profile_id=5 (API Abuse) — these are seeded by migrations
        let now = chrono::Utc::now().to_rfc3339();

        // AND Jail active rules referencing those profile_ids
        sqlx::query(
            "INSERT INTO rules \
             (id, name, description, weight, mode, pipeline, allow, \
              rate_limit_profile_id, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(101i32)
        .bind("test-jail-3")
        .bind("")
        .bind(100i32)
        .bind("enforce")
        .bind("jail")
        .bind(false)
        .bind(3i32)
        .bind(true) // active
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO rules \
             (id, name, description, weight, mode, pipeline, allow, \
              rate_limit_profile_id, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(102i32)
        .bind("test-jail-5")
        .bind("")
        .bind(100i32)
        .bind("enforce")
        .bind("jail")
        .bind(false)
        .bind(5i32)
        .bind(true) // active
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // AND rate_limiter_state has data for both profile_ids
        let epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 15 timestamps for profile_id=3, IP 1.2.3.4
        let ts_3: Vec<i64> = (0..15).map(|i| epoch - i * 3_000).collect();
        sqlx::query(
            "INSERT INTO rate_limiter_state \
             (profile_id, ip_address, timestamps, head, count, capacity, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(3i32)
        .bind("1.2.3.4")
        .bind(serde_json::to_string(&ts_3).unwrap())
        .bind(0i32)
        .bind(15i32)
        .bind(20i32)
        .bind(epoch.to_string())
        .execute(&pool)
        .await
        .unwrap();

        // 8 timestamps for profile_id=5, IP 5.6.7.8
        let ts_5: Vec<i64> = (0..8).map(|i| epoch - i * 5_000).collect();
        sqlx::query(
            "INSERT INTO rate_limiter_state \
             (profile_id, ip_address, timestamps, head, count, capacity, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(5i32)
        .bind("5.6.7.8")
        .bind(serde_json::to_string(&ts_5).unwrap())
        .bind(0i32)
        .bind(8i32)
        .bind(30i32)
        .bind(epoch.to_string())
        .execute(&pool)
        .await
        .unwrap();

        // 5 timestamps for profile_id=5, IP 9.9.9.9 (second IP under same profile)
        let ts_5b: Vec<i64> = (0..5).map(|i| epoch - i * 6_000).collect();
        sqlx::query(
            "INSERT INTO rate_limiter_state \
             (profile_id, ip_address, timestamps, head, count, capacity, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(5i32)
        .bind("9.9.9.9")
        .bind(serde_json::to_string(&ts_5b).unwrap())
        .bind(0i32)
        .bind(5i32)
        .bind(30i32)
        .bind(epoch.to_string())
        .execute(&pool)
        .await
        .unwrap();

        // WHEN the startup loading logic executes (simulating main.rs)
        let rules = CacheRule::read_all_active(&pool).await.unwrap_or_default();
        let active_profile_ids: Vec<i32> = rules
            .iter()
            .filter(|r| r.rule.pipeline == "jail" && r.rule.active)
            .filter_map(|r| r.rule.rate_limit_profile_id)
            .collect();

        let mut rate_limiter_map: HashMap<i32, RateLimiter> = HashMap::new();
        for profile_id in &active_profile_ids {
            // Read profile from DB (same as main.rs)
            let profile = RateLimitProfile::read(&pool, *profile_id)
                .await
                .expect("Profile should exist");

            let rl = RateLimiter::load(
                &pool,
                *profile_id,
                profile.max_retry as u32,
                profile.find_time_seconds,
            )
            .await
            .unwrap_or_else(|_| {
                RateLimiter::new(profile.max_retry as u32, profile.find_time_seconds)
            });

            rate_limiter_map.insert(*profile_id, rl);
        }

        // THEN the HashMap has entries for profile_id=3 and profile_id=5
        assert!(
            rate_limiter_map.contains_key(&3),
            "HashMap should have entry for profile_id=3"
        );
        assert!(
            rate_limiter_map.contains_key(&5),
            "HashMap should have entry for profile_id=5"
        );

        // AND profile_id=3 has 15 timestamps for 1.2.3.4
        let rl3 = rate_limiter_map.get(&3).unwrap();
        let ip3: IpAddr = "1.2.3.4".parse().unwrap();
        let buffer3 = rl3
            .ip_buffers
            .get(&ip3)
            .expect("profile_id=3 should have IP 1.2.3.4");
        assert_eq!(
            buffer3.len(),
            15,
            "profile_id=3 should have 15 timestamps for 1.2.3.4"
        );

        // AND profile_id=5 has entries for both 5.6.7.8 (8 ts) and 9.9.9.9 (5 ts)
        let rl5 = rate_limiter_map.get(&5).unwrap();
        let ip5a: IpAddr = "5.6.7.8".parse().unwrap();
        let buffer5a = rl5
            .ip_buffers
            .get(&ip5a)
            .expect("profile_id=5 should have IP 5.6.7.8");
        assert_eq!(
            buffer5a.len(),
            8,
            "profile_id=5 should have 8 timestamps for 5.6.7.8"
        );

        let ip5b: IpAddr = "9.9.9.9".parse().unwrap();
        let buffer5b = rl5
            .ip_buffers
            .get(&ip5b)
            .expect("profile_id=5 should have IP 9.9.9.9");
        assert_eq!(
            buffer5b.len(),
            5,
            "profile_id=5 should have 5 timestamps for 9.9.9.9"
        );

        // AND total tracked IPs match
        assert_eq!(rl3.len(), 1, "profile_id=3 should track 1 IP");
        assert_eq!(rl5.len(), 2, "profile_id=5 should track 2 IPs");
    }
}

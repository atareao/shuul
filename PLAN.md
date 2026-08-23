# Shuul Rate Limiting + Bans Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add fail2ban-style rate limiting and temporary IP banning directly into shuul's HTTP pipeline, without any firewall backend (no nftables/ipset).

**Architecture:** Each rule gains rate limiting fields. When a request matches a rule with `rate_limit_enabled=true`, it counts as an "attempt" for that IP. Exceeding `max_retry` within `find_time` triggers a temporary HTTP-level ban (shuul returns 403). Bans escalate on repeat offenses and decay after a quiet period.

**Tech Stack:** Rust (Axum 0.8, sqlx/PostgreSQL, regex, chrono, tokio), TypeScript React (Ant Design, CustomTable)

## Global Constraints

- Edition 2024 Rust
- All new SQL columns must have defaults so existing rules work unchanged
- No firewall backend (no nftables/ipset/iptables)
- Bans enforced at HTTP level only
- Templates are static Rust arrays, not database entities
- Frontend uses existing CustomTable + CustomDialog patterns
- All durations stored as i64 seconds in DB

---

### Task 1: Database Migration — Rate limiting fields + bans table

**Files:**
- Create: `backend/migrations/20260821000001_rate_limiting.up.sql`
- Create: `backend/migrations/20260821000001_rate_limiting.down.sql`

**Interfaces:**
- Consumes: existing `rules` table schema
- Produces: `rules` table with new columns, `bans` table

- [ ] **Step 1: Write the up migration**

```sql
-- Add rate limiting columns to rules table
ALTER TABLE rules
    ADD COLUMN IF NOT EXISTS rate_limit_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS max_retry INT NOT NULL DEFAULT 5,
    ADD COLUMN IF NOT EXISTS find_time_seconds INT NOT NULL DEFAULT 600,
    ADD COLUMN IF NOT EXISTS ban_time_seconds INT NOT NULL DEFAULT 3600,
    ADD COLUMN IF NOT EXISTS bantime_increment BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS bantime_multipliers INT[] NOT NULL DEFAULT '{1,2,4,8}',
    ADD COLUMN IF NOT EXISTS bantime_maxtime_seconds INT NOT NULL DEFAULT 604800,
    ADD COLUMN IF NOT EXISTS ban_count_decay_days INT NOT NULL DEFAULT 30,
    ADD COLUMN IF NOT EXISTS ignoreip TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS webhook TEXT;

-- Create bans table
CREATE TABLE IF NOT EXISTS bans (
    id SERIAL PRIMARY KEY,
    ip_address TEXT NOT NULL,
    rule_id INT REFERENCES rules(id) ON DELETE SET NULL,
    jail_name TEXT NOT NULL,
    banned_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ban_duration_seconds INT NOT NULL,
    escalation_level INT NOT NULL DEFAULT 0,
    reason TEXT,
    expired BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bans_ip_address ON bans(ip_address);
CREATE INDEX IF NOT EXISTS idx_bans_expired ON bans(expired);
```

- [ ] **Step 2: Write the down migration**

```sql
DROP TABLE IF EXISTS bans;

ALTER TABLE rules
    DROP COLUMN IF EXISTS webhook,
    DROP COLUMN IF EXISTS ignoreip,
    DROP COLUMN IF EXISTS ban_count_decay_days,
    DROP COLUMN IF EXISTS bantime_maxtime_seconds,
    DROP COLUMN IF EXISTS bantime_multipliers,
    DROP COLUMN IF EXISTS bantime_increment,
    DROP COLUMN IF EXISTS ban_time_seconds,
    DROP COLUMN IF EXISTS find_time_seconds,
    DROP COLUMN IF EXISTS max_retry,
    DROP COLUMN IF EXISTS rate_limit_enabled;
```

- [ ] **Step 3: Verify migration compiles**

Run: `cd backend && cargo check 2>&1 | head -20`
Expected: No errors (migrations are not compiled, just verifying project structure)

- [ ] **Step 4: Commit**

```bash
git add backend/migrations/20260821000001_rate_limiting.up.sql backend/migrations/20260821000001_rate_limiting.down.sql
git commit -m "feat: add rate limiting columns to rules and create bans table"
```

---

### Task 2: Backend — CircularTimestamps + RateLimiter module

**Files:**
- Create: `backend/src/rate_limiter.rs`
- Modify: `backend/src/models/mod.rs` (add `pub mod rate_limiter;`)

**Interfaces:**
- Consumes: nothing (pure Rust)
- Produces: `CircularTimestamps`, `RateLimiter` structs

- [ ] **Step 1: Create rate_limiter.rs**

```rust
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
        Self {
            timestamps: vec![Instant::now(); capacity],
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
```

- [ ] **Step 2: Register module in mod.rs**

Edit `backend/src/models/mod.rs` — add after line 9 (`mod ipdata;`):

```rust
mod rate_limiter;
```

And add to the `pub use` block:

```rust
pub use rate_limiter::{CircularTimestamps, RateLimiter};
```

- [ ] **Step 3: Run tests**

Run: `cd backend && cargo test rate_limiter -- --nocapture`
Expected: 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add backend/src/models/rate_limiter.rs backend/src/models/mod.rs
git commit -m "feat: add CircularTimestamps and RateLimiter modules"
```

---

### Task 3: Backend — BanManager module

**Files:**
- Create: `backend/src/models/ban_manager.rs`
- Modify: `backend/src/models/mod.rs` (register module)

**Interfaces:**
- Consumes: `IpAddr`, `chrono::DateTime<Utc>`
- Produces: `BanInfo`, `BanManager` structs

- [ ] **Step 1: Create ban_manager.rs**

```rust
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
    pub fn ban(&mut self, ip: IpAddr, rule_id: Option<i32>, reason: String) -> &BanInfo {
        let escalation_level = self.get_escalation_level(&ip);
        let duration = self.calculate_ban_duration(escalation_level);

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
        bm.ban(ip, None, "test ban".to_string());
        assert!(bm.is_banned(&ip).is_some());
    }

    #[test]
    fn test_unban() {
        let mut bm = BanManager::new(3600, false, vec![1, 2, 4], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        bm.ban(ip, Some(1), "test".to_string());
        assert!(bm.unban(&ip, Some(1)));
        assert!(bm.is_banned(&ip).is_none());
    }

    #[test]
    fn test_escalation() {
        let mut bm = BanManager::new(3600, true, vec![1, 2, 4, 8], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        // First offense: 3600s (multiplier 1)
        let ban1 = bm.ban(ip, None, "1st".to_string());
        assert_eq!(ban1.ban_duration_seconds, 3600);

        // Second offense: 7200s (multiplier 2)
        let ban2 = bm.ban(ip, None, "2nd".to_string());
        assert_eq!(ban2.ban_duration_seconds, 7200);

        // Third offense: 14400s (multiplier 4)
        let ban3 = bm.ban(ip, None, "3rd".to_string());
        assert_eq!(ban3.ban_duration_seconds, 14400);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut bm = BanManager::new(1, false, vec![1], 86400, 30);
        let ip: IpAddr = "1.2.3.4".parse().unwrap();

        bm.ban(ip, None, "short ban".to_string());
        assert_eq!(bm.active_count(), 1);

        std::thread::sleep(Duration::from_millis(1100));
        bm.cleanup_expired();
        assert_eq!(bm.active_count(), 0);
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

Edit `backend/src/models/mod.rs` — add after `mod rate_limiter;`:

```rust
mod ban_manager;
```

And add to `pub use`:

```rust
pub use ban_manager::{BanInfo, BanManager};
```

- [ ] **Step 3: Run tests**

Run: `cd backend && cargo test ban_manager -- --nocapture`
Expected: 4 tests pass

- [ ] **Step 4: Commit**

```bash
git add backend/src/models/ban_manager.rs backend/src/models/mod.rs
git commit -m "feat: add BanManager with escalation and decay"
```

---

### Task 4: Backend — Extend Rule models with rate limiting fields

**Files:**
- Modify: `backend/src/models/rule.rs`

**Interfaces:**
- Consumes: existing Rule, NewRule, UpdateRule, ReadRuleParams, CacheRule
- Produces: extended versions with rate limiting fields

- [ ] **Step 1: Add rate limiting fields to Rule struct**

Add after `country_code` field:

```rust
    pub rate_limit_enabled: bool,
    pub max_retry: i32,
    pub find_time_seconds: i64,
    pub ban_time_seconds: i64,
    pub bantime_increment: bool,
    pub bantime_multipliers: Vec<i32>,
    pub bantime_maxtime_seconds: i64,
    pub ban_count_decay_days: i32,
    pub ignoreip: Vec<String>,
    pub webhook: Option<String>,
```

- [ ] **Step 2: Add fields to from_row()**

Add after `country_code: row.get("country_code"),`:

```rust
            rate_limit_enabled: row.get("rate_limit_enabled"),
            max_retry: row.get("max_retry"),
            find_time_seconds: row.get("find_time_seconds"),
            ban_time_seconds: row.get("ban_time_seconds"),
            bantime_increment: row.get("bantime_increment"),
            bantime_multipliers: row.get("bantime_multipliers"),
            bantime_maxtime_seconds: row.get("bantime_maxtime_seconds"),
            ban_count_decay_days: row.get("ban_count_decay_days"),
            ignoreip: row.get("ignoreip"),
            webhook: row.get("webhook"),
```

- [ ] **Step 3: Add fields to NewRule struct**

Add after `country_code`:

```rust
    pub rate_limit_enabled: Option<bool>,
    pub max_retry: Option<i32>,
    pub find_time_seconds: Option<i64>,
    pub ban_time_seconds: Option<i64>,
    pub bantime_increment: Option<bool>,
    pub bantime_multipliers: Option<Vec<i32>>,
    pub bantime_maxtime_seconds: Option<i64>,
    pub ban_count_decay_days: Option<i32>,
    pub ignoreip: Option<Vec<String>>,
    pub webhook: Option<String>,
```

- [ ] **Step 4: Add fields to UpdateRule struct**

Same fields as NewRule (all Option).

- [ ] **Step 5: Add fields to ReadRuleParams struct**

Same fields as NewRule (all Option).

- [ ] **Step 6: Update Rule::create() SQL**

Replace the INSERT with:

```rust
    pub async fn create(pool: &PgPool, rule: NewRule) -> Result<Self, Error> {
        let sql = "INSERT INTO rules (weight, allow, store,
            ip_address, protocol, fqdn, path, query, city_name, country_name,
            country_code, active, created_at, updated_at,
            rate_limit_enabled, max_retry, find_time_seconds, ban_time_seconds,
            bantime_increment, bantime_multipliers, bantime_maxtime_seconds,
            ban_count_decay_days, ignoreip, webhook) VALUES ($1, $2, $3,
            $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
            $15, $16, $17, $18, $19, $20, $21, $22, $23, $24) RETURNING *";
        let now = Utc::now();
        query(sql)
            .bind(rule.weight)
            .bind(rule.allow)
            .bind(rule.store)
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.active)
            .bind(now)
            .bind(now)
            .bind(rule.rate_limit_enabled.unwrap_or(false))
            .bind(rule.max_retry.unwrap_or(5))
            .bind(rule.find_time_seconds.unwrap_or(600))
            .bind(rule.ban_time_seconds.unwrap_or(3600))
            .bind(rule.bantime_increment.unwrap_or(false))
            .bind(rule.bantime_multipliers.unwrap_or(vec![1, 2, 4, 8]))
            .bind(rule.bantime_maxtime_seconds.unwrap_or(604800))
            .bind(rule.ban_count_decay_days.unwrap_or(30))
            .bind(rule.ignoreip.unwrap_or_default())
            .bind(rule.webhook)
            .map(Self::from_row)
            .fetch_one(pool)
            .await
    }
```

- [ ] **Step 7: Update Rule::update() SQL**

Replace the UPDATE with similar changes (bind all new fields).

- [ ] **Step 8: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compilation succeeds

- [ ] **Step 9: Commit**

```bash
git add backend/src/models/rule.rs
git commit -m "feat: extend Rule models with rate limiting fields"
```

---

### Task 5: Backend — Extend shuul pipeline with ban check + rate limiter

**Files:**
- Modify: `backend/src/http/shuul.rs`
- Modify: `backend/src/models/mod.rs` (AppState)
- Modify: `backend/src/main.rs`

**Interfaces:**
- Consumes: `AppState` with `ban_manager` and `rate_limiter`
- Produces: Extended pipeline: ban check → rate limit → rule matching

- [ ] **Step 1: Add ban_manager and rate_limiter to AppState**

Edit `backend/src/models/mod.rs` — add to AppState:

```rust
    pub ban_manager: Mutex<BanManager>,
    pub rate_limiter: Mutex<HashMap<i32, RateLimiter>>, // rule_id → RateLimiter
```

- [ ] **Step 2: Initialize in main.rs**

After the existing `let rules = ...` and `let cache = ...` lines, add:

```rust
    let ban_manager = Mutex::new(BanManager::new(
        3600,    // default_ban_duration (1h)
        false,   // bantime_increment (per-rule config)
        vec![1, 2, 4, 8],
        604800,  // bantime_maxtime (1w)
        30,      // ban_count_decay_days
    ));
    let rate_limiter: Mutex<HashMap<i32, RateLimiter>> = Mutex::new(HashMap::new());
```

And add to the AppState construction:

```rust
            ban_manager,
            rate_limiter,
```

- [ ] **Step 3: Rewrite shuul.rs with extended pipeline**

Replace the entire file:

```rust
//! # Endpoint principal de captura
//!
//! Pipeline extendido:
//! 1. Extraer request de headers
//! 2. Check: ¿IP baneada? → 403
//! 3. Rate limiter: ¿IP excede threshold? → Ban + 403
//! 4. Reglas estáticas (allow/deny)
//! 5. Persistir si la regla lo indica

use crate::models::{AppState, EmptyResponse, NewRequest, Request, BanManager, RateLimiter};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use std::mem;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, error};

pub fn shuul_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::any(shuul))
}

/// Main entry point for the shuul service.
pub async fn shuul(
    State(app_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let mut request = NewRequest::from_request(&headers, &app_state.maxmind_db);
    debug!("Captured request: {:?}", request);

    // ── Step 1: Check if IP is actively banned ──
    if let Some(ip_str) = &request.ip_address {
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            if let Ok(ban_manager) = app_state.ban_manager.lock() {
                if let Some(ban) = ban_manager.is_banned(&ip) {
                    debug!("IP {} is banned (reason: {})", ip, ban.reason);
                    return EmptyResponse::create(
                        StatusCode::FORBIDDEN,
                        &format!("Banned: {}", ban.reason),
                    );
                }
            }
        }
    }

    // ── Step 2: Match against cached rules ──
    let mut allow = true;
    let mut save = true;
    let mut matched_rule_id: Option<i32> = None;

    if let Ok(rules) = app_state.rules.lock() {
        for cache_rule in rules.iter() {
            if cache_rule.matches(&request) {
                request.rule_id = Some(cache_rule.rule.id);
                debug!("Selected rule: {:?}", cache_rule.rule);
                save = cache_rule.rule.store;
                allow = cache_rule.rule.allow;
                matched_rule_id = Some(cache_rule.rule.id);

                // ── Step 3: Rate limiter check ──
                if cache_rule.rule.rate_limit_enabled {
                    if let Some(ip_str) = &request.ip_address {
                        if let Ok(ip) = ip_str.parse::<IpAddr>() {
                            let should_ban = {
                                if let Ok(mut rate_limiters) = app_state.rate_limiter.lock() {
                                    let rl = rate_limiters
                                        .entry(cache_rule.rule.id)
                                        .or_insert_with(|| RateLimiter::new(
                                            cache_rule.rule.max_retry as u32,
                                            cache_rule.rule.find_time_seconds,
                                        ));
                                    rl.record(ip)
                                } else {
                                    false
                                }
                            };

                            if should_ban {
                                debug!(
                                    "IP {} exceeded rate limit for rule {}, banning",
                                    ip, cache_rule.rule.id
                                );
                                if let Ok(mut ban_manager) = app_state.ban_manager.lock() {
                                    ban_manager.ban(
                                        ip,
                                        Some(cache_rule.rule.id),
                                        format!(
                                            "Rate limit: {} requests in {}s",
                                            cache_rule.rule.max_retry,
                                            cache_rule.rule.find_time_seconds
                                        ),
                                    );
                                }
                                allow = false;
                            }
                        }
                    }
                }

                break;
            }
        }
    }

    if request.rule_id.is_none() {
        debug!("No matching rule found for request: {:?}", &request);
    }

    // ── Step 4: Persist the request if the rule says so ──
    if save {
        debug!("Saving request as per rule configuration");
        save_on_cache_or_db(&app_state, request).await;
    } else {
        debug!("Not saving request as per rule configuration");
    }

    if allow {
        EmptyResponse::create(StatusCode::OK, "Ok")
    } else {
        EmptyResponse::create(StatusCode::FORBIDDEN, "Ko")
    }
}

/// Saves a request either to the in-memory cache or directly to the database.
async fn save_on_cache_or_db(app_state: &AppState, request: NewRequest) {
    if app_state.cache_enabled {
        debug!("Cache is enabled, saving request to cache");
        let mut requests_to_save: Option<Vec<NewRequest>> = None;
        {
            if let Ok(mut cache_guard) = app_state.cache.lock() {
                cache_guard.push(request);
                debug!("Request saved to cache. Cache size: {}", cache_guard.len());
                if cache_guard.len() >= app_state.cache_size {
                    requests_to_save = Some(mem::take(&mut *cache_guard));
                    debug!("Cache size reached limit, preparing to bulk save to database");
                }
            }
        }
        if let Some(requests) = requests_to_save {
            debug!(
                "Caching limit reached, saving {} requests to database",
                requests.len()
            );
            match Request::create_bulk(&app_state.pool, requests).await {
                Ok(data) => debug!("Saved {} requests from cache to database", data.len()),
                Err(e) => error!("Error saving requests from cache to database: {:?}", e),
            }
        }
    } else {
        match Request::create(&app_state.pool, request).await {
            Ok(req) => debug!("Saved request to database: {:?}", req),
            Err(e) => error!("Error saving request to database: {:?}", e),
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compilation succeeds

- [ ] **Step 5: Commit**

```bash
git add backend/src/http/shuul.rs backend/src/models/mod.rs backend/src/main.rs
git commit -m "feat: extend shuul pipeline with ban check and rate limiter"
```

---

### Task 6: Backend — Ban CRUD endpoints

**Files:**
- Create: `backend/src/http/ban.rs`
- Modify: `backend/src/http/mod.rs`
- Modify: `backend/src/main.rs`

**Interfaces:**
- Consumes: `AppState` with `ban_manager`
- Produces: `GET /api/v1/bans`, `POST /api/v1/bans`, `DELETE /api/v1/bans`, `GET /api/v1/bans/info`

- [ ] **Step 1: Create ban.rs**

```rust
//! # Endpoints de bans
//!
//! CRUD para bans activos: listar, banear manualmente, desbanear.

use crate::models::error::AppError;
use crate::models::{ApiResponse, AppState, Data};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::debug;

pub fn ban_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", routing::get(list_handler))
        .route("/", routing::post(ban_handler))
        .route("/", routing::delete(unban_handler))
        .route("/info", routing::get(info_handler))
}

#[derive(Debug, Serialize)]
pub struct BanResponse {
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub reason: String,
    pub ban_duration_seconds: i64,
    pub escalation_level: u32,
    pub time_remaining_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub ip_address: String,
    pub rule_id: Option<i32>,
    pub reason: Option<String>,
    pub ban_duration_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UnbanParams {
    pub ip_address: String,
    pub rule_id: Option<i32>,
}

/// GET /api/v1/bans — List all active bans.
pub async fn list_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let bans = {
        let ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager
            .active_bans()
            .into_iter()
            .map(|(ip, ban)| BanResponse {
                ip_address: ip.to_string(),
                rule_id: ban.rule_id,
                reason: ban.reason.clone(),
                ban_duration_seconds: ban.ban_duration_seconds,
                escalation_level: ban.escalation_level,
                time_remaining_seconds: ban.time_remaining().as_secs(),
            })
            .collect::<Vec<_>>()
    };
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Active bans",
        Data::Some(serde_json::to_value(bans)?),
    ))
}

/// POST /api/v1/bans — Manually ban an IP.
pub async fn ban_handler(
    State(app_state): State<Arc<AppState>>,
    Json(params): Json<BanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ip: IpAddr = params
        .ip_address
        .parse()
        .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;

    let duration = params.ban_duration_seconds.unwrap_or(3600);
    let reason = params.reason.unwrap_or_else(|| "Manual ban".to_string());

    let ban_info = {
        let mut ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager.ban(ip, params.rule_id, reason).clone()
    };

    Ok(ApiResponse::new(
        StatusCode::CREATED,
        "IP banned",
        Data::Some(serde_json::to_value(BanResponse {
            ip_address: ip.to_string(),
            rule_id: params.rule_id,
            reason: ban_info.reason,
            ban_duration_seconds: ban_info.ban_duration_seconds,
            escalation_level: ban_info.escalation_level,
            time_remaining_seconds: ban_info.time_remaining().as_secs(),
        })?),
    ))
}

/// DELETE /api/v1/bans — Unban an IP.
pub async fn unban_handler(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<UnbanParams>,
) -> Result<impl IntoResponse, AppError> {
    let ip: IpAddr = params
        .ip_address
        .parse()
        .map_err(|_| AppError::InvalidInput("Invalid IP address".to_string()))?;

    let removed = {
        let mut ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager.unban(&ip, params.rule_id)
    };

    if removed {
        Ok(ApiResponse::new(
            StatusCode::OK,
            "IP unbanned",
            Data::None,
        ))
    } else {
        Ok(ApiResponse::new(
            StatusCode::NOT_FOUND,
            "IP not found or not banned",
            Data::None,
        ))
    }
}

/// GET /api/v1/bans/info — Count of active bans.
pub async fn info_handler(
    State(app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let count = {
        let ban_manager = app_state
            .ban_manager
            .lock()
            .map_err(|_| AppError::CachePoisoned)?;
        ban_manager.active_count()
    };
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Active bans count",
        Data::Some(serde_json::to_value(count)?),
    ))
}
```

- [ ] **Step 2: Register in mod.rs**

Edit `backend/src/http/mod.rs` — add:

```rust
mod ban;
```

And in the `pub use` block:

```rust
pub use ban::ban_router;
```

- [ ] **Step 3: Register in main.rs**

After `use http::{...}` add `ban_router,`. Then add route:

```rust
        .nest("/bans", ban_router())
```

- [ ] **Step 4: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compilation succeeds

- [ ] **Step 5: Commit**

```bash
git add backend/src/http/ban.rs backend/src/http/mod.rs backend/src/main.rs
git commit -m "feat: add ban CRUD endpoints"
```

---

### Task 7: Backend — Templates endpoint

**Files:**
- Create: `backend/src/templates.rs`
- Modify: `backend/src/http/mod.rs`
- Modify: `backend/src/main.rs`

**Interfaces:**
- Consumes: nothing (static data)
- Produces: `GET /api/v1/templates` returning categorized rule templates

- [ ] **Step 1: Create templates.rs**

```rust
//! # Plantillas de reglas preconfiguradas
//!
//! Catálogo de reglas recomendadas para servicios populares.
//! Los usuarios pueden aplicar estas plantillas desde el frontend.

use serde::Serialize;

/// A preconfigured rule template.
#[derive(Debug, Serialize, Clone)]
pub struct RuleTemplate {
    pub name: String,
    pub description: String,
    pub category: String,
    pub severity: String, // "🔥 Crítico", "🔴 Alto", "🟡 Medio", "🟢 Bajo"
    pub path: Option<String>,
    pub query: Option<String>,
    pub country_code: Option<String>,
    pub allow: bool,
    pub store: bool,
    pub rate_limit_enabled: bool,
    pub max_retry: Option<i32>,
    pub find_time_seconds: Option<i64>,
    pub ban_time_seconds: Option<i64>,
    pub bantime_increment: bool,
    pub bantime_multipliers: Vec<i32>,
    pub bantime_maxtime_seconds: i64,
    pub ban_count_decay_days: i32,
}

/// All available templates, grouped by category.
pub fn all_templates() -> Vec<RuleTemplate> {
    vec![
        // ── WordPress ──
        RuleTemplate {
            name: "WordPress - wp-login".into(),
            description: "Protege el login de WordPress contra fuerza bruta".into(),
            category: "wordpress".into(),
            severity: "🔥 Crítico".into(),
            path: Some(r"^/wp-login\.php".into()),
            query: None,
            country_code: None,
            allow: true,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(5),
            find_time_seconds: Some(600),
            ban_time_seconds: Some(3600),
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604800,
            ban_count_decay_days: 30,
        },
        RuleTemplate {
            name: "WordPress - xmlrpc".into(),
            description: "xmlrpc.php es vector clásico de fuerza bruta y DDoS".into(),
            category: "wordpress".into(),
            severity: "🔥 Crítico".into(),
            path: Some(r"^/xmlrpc\.php".into()),
            query: None,
            country_code: None,
            allow: false,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(3),
            find_time_seconds: Some(600),
            ban_time_seconds: Some(86400),
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604800,
            ban_count_decay_days: 30,
        },
        RuleTemplate {
            name: "WordPress - wp-admin".into(),
            description: "Protege el panel de administración de WordPress".into(),
            category: "wordpress".into(),
            severity: "🔴 Alto".into(),
            path: Some(r"^/wp-admin".into()),
            query: None,
            country_code: None,
            allow: true,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(10),
            find_time_seconds: Some(1800),
            ban_time_seconds: Some(7200),
            bantime_increment: false,
            bantime_multipliers: vec![1],
            bantime_maxtime_seconds: 7200,
            ban_count_decay_days: 30,
        },
        // ── phpMyAdmin ──
        RuleTemplate {
            name: "phpMyAdmin - login".into(),
            description: "Protege phpMyAdmin contra accesos no autorizados".into(),
            category: "paneles".into(),
            severity: "🔥 Crítico".into(),
            path: Some(r"^/(phpmyadmin|pma|mysql|phpPgAdmin)".into()),
            query: None,
            country_code: None,
            allow: true,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(3),
            find_time_seconds: Some(600),
            ban_time_seconds: Some(86400),
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604800,
            ban_count_decay_days: 30,
        },
        // ── API Auth ──
        RuleTemplate {
            name: "API - login endpoint".into(),
            description: "Protege endpoints de autenticación contra fuerza bruta".into(),
            category: "api".into(),
            severity: "🔴 Alto".into(),
            path: Some(r"^/(api|auth)/(login|signin|token)".into()),
            query: None,
            country_code: None,
            allow: true,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(5),
            find_time_seconds: Some(600),
            ban_time_seconds: Some(3600),
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604800,
            ban_count_decay_days: 30,
        },
        RuleTemplate {
            name: "API - register endpoint".into(),
            description: "Protege registros contra bots".into(),
            category: "api".into(),
            severity: "🔴 Alto".into(),
            path: Some(r"^/(api|auth)/(register|signup)".into()),
            query: None,
            country_code: None,
            allow: true,
            store: true,
            rate_limit_enabled: true,
            max_retry: Some(3),
            find_time_seconds: Some(3600),
            ban_time_seconds: Some(86400),
            bantime_increment: true,
            bantime_multipliers: vec![1, 2, 4, 8],
            bantime_maxtime_seconds: 604800,
            ban_count_decay_days: 30,
        },
        // ── Seguridad general ──
        RuleTemplate {
            name: "Archivos sensibles".into(),
            description: "Bloquea acceso a archivos de configuración y backups".into(),
            category: "seguridad".into(),
            severity: "🔴 Alto".into(),
            path: Some(r"\.(env|bak|sql|dump|config|yml|yaml|json|log|ini)$".into()),
            query: None,
            country_code: None,
            allow: false,
            store: true,
            rate_limit_enabled: false,
            max_retry: None,
            find_time_seconds: None,
            ban_time_seconds: None,
            bantime_increment: false,
            bantime_multipliers: vec![1],
            bantime_maxtime_seconds: 3600,
            ban_count_decay_days: 30,
        },
        RuleTemplate {
            name: "Directorios de sistema".into(),
            description: "Bloquea acceso a directorios internos del proyecto".into(),
            category: "seguridad".into(),
            severity: "🔥 Crítico".into(),
            path: Some(r"^/(vendor|node_modules|storage|cache|logs|tmp|\.git|\.env)".into()),
            query: None,
            country_code: None,
            allow: false,
            store: true,
            rate_limit_enabled: false,
            max_retry: None,
            find_time_seconds: None,
            ban_time_seconds: None,
            bantime_increment: false,
            bantime_multipliers: vec![1],
            bantime_maxtime_seconds: 3600,
            ban_count_decay_days: 30,
        },
        // ── Geo ──
        RuleTemplate {
            name: "Bloquear países sin negocio".into(),
            description: "Deniega tráfico de países donde no operas".into(),
            category: "geo".into(),
            severity: "🟢 Bajo".into(),
            path: None,
            query: None,
            country_code: Some(r"^(RU|CN|KP|IR)$".into()),
            allow: false,
            store: true,
            rate_limit_enabled: false,
            max_retry: None,
            find_time_seconds: None,
            ban_time_seconds: None,
            bantime_increment: false,
            bantime_multipliers: vec![1],
            bantime_maxtime_seconds: 3600,
            ban_count_decay_days: 30,
        },
    ]
}
```

- [ ] **Step 2: Create templates endpoint**

Create `backend/src/http/template.rs`:

```rust
//! # Endpoint de plantillas
//!
//! Devuelve el catálogo de reglas preconfiguradas.

use crate::models::{ApiResponse, AppState, Data};
use crate::templates::all_templates;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing};
use std::sync::Arc;

pub fn template_router() -> Router<Arc<AppState>> {
    Router::new().route("/", routing::get(list_templates))
}

/// GET /api/v1/templates — List all rule templates.
pub async fn list_templates(
    State(_app_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, crate::models::error::AppError> {
    let templates = all_templates();
    Ok(ApiResponse::new(
        StatusCode::OK,
        "Rule templates",
        Data::Some(serde_json::to_value(templates)?),
    ))
}
```

- [ ] **Step 3: Register in mod.rs**

Edit `backend/src/http/mod.rs` — add:

```rust
mod template;
pub use template::template_router;
```

- [ ] **Step 4: Register in main.rs**

Add `template_router` to imports and nest at `/templates`.

- [ ] **Step 5: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compilation succeeds

- [ ] **Step 6: Commit**

```bash
git add backend/src/templates.rs backend/src/http/template.rs backend/src/http/mod.rs backend/src/main.rs
git commit -m "feat: add rule templates catalog and endpoint"
```

---

### Task 8: Frontend — Extend Rule model

**Files:**
- Modify: `frontend/src/models/rule.ts`

- [ ] **Step 1: Add rate limiting fields to Rule interface**

```typescript
export default interface Rule {
    id: number;
    weight?: number;
    allow?: boolean;
    store?: boolean;
    ip_address?: string;
    protocol?: string;
    fqdn?: string;
    path?: string;
    query?: string;
    city_name?: string;
    country_name?: string;
    country_code?: string;
    active?: number;
    created_at?: Date;
    updated_at?: Date;
    // Rate limiting fields
    rate_limit_enabled?: boolean;
    max_retry?: number;
    find_time_seconds?: number;
    ban_time_seconds?: number;
    bantime_increment?: boolean;
    bantime_multipliers?: number[];
    bantime_maxtime_seconds?: number;
    ban_count_decay_days?: number;
    ignoreip?: string[];
    webhook?: string;
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/models/rule.ts
git commit -m "feat: extend frontend Rule model with rate limiting fields"
```

---

### Task 9: Frontend — Extend Rules page with rate limiting fields

**Files:**
- Modify: `frontend/src/pages/admin/rules_page.tsx`

- [ ] **Step 1: Add rate limiting fields to FIELDS array**

Add after the `country_code` field:

```typescript
    { key: 'rate_limit_enabled', label: 'Rate Limit', type: 'boolean', value: false, width: 100, visible: true },
```

- [ ] **Step 2: Add Ban model**

Create `frontend/src/models/ban.ts`:

```typescript
export default interface Ban {
    id: number;
    ip_address: string;
    rule_id?: number;
    jail_name: string;
    banned_at: string;
    ban_duration_seconds: number;
    escalation_level: number;
    reason?: string;
    expired: boolean;
    created_at: string;
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/pages/admin/rules_page.tsx frontend/src/models/ban.ts
git commit -m "feat: add rate limit field to rules page and Ban model"
```

---

### Task 10: Frontend — Bans page

**Files:**
- Create: `frontend/src/pages/admin/bans_page.tsx`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/layouts/admin_layout.tsx`

- [ ] **Step 1: Create bans_page.tsx**

```typescript
import React from "react";
import { useNavigate } from 'react-router';
import { useTranslation } from "react-i18next";
import { Button, Space, Typography } from 'antd';
import { DeleteFilled, PlusOutlined } from '@ant-design/icons';
import type Ban from "@/models/ban";
import CustomTable from '@/components/custom_table';
import type { FieldDefinition } from '@/common/types';

const { Text } = Typography;
const TITLE = "Active Bans";
const ENDPOINT = "bans";

const FIELDS: FieldDefinition<Ban>[] = [
    { key: 'id', label: 'Id', type: 'number', value: 0, editable: false, fixed: 'left', width: 80 },
    { key: 'ip_address', label: 'IP Address', type: 'string', value: "", width: 150, visible: true },
    { key: 'reason', label: 'Reason', type: 'string', value: "", width: 200, visible: true },
    { key: 'ban_duration_seconds', label: 'Duration (s)', type: 'number', value: 0, width: 120, visible: true },
    { key: 'escalation_level', label: 'Level', type: 'number', value: 0, width: 80, visible: true },
];

export class InnerPage extends React.Component<{ navigate: any; t: any }, {}> {
    private renderHeaderAction = (onCreate: () => void) => {
        return (
            <Button type="primary" onClick={onCreate} icon={<PlusOutlined />}>
                {this.props.t("Ban IP")}
            </Button>
        );
    };

    private renderActionColumn = (item: Ban, _onEdit: any, onDelete: (item: Ban) => void) => {
        return (
            <Space size="middle">
                <Button onClick={() => onDelete(item)} title={this.props.t('Unban')} danger>
                    <DeleteFilled />
                </Button>
            </Space>
        );
    };

    render = () => {
        return (
            <CustomTable<Ban>
                title={TITLE}
                endpoint={ENDPOINT}
                fields={FIELDS}
                t={this.props.t}
                hasActions={true}
                renderHeaderAction={this.renderHeaderAction}
                renderActionColumn={this.renderActionColumn}
            />
        );
    }
}

export default function Page() {
    const navigate = useNavigate();
    const { t } = useTranslation();
    return <InnerPage navigate={navigate} t={t} />;
}
```

- [ ] **Step 2: Add route in App.tsx**

Add import: `const BansPage = lazy(() => import('@/pages/admin/bans_page'));`
Add route: `<Route path="bans" element={<BansPage />} />`

- [ ] **Step 3: Add menu item in admin_layout.tsx**

Add icon import: `import { StopOutlined } from '@ant-design/icons';`
Add to items array:
```typescript
    getItem('Bans', '6', <StopOutlined />),
```
Add to navigations:
```typescript
    6: "/admin/bans",
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/pages/admin/bans_page.tsx frontend/src/App.tsx frontend/src/layouts/admin_layout.tsx
git commit -m "feat: add bans page with route and menu item"
```

---

### Task 11: Frontend — Dashboard updates

**Files:**
- Modify: `frontend/src/pages/admin/dashboard_page.tsx`

- [ ] **Step 1: Add active bans count to dashboard**

Add to state:
```typescript
    total_active_bans: number,
```

Initialize in constructor:
```typescript
    total_active_bans: 0,
```

Add to componentDidMount:
```typescript
    const total_active_bans = await loadData("bans/info", new Map());
```

Add to setState:
```typescript
    total_active_bans: total_active_bans.status === 200 ? total_active_bans.data as number : 0,
```

Add to render (after filtered requests):
```typescript
    <Typography.Title
        level={4}
        style={{ margin: 5, cursor: "pointer" }}
        onClick={() => this.props.navigate("/admin/bans")}
    >
        {`${this.props.t("Active bans")}: ${this.state.total_active_bans}`}
    </Typography.Title>
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/pages/admin/dashboard_page.tsx
git commit -m "feat: add active bans count to dashboard"
```

---

### Task 12: Backend — Background task for ban cleanup

**Files:**
- Modify: `backend/src/main.rs`

- [ ] **Step 1: Add background cleanup task**

Add after AppState construction and before server start:

```rust
    // Background task: cleanup expired bans every 60 seconds
    let cleanup_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Ok(mut ban_manager) = cleanup_state.ban_manager.lock() {
                let before = ban_manager.active_count();
                ban_manager.cleanup_expired();
                let after = ban_manager.active_count();
                if before != after {
                    debug!("Ban cleanup: {} → {} active bans", before, after);
                }
            }
            if let Ok(mut rate_limiters) = cleanup_state.rate_limiter.lock() {
                rate_limiters.retain(|_, rl| {
                    rl.cleanup_expired();
                    !rl.is_empty()
                });
            }
        }
    });
```

- [ ] **Step 2: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compilation succeeds

- [ ] **Step 3: Commit**

```bash
git add backend/src/main.rs
git commit -m "feat: add background task for ban and rate limiter cleanup"
```

---

### Task 13: Integration tests

**Files:**
- Create: `backend/tests/rate_limiting_test.rs`

- [ ] **Step 1: Create integration test**

```rust
//! Integration tests for rate limiting and ban functionality.

use std::net::IpAddr;
use backend::models::{BanManager, RateLimiter, CircularTimestamps};

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
```

- [ ] **Step 2: Run tests**

Run: `cd backend && cargo test --test rate_limiting_test -- --nocapture`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add backend/tests/rate_limiting_test.rs
git commit -m "test: add integration tests for rate limiting and bans"
```
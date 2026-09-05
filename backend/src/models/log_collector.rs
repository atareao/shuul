//! # Log Collector — In-memory ring buffer for frontend log viewer
//!
//! Provides a global ring buffer (`LOG_COLLECTOR`) that captures WAF/Jail events
//! emitted by the `audit_log!` macro. Capacity is configurable at runtime.
//!
//! The macro both logs to `tracing::info!` (stdout, existing behavior) and pushes
//! a `LogEntry` to the ring buffer for the frontend.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};

/// A single log entry captured from the WAF/Jail pipelines.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub ts: String,
    pub event: String,
    pub pipeline: String,
    pub ip: Option<String>,
    pub country: Option<String>,
    pub rule_id: Option<i32>,
    pub rule_name: Option<String>,
    pub path: Option<String>,
    pub method: Option<String>,
    pub query: Option<String>,
    pub ua: Option<String>,
    pub fqdn: Option<String>,
    pub referer: Option<String>,
    pub status_code: Option<i32>,
    pub profile: Option<String>,
    pub reason: Option<String>,
}

/// Ring buffer collector with configurable capacity.
pub struct LogCollector {
    buffer: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogCollector {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    /// Returns all entries (front-to-back, oldest first).
    pub fn all(&self) -> Vec<LogEntry> {
        self.buffer.iter().cloned().collect()
    }

    pub fn set_capacity(&mut self, new_cap: usize) {
        self.capacity = new_cap;
        while self.buffer.len() > self.capacity {
            self.buffer.pop_front();
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

// ── Global singleton ──

pub static LOG_COLLECTOR: LazyLock<Mutex<LogCollector>> =
    LazyLock::new(|| Mutex::new(LogCollector::new(1000)));

/// Push a log entry into the global ring buffer (fire-and-forget).
pub fn push_log(entry: LogEntry) {
    if let Ok(mut collector) = LOG_COLLECTOR.lock() {
        collector.push(entry);
    }
}

/// Log an audit event to both stdout (tracing) and the frontend ring buffer.
///
/// # Usage
///
/// ```ignore
/// audit_log!("block",
///     "pipeline": "waf",
///     "rule_id": 42,
///     "rule_name": "Block bad IPs",
///     "ip": "1.2.3.4",
/// );
/// ```
///
/// The macro:
/// 1. Builds a JSON object from the event category and key-value arguments
/// 2. Emits a `tracing::info!` line with `[CATEGORY] ...` (preserving existing stdout logging)
/// 3. Constructs a `LogEntry` from the JSON fields and pushes it into `LOG_COLLECTOR`
#[macro_export]
macro_rules! audit_log {
    ($category:expr, $($arg:tt)*) => {{
        let json_value = serde_json::json!({
            "event": $category,
            "ts": chrono::Utc::now().to_rfc3339(),
            $($arg)*
        });
        tracing::info!("[{}] {}", $category.to_uppercase(), json_value);

        let _ts = json_value["ts"].as_str().unwrap_or("").to_string();
        let entry = $crate::models::log_collector::LogEntry {
            ts: _ts,
            event: $category.to_lowercase(),
            pipeline: json_value
                .get("pipeline")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ip: json_value.get("ip").and_then(|v| v.as_str()).map(|s| s.to_string()),
            country: json_value
                .get("country")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            rule_id: json_value
                .get("rule_id")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
            rule_name: json_value
                .get("rule_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            path: json_value
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            method: json_value
                .get("method")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            query: json_value
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ua: json_value.get("ua").and_then(|v| v.as_str()).map(|s| s.to_string()),
            fqdn: json_value
                .get("fqdn")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            referer: json_value
                .get("referer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status_code: json_value
                .get("status_code")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
            profile: json_value
                .get("profile")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            reason: json_value
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        };
        $crate::models::log_collector::push_log(entry);
    }};
}

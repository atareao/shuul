//! # StatsCollector — Contadores de estadísticas en memoria
//!
//! Almacena estadísticas agregadas de requests bloqueadas/permitidas
//! para alimentar el dashboard sin depender de la tabla `requests`.
//!
//! ## Persistencia
//!
//! Cada 30 minutos se persiste un snapshot JSON a la tabla `stats_cache`.
//! Al arrancar, se carga el último snapshot disponible.
//!
//! ## Buckets temporales
//!
//! - `minute_series`: 60 buckets (última hora)
//! - `hour_series`: 24 buckets (último día)
//! - `day_series`: 31 buckets (último mes)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use tracing::{debug, error, info, warn};

/// Un bucket temporal con conteos agregados.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub timestamp: i64,
    pub blocked: u64,
    pub allowed: u64,
}

impl Bucket {
    pub fn new(ts: i64) -> Self {
        Self {
            timestamp: ts,
            blocked: 0,
            allowed: 0,
        }
    }
}

/// Un bucket temporal con conteo de requests para un método HTTP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodBucket {
    pub timestamp: i64,
    pub count: u64,
}

/// Snapshot completo para persistir/recuperar.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatsSnapshot {
    total_allowed: u64,
    total_blocked: u64,
    top_rules: HashMap<i32, u64>,
    top_countries: HashMap<String, u64>,
    top_methods: HashMap<String, u64>,
    top_paths: HashMap<String, u64>,
    top_fqdns: HashMap<String, u64>,
    day_series: Vec<Bucket>,
    #[serde(default)]
    hour_series: Vec<Bucket>,
    #[serde(default)]
    minute_series: Vec<Bucket>,
    #[serde(default)]
    method_series: HashMap<String, Vec<MethodBucket>>,
}

/// Colector de estadísticas en memoria.
pub struct StatsCollector {
    total_allowed: AtomicU64,
    total_blocked: AtomicU64,
    top_rules: Mutex<HashMap<i32, u64>>,
    top_countries: Mutex<HashMap<String, u64>>,
    top_methods: Mutex<HashMap<String, u64>>,
    top_paths: Mutex<HashMap<String, u64>>,
    top_fqdns: Mutex<HashMap<String, u64>>,
    minute_series: Mutex<Vec<Bucket>>,
    hour_series: Mutex<Vec<Bucket>>,
    day_series: Mutex<Vec<Bucket>>,
    method_series: Mutex<HashMap<String, Vec<MethodBucket>>>,
    last_persist: AtomicI64,
}

impl StatsCollector {
    /// Crea un nuevo `StatsCollector` con valores iniciales vacíos.
    pub fn new() -> Self {
        Self {
            total_allowed: AtomicU64::new(0),
            total_blocked: AtomicU64::new(0),
            top_rules: Mutex::new(HashMap::new()),
            top_countries: Mutex::new(HashMap::new()),
            top_methods: Mutex::new(HashMap::new()),
            top_paths: Mutex::new(HashMap::new()),
            top_fqdns: Mutex::new(HashMap::new()),
            minute_series: Mutex::new(Vec::with_capacity(60)),
            hour_series: Mutex::new(Vec::with_capacity(24)),
            day_series: Mutex::new(Vec::with_capacity(31)),
            method_series: Mutex::new(HashMap::new()),
            last_persist: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    /// Carga un snapshot desde la BD (si existe).
    pub async fn load(pool: &SqlitePool) -> Self {
        let stats = Self::new();
        match sqlx::query("SELECT snapshot FROM stats_cache WHERE id = 1")
            .fetch_optional(pool)
            .await
        {
            Ok(Some(row)) => {
                let snapshot_str: String = row.get("snapshot");
                match serde_json::from_str::<StatsSnapshot>(&snapshot_str) {
                    Ok(snapshot) => {
                        stats
                            .total_allowed
                            .store(snapshot.total_allowed, Ordering::Relaxed);
                        stats
                            .total_blocked
                            .store(snapshot.total_blocked, Ordering::Relaxed);
                        if let Ok(mut map) = stats.top_rules.lock() {
                            *map = snapshot.top_rules;
                        }
                        if let Ok(mut map) = stats.top_countries.lock() {
                            *map = snapshot.top_countries;
                        }
                        if let Ok(mut map) = stats.top_methods.lock() {
                            *map = snapshot.top_methods;
                        }
                        if let Ok(mut map) = stats.top_paths.lock() {
                            *map = snapshot.top_paths;
                        }
                        if let Ok(mut map) = stats.top_fqdns.lock() {
                            *map = snapshot.top_fqdns;
                        }
                        if let Ok(mut series) = stats.day_series.lock() {
                            *series = snapshot.day_series;
                        }
                        if let Ok(mut series) = stats.hour_series.lock() {
                            *series = snapshot.hour_series;
                        }
                        if let Ok(mut series) = stats.minute_series.lock() {
                            *series = snapshot.minute_series;
                        }
                        if let Ok(mut map) = stats.method_series.lock() {
                            *map = snapshot.method_series;
                        }
                        info!(
                            "StatsCollector: loaded snapshot (blocked={}, allowed={})",
                            snapshot.total_blocked, snapshot.total_allowed
                        );
                    },
                    Err(e) => {
                        warn!("StatsCollector: failed to parse snapshot JSON: {e}, starting fresh");
                    },
                }
            },
            Ok(None) => {
                debug!("StatsCollector: no snapshot found, starting fresh");
            },
            Err(e) => {
                warn!("StatsCollector: failed to load snapshot: {e}, starting fresh");
            },
        }
        stats
    }

    /// Persiste el snapshot actual a la BD.
    pub async fn persist(&self, pool: &SqlitePool) {
        let now = Utc::now().timestamp();
        self.last_persist.store(now, Ordering::Relaxed);

        let snapshot = StatsSnapshot {
            total_allowed: self.total_allowed.load(Ordering::Relaxed),
            total_blocked: self.total_blocked.load(Ordering::Relaxed),
            top_rules: self
                .top_rules
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            top_countries: self
                .top_countries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            top_methods: self
                .top_methods
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            top_paths: self
                .top_paths
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            top_fqdns: self
                .top_fqdns
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            day_series: self
                .day_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            hour_series: self
                .hour_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            minute_series: self
                .minute_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            method_series: self
                .method_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        };

        let json_str = match serde_json::to_string(&snapshot) {
            Ok(s) => s,
            Err(e) => {
                error!("StatsCollector: failed to serialize snapshot: {e}");
                return;
            },
        };

        let result = sqlx::query(
            "INSERT INTO stats_cache (id, snapshot, updated_at) VALUES (1, ?, ?)
             ON CONFLICT (id) DO UPDATE SET snapshot = excluded.snapshot, updated_at = excluded.updated_at",
        )
        .bind(&json_str)
        .bind(Utc::now())
        .execute(pool)
        .await;

        match result {
            Ok(_) => debug!("StatsCollector: snapshot persisted"),
            Err(e) => warn!("StatsCollector: failed to persist snapshot: {e}"),
        }
    }

    /// Registra un bloqueo (WAF deny o Jail match).
    pub fn record_blocked(
        &self,
        rule_id: Option<i32>,
        country_code: Option<&str>,
        method: Option<&str>,
        path: Option<&str>,
        fqdn: Option<&str>,
    ) {
        let now = Utc::now().timestamp();
        self.total_blocked.fetch_add(1, Ordering::Relaxed);

        if let Some(rid) = rule_id {
            if let Ok(mut map) = self.top_rules.lock() {
                *map.entry(rid).or_insert(0) += 1;
            }
        }

        if let Some(cc) = country_code {
            if !cc.is_empty() {
                if let Ok(mut map) = self.top_countries.lock() {
                    *map.entry(cc.to_string()).or_insert(0) += 1;
                }
            }
        }

        if let Some(m) = method {
            if !m.is_empty() {
                if let Ok(mut map) = self.top_methods.lock() {
                    *map.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
        if let Some(p) = path {
            if !p.is_empty() {
                if let Ok(mut map) = self.top_paths.lock() {
                    *map.entry(p.to_string()).or_insert(0) += 1;
                }
            }
        }

        if let Some(f) = fqdn {
            if !f.is_empty() {
                if let Ok(mut map) = self.top_fqdns.lock() {
                    *map.entry(f.to_string()).or_insert(0) += 1;
                }
            }
        }

        self.add_to_bucket(false, now);
        self.add_method_to_bucket(method, now);
    }

    /// Registra una request permitida (no match o allow=true).
    pub fn record_allowed(&self, method: Option<&str>, path: Option<&str>, fqdn: Option<&str>) {
        let now = Utc::now().timestamp();
        self.total_allowed.fetch_add(1, Ordering::Relaxed);

        if let Some(m) = method {
            if !m.is_empty() {
                if let Ok(mut map) = self.top_methods.lock() {
                    *map.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
        if let Some(p) = path {
            if !p.is_empty() {
                if let Ok(mut map) = self.top_paths.lock() {
                    *map.entry(p.to_string()).or_insert(0) += 1;
                }
            }
        }

        if let Some(f) = fqdn {
            if !f.is_empty() {
                if let Ok(mut map) = self.top_fqdns.lock() {
                    *map.entry(f.to_string()).or_insert(0) += 1;
                }
            }
        }

        self.add_to_bucket(true, now);
        self.add_method_to_bucket(method, now);
    }

    /// Añade un evento al bucket temporal correspondiente.
    fn add_to_bucket(&self, allowed: bool, now: i64) {
        let minute_ts = now - (now % 60);

        // Minute series
        if let Ok(mut series) = self.minute_series.lock() {
            Self::bump_bucket(&mut series, minute_ts, 60, allowed);
        }

        // Hour series (lazy rollup from minutes)
        let hour_ts = now - (now % 3600);
        if let Ok(mut series) = self.hour_series.lock() {
            Self::bump_bucket(&mut series, hour_ts, 24, allowed);
        }

        // Day series
        let day_ts = now - (now % 86400);
        if let Ok(mut series) = self.day_series.lock() {
            Self::bump_bucket(&mut series, day_ts, 31, allowed);
        }
    }

    /// Incrementa un bucket existente o crea uno nuevo, manteniendo el tamaño máximo.
    fn bump_bucket(series: &mut Vec<Bucket>, ts: i64, max_size: usize, allowed: bool) {
        if let Some(bucket) = series.iter_mut().rev().find(|b| b.timestamp == ts) {
            if allowed {
                bucket.allowed += 1;
            } else {
                bucket.blocked += 1;
            }
        } else {
            let mut bucket = Bucket::new(ts);
            if allowed {
                bucket.allowed = 1;
            } else {
                bucket.blocked = 1;
            }
            series.push(bucket);
            // Keep only the last `max_size` buckets
            if series.len() > max_size {
                series.remove(0);
            }
        }
    }

    /// Añade un evento al bucket temporal correspondiente del método HTTP.
    fn add_method_to_bucket(&self, method: Option<&str>, now: i64) {
        let Some(m) = method else { return };
        if m.is_empty() {
            return;
        }
        let m = m.to_string();
        let minute_ts = now - (now % 60);
        let hour_ts = now - (now % 3600);
        let day_ts = now - (now % 86400);
        if let Ok(mut map) = self.method_series.lock() {
            let series = map.entry(m.clone()).or_default();
            Self::bump_method_bucket(series, minute_ts, 60);
            let series = map.entry(m.clone()).or_default();
            Self::bump_method_bucket(series, hour_ts, 24);
            let series = map.entry(m).or_default();
            Self::bump_method_bucket(series, day_ts, 31);
        }
    }

    /// Incrementa un bucket existente o crea uno nuevo, manteniendo el tamaño máximo.
    fn bump_method_bucket(series: &mut Vec<MethodBucket>, ts: i64, max_size: usize) {
        if let Some(bucket) = series.iter_mut().rev().find(|b| b.timestamp == ts) {
            bucket.count += 1;
        } else {
            series.push(MethodBucket {
                timestamp: ts,
                count: 1,
            });
            if series.len() > max_size {
                series.remove(0);
            }
        }
    }

    // ── Getters para los endpoints del dashboard ──

    pub fn get_total_allowed(&self) -> u64 {
        self.total_allowed.load(Ordering::Relaxed)
    }

    pub fn get_total_blocked(&self) -> u64 {
        self.total_blocked.load(Ordering::Relaxed)
    }

    pub fn get_top_rules(&self) -> Vec<(i32, u64)> {
        let map = self.top_rules.lock().unwrap_or_else(|e| e.into_inner());
        let mut vec: Vec<(i32, u64)> = map.iter().map(|(k, v)| (*k, *v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(10);
        vec
    }

    pub fn get_top_countries(&self) -> Vec<(String, u64)> {
        let map = self.top_countries.lock().unwrap_or_else(|e| e.into_inner());
        let mut vec: Vec<(String, u64)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(10);
        vec
    }

    pub fn get_top_methods(&self) -> Vec<(String, u64)> {
        let map = self.top_methods.lock().unwrap_or_else(|e| e.into_inner());
        let mut vec: Vec<(String, u64)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(10);
        vec
    }

    pub fn get_top_paths(&self) -> Vec<(String, u64)> {
        let map = self.top_paths.lock().unwrap_or_else(|e| e.into_inner());
        let mut vec: Vec<(String, u64)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(10);
        vec
    }

    pub fn get_top_fqdns(&self) -> Vec<(String, u64)> {
        let map = self.top_fqdns.lock().unwrap_or_else(|e| e.into_inner());
        let mut vec: Vec<(String, u64)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(10);
        vec
    }

    /// Devuelve la serie temporal según la unidad solicitada.
    pub fn get_evolution(&self, unit: &str) -> Vec<Bucket> {
        match unit {
            "minute" => self
                .minute_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            "hour" => self
                .hour_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            "day" => self
                .day_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            _ => self
                .day_series
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        }
    }

    /// Devuelve la evolución desglosada por método HTTP para la unidad solicitada.
    pub fn get_method_evolution(&self, unit: &str) -> Vec<(String, Vec<MethodBucket>)> {
        let map = self.method_series.lock().unwrap_or_else(|e| e.into_inner());
        let max_size = match unit {
            "minute" => 60,
            "hour" => 24,
            _ => 31,
        };
        let mut result: Vec<(String, Vec<MethodBucket>)> = map
            .iter()
            .map(|(method, series)| {
                let mut s = series.clone();
                if s.len() > max_size {
                    s.drain(..s.len() - max_size);
                }
                (method.clone(), s)
            })
            .collect();
        result.sort_by(|a, b| {
            let a_total: u64 = a.1.iter().map(|b| b.count).sum();
            let b_total: u64 = b.1.iter().map(|b| b.count).sum();
            b_total.cmp(&a_total)
        });
        result
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

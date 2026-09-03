//! # Modelo de configuración global (Settings)
//!
//! Define [`Settings`] con los parámetros globales de la aplicación
//! almacenados en la tabla `settings` (key-value).
//!
//! Los valores se cargan y persisten mediante los métodos
//! [`Settings::load`] y [`Settings::save`].

use std::net::IpAddr;
use std::str::FromStr;

use regex::Regex;
use sqlx::{Row, SqlitePool};

use crate::models::error::AppError;

/// Configuración global de la aplicación.
///
/// Cada campo se corresponde con una clave en la tabla `settings`.
/// - `safe_paths`: patrones regex de rutas seguras (exentas de rate limiting)
/// - `trusted_ips`: IPs o subredes CIDR consideradas confiables
/// - `trusted_user_agents`: patrones regex de user-agent confiables
/// - `default_rule_mode`: modo por defecto para nuevas reglas
/// - `log_retention_days`: días de retención de logs de peticiones
#[derive(Debug, Clone)]
pub struct Settings {
    pub safe_paths: Vec<String>,
    pub trusted_ips: Vec<IpNet>,
    pub trusted_user_agents: Vec<String>,
    pub default_rule_mode: String,
    pub log_retention_days: i32,
    /// Precompiled regex patterns for safe paths (rebuilt on load/update).
    pub safe_paths_re: Vec<Regex>,
    /// Precompiled regex patterns for trusted user agents (rebuilt on load/update).
    pub trusted_user_agents_re: Vec<Regex>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            safe_paths: Vec::new(),
            trusted_ips: Vec::new(),
            trusted_user_agents: Vec::new(),
            default_rule_mode: "log_only".to_string(),
            log_retention_days: 30,
            safe_paths_re: Vec::new(),
            trusted_user_agents_re: Vec::new(),
        }
    }
}

/// Una IP o subred CIDR (e.g., `10.0.0.0/8`).
#[derive(Debug, Clone)]
pub struct IpNet {
    pub addr: IpAddr,
    pub prefix_len: u8,
}

impl IpNet {
    /// Comprueba si una IP está dentro de esta subred CIDR.
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (ip, self.addr) {
            (IpAddr::V4(ip), IpAddr::V4(net)) => {
                let mask = if self.prefix_len == 0 {
                    0u32
                } else {
                    u32::MAX << (32 - self.prefix_len)
                };
                (u32::from(*ip) & mask) == (u32::from(net) & mask)
            },
            (IpAddr::V6(ip), IpAddr::V6(net)) => {
                let mask = if self.prefix_len == 0 {
                    0u128
                } else {
                    u128::MAX << (128 - self.prefix_len)
                };
                (u128::from(*ip) & mask) == (u128::from(net) & mask)
            },
            _ => false,
        }
    }
}

impl FromStr for IpNet {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((addr_str, prefix_str)) = s.split_once('/') {
            let addr: IpAddr = addr_str
                .parse()
                .map_err(|_| AppError::InvalidInput(format!("Invalid IP address: {addr_str}")))?;
            let prefix_len: u8 = prefix_str.parse().map_err(|_| {
                AppError::InvalidInput(format!("Invalid prefix length: {prefix_str}"))
            })?;
            // Validate prefix length based on address type
            match addr {
                IpAddr::V4(_) if prefix_len > 32 => {
                    return Err(AppError::InvalidInput(format!(
                        "IPv4 prefix length must be ≤ 32, got {prefix_len}"
                    )));
                },
                IpAddr::V6(_) if prefix_len > 128 => {
                    return Err(AppError::InvalidInput(format!(
                        "IPv6 prefix length must be ≤ 128, got {prefix_len}"
                    )));
                },
                _ => {},
            }
            Ok(Self { addr, prefix_len })
        } else {
            // Single IP without CIDR notation — treat as /32 or /128
            let addr: IpAddr = s
                .parse()
                .map_err(|_| AppError::InvalidInput(format!("Invalid IP address: {s}")))?;
            let prefix_len = match addr {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };
            Ok(Self { addr, prefix_len })
        }
    }
}

impl std::fmt::Display for IpNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix_len)
    }
}

impl Settings {
    /// Carga la configuración desde la base de datos.
    ///
    /// Lee todas las claves de la tabla `settings` y construye un [`Settings`].
    /// Si una clave no existe, se usa el valor por defecto.
    pub async fn load(pool: &SqlitePool) -> Result<Self, AppError> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(pool)
            .await?;

        let mut map: std::collections::HashMap<String, String> = rows
            .into_iter()
            .map(|row| {
                let key: String = row.get("key");
                let value: String = row.get("value");
                (key, value)
            })
            .collect();

        // Safe paths — comma-separated regex patterns
        let safe_paths_raw = map.remove("safe_paths").unwrap_or_default();
        let safe_paths: Vec<String> = safe_paths_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Trusted IPs — comma-separated CIDR notation
        let trusted_ips_raw = map.remove("trusted_ips").unwrap_or_default();
        let trusted_ips: Vec<IpNet> = trusted_ips_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<IpNet>().ok())
            .collect();

        // Trusted user agents — comma-separated regex patterns
        let trusted_ua_raw = map.remove("trusted_user_agents").unwrap_or_default();
        let trusted_user_agents: Vec<String> = trusted_ua_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let default_rule_mode = map
            .remove("default_rule_mode")
            .unwrap_or_else(|| "log_only".to_string());

        let log_retention_days_raw = map.remove("log_retention_days").unwrap_or_default();
        let log_retention_days: i32 = log_retention_days_raw.parse().unwrap_or(30);

        let mut s = Self {
            safe_paths,
            trusted_ips,
            trusted_user_agents,
            default_rule_mode,
            log_retention_days,
            safe_paths_re: Vec::new(),
            trusted_user_agents_re: Vec::new(),
        };
        s.recompile_patterns();
        Ok(s)
    }

    /// Recompila los patrones regex de `safe_paths` y `trusted_user_agents`
    /// en sus correspondientes vectores precompilados.
    ///
    /// Los patrones inválidos se silencian (no se incluyen en el vector).
    fn recompile_patterns(&mut self) {
        self.safe_paths_re = self
            .safe_paths
            .iter()
            .filter_map(|s| Regex::new(s).ok())
            .collect();
        self.trusted_user_agents_re = self
            .trusted_user_agents
            .iter()
            .filter_map(|s| Regex::new(s).ok())
            .collect();
    }

    /// Recompila los patrones regex. Debe llamarse después de mutar
    /// `safe_paths` o `trusted_user_agents` y antes de persistir.
    pub fn recompile(&mut self) {
        self.recompile_patterns();
    }

    /// Persiste la configuración en la base de datos (UPSERT).
    ///
    /// Cada campo de [`Settings`] se guarda como una fila independiente
    /// en la tabla `settings`.
    pub async fn save(pool: &SqlitePool, settings: &Self) -> Result<(), AppError> {
        let pairs = vec![
            ("safe_paths", settings.safe_paths.join(", ")),
            (
                "trusted_ips",
                settings
                    .trusted_ips
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            (
                "trusted_user_agents",
                settings.trusted_user_agents.join(", "),
            ),
            ("default_rule_mode", settings.default_rule_mode.clone()),
            (
                "log_retention_days",
                settings.log_retention_days.to_string(),
            ),
        ];

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?) \
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(&value)
            .execute(pool)
            .await?;
        }

        Ok(())
    }
}

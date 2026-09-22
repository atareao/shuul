use std::net::IpAddr;
use std::time::Instant;

/// A ban that has been triggered by the Jail pipeline but not yet executed.
///
/// The WAF pipeline checks `pending_bans` when no WAF rule matches and
/// executes the ban (calling `ban_manager.ban()` + `persist_ban()`).
/// This allows WAF allow rules (`allow=true`) to prevent a ban.
#[derive(Debug, Clone)]
pub struct PendingBan {
    pub ip: IpAddr,
    pub rule_id: i32,
    pub reason: String,
    pub ban_duration_seconds: Option<i64>,
    #[allow(dead_code)]
    pub escalation_level: i32,
    pub created_at: Instant,
}

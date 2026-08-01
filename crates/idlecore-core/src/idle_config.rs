//! Idle gains configuration -- shared between server and client

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Result of idle gain calculation for a specific time bracket.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IdleGains {
    pub xp: u64,
    pub gold: u64,
}

impl IdleGains {
    pub fn new(xp: u64, gold: u64) -> Self {
        Self { xp, gold }
    }
}

/// Calculate XP and Gold gains based on elapsed offline time.
///
/// Time brackets:
///   < 1 hour:    10 XP, 5 Gold
///   1-6 hours:   60 XP, 30 Gold
///   6-12 hours:  100 XP, 50 Gold
///   12-24 hours: 150 XP, 75 Gold
///   > 24 hours:  capped at 150 XP, 75 Gold (max)
///
/// Minimum threshold: must be offline for at least 1 minute (60s) to earn anything.
pub fn gains_for_time(elapsed: Duration) -> IdleGains {
    let seconds = elapsed.as_secs();

    // Minimum eligibility: 1 minute offline
    if seconds <= 60 {
        return IdleGains { xp: 0, gold: 0 };
    }

    if seconds < 3600 {
        // < 1 hour
        IdleGains { xp: 10, gold: 5 }
    } else if seconds < 21600 {
        // 1 - 6 hours
        IdleGains { xp: 60, gold: 30 }
    } else if seconds < 43200 {
        // 6 - 12 hours
        IdleGains { xp: 100, gold: 50 }
    } else {
        // 12+ hours (capped at 24 hours)
        IdleGains { xp: 150, gold: 75 }
    }
}

/// Maximum allowed offline time for idle gains (24 hours).
pub const MAX_IDLE_SECONDS: u64 = 86400;

/// Minimum offline time required to earn any idle gains (1 minute).
pub const MIN_IDLE_SECONDS: u64 = 60;

/// Get idle hours from elapsed time (for UI display)
pub fn idle_hours(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() / 3600.0
}

/// Check if idle time exceeds minimum threshold (1 minute)
pub fn is_idle_eligible(elapsed: Duration) -> bool {
    elapsed.as_secs() > 60
}

/// Validate that an idle duration is within acceptable bounds for calculation.
/// Returns `Ok(Duration)` if valid, or `Err(String)` with a descriptive error.
pub fn validate_idle_duration(elapsed: Duration) -> Result<Duration, String> {
    if elapsed.is_zero() {
        return Err("elapsed time is zero -- player was just logged in".to_string());
    }
    if elapsed.as_secs() > MAX_IDLE_SECONDS {
        return Err(format!(
            "elapsed time ({}) exceeds maximum cap of {} seconds (24h)",
            elapsed.as_secs(),
            MAX_IDLE_SECONDS
        ));
    }
    if elapsed.as_secs() < MIN_IDLE_SECONDS {
        // This is still valid but will return zero gains -- documented behavior
        return Ok(elapsed);
    }
    Ok(elapsed)
}

/// Format elapsed seconds into a human-readable offline duration string.
pub fn format_offline_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "0 min".to_string();
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

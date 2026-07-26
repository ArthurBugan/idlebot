//! Idle Gains Calculation
//!
//! Offline XP and Gold accumulate based on elapsed time since last login.
//! Anti-cheat: server-only — client only uses local time to calculate.
//! SpacetimeDB server validates and applies gains every 5 minutes.
//! Single-player local version also applies gains on login.

/// Formula from PROPOSAL section 2.2:
/// < 1 hour:    10 XP, 5 Gold
/// 1-6 hours:   60 XP, 30 Gold
/// 6-12 hours:  100 XP, 50 Gold
/// 12-24 hours: 150 XP, 75 Gold
/// Max: 24 hours
pub fn gains_for_time(elapsed: std::time::Duration) -> IdleGains {
    let seconds = elapsed.as_secs();
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
        // 12+ hours (max 24)
        IdleGains { xp: 150, gold: 75 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IdleGains {
    pub xp: u64,
    pub gold: u64,
}

impl IdleGains {
    pub fn new(xp: u64, gold: u64) -> Self {
        Self { xp, gold }
    }
}

/// Check if idle time exceeds minimum threshold (1 minute)
pub fn is_idle_eligible(elapsed: std::time::Duration) -> bool {
    elapsed.as_secs() > 60
}

/// Get idle hours from elapsed time (for UI display)
pub fn idle_hours(elapsed: std::time::Duration) -> f64 {
    elapsed.as_secs_f64() / 3600.0
}

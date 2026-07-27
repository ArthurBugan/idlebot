//! Idle gains configuration — shared between server and client
//!
//! Defines IdleGains struct, gains calculation formula, and constants.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains_zero_time() {
        // Edge case: no time elapsed — should return zero gains
        let gains = gains_for_time(Duration::from_secs(0));
        assert_eq!(gains.xp, 0);
        assert_eq!(gains.gold, 0);
    }

    #[test]
    fn gains_one_minute() {
        // At exactly 60 seconds (the minimum threshold)
        let gains = gains_for_time(Duration::from_secs(60));
        assert_eq!(gains.xp, 0);
        assert_eq!(gains.gold, 0);
    }

    #[test]
    fn gains_31_seconds_below_threshold() {
        // Below minimum threshold
        let gains = gains_for_time(Duration::from_secs(31));
        assert_eq!(gains.xp, 0);
        assert_eq!(gains.gold, 0);
    }

    #[test]
    fn gains_30_seconds_below_threshold() {
        let gains = gains_for_time(Duration::from_secs(30));
        assert_eq!(gains.xp, 0);
        assert_eq!(gains.gold, 0);
    }

    #[test]
    fn gains_first_bracket_less_than_1_hour() {
        // 5 minutes — should give first bracket gains
        let elapsed = Duration::from_secs(300);
        let gains = gains_for_time(elapsed);
        assert_eq!(gains.xp, 10);
        assert_eq!(gains.gold, 5);
    }

    #[test]
    fn gains_first_bracket_at_1_hour() {
        // At exactly 3600 seconds (1 hour) — should give second bracket
        let gains = gains_for_time(Duration::from_secs(3600));
        assert_eq!(gains.xp, 60);
        assert_eq!(gains.gold, 30);
    }

    #[test]
    fn gains_first_bracket_just_above_1_hour() {
        // 1 hour + 1 second — still in second bracket
        let gains = gains_for_time(Duration::from_secs(3601));
        assert_eq!(gains.xp, 60);
        assert_eq!(gains.gold, 30);
    }

    #[test]
    fn gains_second_bracket_midpoint() {
        // 3 hours (10800s) — middle of second bracket
        let elapsed = Duration::from_secs(10800);
        let gains = gains_for_time(elapsed);
        assert_eq!(gains.xp, 60);
        assert_eq!(gains.gold, 30);
    }

    #[test]
    fn gains_second_bracket_at_6_hours() {
        // At exactly 21600 seconds (6 hours) — should give third bracket
        let gains = gains_for_time(Duration::from_secs(21600));
        assert_eq!(gains.xp, 100);
        assert_eq!(gains.gold, 50);
    }

    #[test]
    fn gains_third_bracket_6_hours_plus() {
        // 7 hours — third bracket
        let gains = gains_for_time(Duration::from_secs(25200));
        assert_eq!(gains.xp, 100);
        assert_eq!(gains.gold, 50);
    }

    #[test]
    fn gains_third_bracket_midpoint() {
        // 9 hours — middle of third bracket
        let elapsed = Duration::from_secs(32400);
        let gains = gains_for_time(elapsed);
        assert_eq!(gains.xp, 100);
        assert_eq!(gains.gold, 50);
    }

    #[test]
    fn gains_third_bracket_at_12_hours() {
        // At exactly 43200 seconds (12 hours) — should give fourth bracket
        let gains = gains_for_time(Duration::from_secs(43200));
        assert_eq!(gains.xp, 150);
        assert_eq!(gains.gold, 75);
    }

    #[test]
    fn gains_fourth_bracket_at_24_hours() {
        // At exactly 86400 seconds (24 hours) — fourth bracket
        let gains = gains_for_time(Duration::from_secs(86400));
        assert_eq!(gains.xp, 150);
        assert_eq!(gains.gold, 75);
    }

    #[test]
    fn gains_over_24_hours_capped() {
        // 48 hours — must be capped at max
        let elapsed = Duration::from_secs(172800);
        let gains = gains_for_time(elapsed);
        assert_eq!(gains.xp, 150);
        assert_eq!(gains.gold, 75);
    }

    #[test]
    fn gains_12_hours_bracket() {
        // Just above 12 hours
        let gains = gains_for_time(Duration::from_secs(43201));
        assert_eq!(gains.xp, 150);
        assert_eq!(gains.gold, 75);
    }

    #[test]
    fn is_idle_eligible_below_min() {
        let result = is_idle_eligible(Duration::from_secs(30));
        assert!(!result);
    }

    #[test]
    fn is_idle_eligible_at_min() {
        let result = is_idle_eligible(Duration::from_secs(60));
        assert!(!result); // strictly greater than
    }

    #[test]
    fn is_idle_eligible_above_min() {
        let result = is_idle_eligible(Duration::from_secs(61));
        assert!(result);
    }

    #[test]
    fn idle_hours_calculation() {
        let elapsed = Duration::from_secs(3600); // 1 hour
        let hours = idle_hours(elapsed);
        assert!((hours - 1.0).abs() < 0.001);

        let elapsed = Duration::from_secs(7200); // 2 hours
        let hours = idle_hours(elapsed);
        assert!((hours - 2.0).abs() < 0.001);
    }

    #[test]
    fn format_offline_duration_basic() {
        assert_eq!(format_offline_duration(0), "0 min");
        assert_eq!(format_offline_duration(30), "30m");
        assert_eq!(format_offline_duration(61), "1m");
    }

    #[test]
    fn format_offline_duration_hours() {
        assert_eq!(format_offline_duration(3600), "1h 0m");
        assert_eq!(format_offline_duration(3661), "1h 1m");
    }

    #[test]
    fn format_offline_duration_24h() {
        assert_eq!(format_offline_duration(86400), "24h 0m");
    }

    #[test]
    fn gains_for_time_all_brackets_overview() {
        // Comprehensive test covering all time brackets
        let cases: Vec<(u64, u64, u64, u64, u64)> = vec![
            // (seconds, expected_xp, expected_gold)
            (0, 0, 0),
            (30, 0, 0),
            (60, 0, 0), // threshold
            (301, 10, 5),
            (3599, 10, 5),
            (3600, 60, 30),
            (7200, 60, 30),
            (21599, 60, 30),
            (21600, 100, 50),
            (25200, 100, 50),
            (43199, 100, 50),
            (43200, 150, 75),
            (86399, 150, 75),
            (86400, 150, 75),
            (172800, 150, 75),
            (u64::MAX, 150, 75),
        ];

        for (seconds, exp_xp, exp_gold) in cases {
            let elapsed = Duration::from_secs(seconds);
            let gains = gains_for_time(elapsed);
            assert_eq!(gains.xp, exp_xp, "Failed for {seconds}s: expected XP {exp_xp}, got {gains.xp}");
            assert_eq!(gains.gold, exp_gold, "Failed for {seconds}s: expected Gold {exp_gold}, got {gains.gold}");
        }
    }
}

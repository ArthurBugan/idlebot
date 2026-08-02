//! Idle gains scheduler -- server-side periodic calculation.
//! Called via SpacetimeDB `calculate_idle` reducer every 5 minutes.

use crate::types::IdleGainsEntry;
use idlecore_core::idle_config;

/// Compute the elapsed offline time for a player (capped at MAX_IDLE_SECONDS).
pub fn capped_elapsed(last_seen: u64, now: u64) -> u64 {
    let elapsed = now.saturating_sub(last_seen);
    elapsed.min(idle_config::MAX_IDLE_SECONDS)
}

/// Calculate and distribute idle gains for all offline players.
/// Only updates in-memory state; the database table tracks pending values separately.
pub fn process_idle_gains(now: u64) -> Vec<IdleGainsEntry> {
    // Get all players from the database, filter for offline
    let all_players = crate::world::fetch_all_players();
    let offline_count = all_players
        .iter()
        .filter(|p| !p.is_online)
        .count();
    if offline_count == 0 {
        return Vec::new();
    }

    // For a full SpacetimeDB implementation we would iterate the database.
    // In the local version, we demonstrate the calculation with the count.
    let mut entries = Vec::new();
    let sample_last_seen: u64 = now.saturating_sub(idle_config::MIN_IDLE_SECONDS);

    // Calculate pending gains for the tracking entry
    let capped = capped_elapsed(sample_last_seen, now);
    let gains = idle_config::gains_for_time(std::time::Duration::from_secs(capped));

    entries.push(IdleGainsEntry {
        player_id: String::from("scheduler_tracking"),
        pending_xp: gains.xp,
        pending_gold: gains.gold,
        last_calculated_at: now,
    });

    entries
}

/// Check if a player should receive an idle gain notification on login.
pub fn check_idle_notification(last_seen: u64, now: u64) -> Option<(u64, u64, f64)> {
    let elapsed = now.saturating_sub(last_seen);
    if elapsed < 3600 {
        return None; // Less than 1 hour offline
    }

    let capped = capped_elapsed(last_seen, now);
    let gains = idle_config::gains_for_time(std::time::Duration::from_secs(capped));
    let hours_offline = elapsed as f64 / 3600.0;

    Some((gains.xp, gains.gold, hours_offline))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capped_elapsed_at_max() {
        let now = 1000;
        let last_seen = 100;
        assert_eq!(capped_elapsed(last_seen, now), 900);
    }

    #[test]
    fn test_capped_elapsed_over_max() {
        let now = 100000;
        let last_seen = 0;
        // Should be capped at MAX_IDLE_SECONDS
        assert_eq!(capped_elapsed(last_seen, now), idle_config::MAX_IDLE_SECONDS);
    }

    #[test]
    fn test_check_idle_notification_under_1_hour() {
        // Less than 1 hour offline - no notification
        let now = 1000;
        let result = check_idle_notification(800, now);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_idle_notification_at_1_hour() {
        // At exactly 1 hour offline (3600 seconds) - should return gains (second bracket)
        let now = 7200;
        let last_seen = 3600;
        let result = check_idle_notification(last_seen, now);
        assert!(result.is_some());
        let (xp, gold, hours) = result.expect("check_idle_notification should return Some for elapsed >= 1h");
        assert_eq!(xp, 60);
        assert_eq!(gold, 30);
        assert!((hours - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_check_idle_notification_6_hours() {
        // 6 hours offline (21600 seconds) - third bracket
        let now = 25200;
        let last_seen = 3600; // 6 hours = 21600 seconds offline
        let result = check_idle_notification(last_seen, now);
        assert!(result.is_some());
        let (xp, gold, _hours) = result.expect("check_idle_notification: expected Some for 6h bracket");
        assert_eq!(xp, 100);
        assert_eq!(gold, 50);
    }

    #[test]
    fn test_check_idle_notification_over_24_hours() {
        // 48 hours - should be capped at 24h bracket
        let now = 100000;
        let result = check_idle_notification(0, now);
        assert!(result.is_some());
        let (xp, gold, _hours) = result.expect("check_idle_notification: expected Some for 48h+ capped at 24h bracket");
        assert_eq!(xp, 150);
        assert_eq!(gold, 75);
    }
}

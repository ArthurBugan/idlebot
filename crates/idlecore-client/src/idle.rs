//! Idle gains UI and state management for the client.
//!
//! Uses shared core::idle_config for gain calculations.
//! Provides types and functions for idle gains tracking.

use bevy::prelude::*;
use idlecore_core::idle_config::{self};
use std::time::Duration;

/// Marker component for the idle gains panel entity.
#[derive(Component)]
pub struct IdleGainsPanel;

/// Button state machine for the Claim All button.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimButtonState {
    Ready,
    Pressing,
    JustClicked,
    Claiming,
    JustClaimed,
    Claimed,
}

impl Default for ClaimButtonState {
    fn default() -> Self {
        Self::Ready
    }
}

/// The idle gains claimed state component.
#[derive(Component)]
pub struct IdleGainsClaimed {
    pub claimed_xp: u64,
    pub claimed_gold: u64,
}

impl Default for IdleGainsClaimed {
    fn default() -> Self {
        Self {
            claimed_xp: 0,
            claimed_gold: 0,
        }
    }
}

/// Result of idle gain calculation.
#[derive(Debug, Clone)]
pub struct IdleGainResult {
    pub xp: u64,
    pub gold: u64,
    pub hours_offline: f64,
}

/// Calculate idle gains for a given offline duration.
pub fn calculate_offline_gains(elapsed_seconds: u64) -> IdleGainResult {
    let gains = idle_config::gains_for_time(Duration::from_secs(elapsed_seconds));
    IdleGainResult {
        xp: gains.xp,
        gold: gains.gold,
        hours_offline: elapsed_seconds as f64 / 3600.0,
    }
}

/// Check if a player is eligible for idle gains (offline > 1 minute).
pub fn is_offline_eligible(elapsed_seconds: u64) -> bool {
    elapsed_seconds > 60
}

/// Get human-readable offline duration string.
pub fn format_offline_duration(seconds: u64) -> String {
    idle_config::format_offline_duration(seconds)
}

/// Apply idle gains to player state.
pub fn apply_idle_gains_to_player(
    player: &mut crate::player::ClientPlayer,
    now_seconds: u64,
) {
    let last_seen = player.last_login_time;
    let elapsed = if last_seen == 0 {
        0
    } else {
        now_seconds.saturating_sub(last_seen)
    };

    let result = calculate_offline_gains(elapsed);

    if elapsed > 60 {
        player.xp += result.xp;
        player.gold += result.gold;
        player.level = crate::progression::calculate_level(player.xp);
        player.last_login_time = now_seconds;
        player.time_offline = Some(elapsed);
    }
}

/// ClientPlayer extension helpers.
impl crate::player::ClientPlayer {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_offline_gains() {
        let result = calculate_offline_gains(300); // 5 minutes
        assert_eq!(result.xp, 10);
        assert_eq!(result.gold, 5);
    }

    #[test]
    fn test_calculate_offline_gains_1_hour() {
        let result = calculate_offline_gains(3600);
        assert_eq!(result.xp, 60);
        assert_eq!(result.gold, 30);
    }

    #[test]
    fn test_calculate_offline_gains_zero() {
        let result = calculate_offline_gains(30);
        assert_eq!(result.xp, 0);
        assert_eq!(result.gold, 0);
    }

    #[test]
    fn test_is_offline_eligible() {
        assert!(!is_offline_eligible(60));
        assert!(is_offline_eligible(61));
    }

    #[test]
    fn test_format_offline_duration() {
        assert_eq!(format_offline_duration(0), "0 min");
        assert_eq!(format_offline_duration(61), "1m 1s");
        assert_eq!(format_offline_duration(3600), "1h 0m");
    }
}

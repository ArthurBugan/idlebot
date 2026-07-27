//! Idle gains UI and state management for the client.
//!
//! Uses shared core::idle_config for gain calculations.
//! Provides a Bevy UI panel showing pending idle XP/Gold and a Claim All button.
//! Uses a simple state machine for the button (Ready -> Claiming -> Claimed -> Ready).

use bevy::prelude::*;
use idlecore_core::idle_config::{self, IdleGains};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Marker component for the idle gains panel entity.
#[derive(Component)]
pub struct IdleGainsPanel;

/// Button state machine for the Claim All button.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimButtonState {
    /// Button is ready to be clicked
    Ready,
    /// Button is being pressed/animated
    Pressing,
    /// Button just clicked, waiting for effect
    JustClicked,
    /// Button is processing the claim
    Claiming,
    /// Button finished claiming, showing result
    JustClaimed,
    /// Button is locked after claim (cooldown period)
    Claimed,
}

impl Default for ClaimButtonState {
    fn default() -> Self {
        Self::Ready
    }
}

/// Display state for the idle coefficient indicator.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleDisplayState {
    NotOffline,
    Offline1x,   // 1-6 hours bracket
    Offline2x,   // 6-12 hours bracket
    Offline3x,   // 12-24 hours bracket
    Offline4x,   // 24h+
    Offline5x,   // 24h+
}

impl Default for IdleDisplayState {
    fn default() -> Self {
        Self::NotOffline
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

/// Result of idle gain calculation — wrapper around core's IdleGains with metadata.
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

/// Create the idle gains panel entity with all child entities.
pub fn create_idle_gains_panel(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut texts: ResMut<Assets<Text>>,
) -> Entity {
    let mut parent_entity = None;

    // Background panel (dark semi-transparent)
    let panel_mesh = meshes.add(Cuboid::new(20.0, 1.5, 1.5));
    let panel_material = StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.1),
        cull_mode: Some(CullMode::Back),
        ..Default::default()
    };

    let panel_entity = commands
        .spawn((
            Name::new("idle_gains_panel"),
            Mesh3d(panel_mesh),
            MeshMaterial3d(panel_material),
            Transform::default(),
            IdleGainsPanel,
        ))
        .id();
    parent_entity = Some(panel_entity);

    // Header text
    let header_text = texts.add(Text {
        value: "Idle Gains".to_string(),
        style: TextStyle {
            color: Color::srgb(0.8, 0.9, 1.0),
            font_size: 16.0,
        },
        ..Default::default()
    });
    let _ = commands
        .spawn((
            Name::new("header_text"),
            Text3d(header_text),
            Transform::from_xyz(0.0, 0.8, 0.5),
            Visibility::default(),
        ))
        .id();

    // Pending XP text
    let pending_xp_text = texts.add(Text {
        value: "XP: 0".to_string(),
        style: TextStyle {
            color: Color::srgb(1.0, 1.0, 0.8),
            font_size: 14.0,
        },
        ..Default::default()
    });
    let _ = commands
        .spawn((
            Name::new("pending_xp_text"),
            Text3d(pending_xp_text),
            Transform::from_xyz(0.0, 0.0, 0.5),
            Visibility::default(),
        ))
        .id();

    // Pending Gold text
    let pending_gold_text = texts.add(Text {
        value: "Gold: 0".to_string(),
        style: TextStyle {
            color: Color::srgb(0.8, 1.0, 0.6),
            font_size: 14.0,
        },
        ..Default::default()
    });
    let _ = commands
        .spawn((
            Name::new("pending_gold_text"),
            Text3d(pending_gold_text),
            Transform::from_xyz(0.0, -0.3, 0.5),
            Visibility::default(),
        ))
        .id();

    // Offline duration text
    let offline_text = texts.add(Text {
        value: "0h 0m".to_string(),
        style: TextStyle {
            color: Color::srgb(0.6, 0.6, 0.8),
            font_size: 12.0,
        },
        ..Default::default()
    });
    let _ = commands
        .spawn((
            Name::new("offline_duration_text"),
            Text3d(offline_text),
            Transform::from_xyz(0.0, -0.8, 0.5),
            Visibility::default(),
        ))
        .id();

    // Claim All button
    let button_mesh = meshes.add(Cuboid::new(2.0, 0.3, 2.0));
    let button_material = StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 0.9),
        cull_mode: Some(CullMode::Front),
        ..Default::default()
    };

    commands
        .spawn((
            Name::new("claim_all_button"),
            Mesh3d(button_mesh),
            MeshMaterial3d(button_material),
            Transform::from_xyz(0.0, -1.5, 0.5),
            ClaimAllButton,
        ))
        .id();

    panel_entity
}

/// Calculate and display idle gains when player logs in.
/// Updates the panel's pending values and time display.
pub fn apply_idle_gains_to_panel(player: &mut ClientPlayer, now_seconds: u64) {
    let last_seen = player.last_login_time;
    let elapsed = if last_seen == 0 {
        0
    } else {
        now_seconds.saturating_sub(last_seen)
    };

    let result = calculate_offline_gains(elapsed);

    // Store the claimed gains (they get applied to player state)
    let pending_xp = result.xp;
    let pending_gold = result.gold;

    // Store time offset for display
    if elapsed > 60 {
        player.time_offline_seconds = Some(elapsed);
    }
}

/// Update the idle panel UI with current values.
pub fn update_idle_panel(
    time: Res<Time>,
    player: &ClientPlayer,
    text_entities: &mut Query<&mut Text, (With<Name>, With<Parent>)>,
) {
    let last_login = if player.last_login_time == 0 {
        SystemTime::now().duration_since(UNIX_EPOCH).expect("failed to get system time for idle gains display").as_secs()
    } else {
        player.last_login_time
    };

    let now = time.elapsed_secs().unwrap_or(0) + last_login;
    let seconds_offline = now.saturating_sub(last_login);

    // Check eligibility
    let has_gains = seconds_offline > 60;

    if has_gains {
        let result = calculate_offline_gains(seconds_offline);

        // Update pending XP text
        for mut text in text_entities.iter_mut() {
            if text.style.color == Color::srgb(1.0, 1.0, 0.8) {
                text.value = format!("XP: {}", result.xp);
            }
        }

        // Update pending Gold text
        for mut text in text_entities.iter_mut() {
            if text.style.color == Color::srgb(0.8, 1.0, 0.6) {
                text.value = format!("Gold: {}", result.gold);
            }
        }

        // Update offline duration text
        for mut text in text_entities.iter_mut() {
            if text.style.color == Color::srgb(0.6, 0.6, 0.8) {
                text.value = format_offline_duration(seconds_offline);
            }
        }

        // Mark button as enabled
        for mut state in text_entities.iter_mut() {
            if text_entities
                .iter()
                .any(|t| {
                    !t.style.font_size.is_none()
                        && t.style.color == Color::srgb(0.3, 0.6, 0.9)
                        && t.value == "Claim All"
                        && text_entities.iter().any(|other| {
                            other.style.font_size.is_none()
                                && other.style.color == Color::srgb(0.3, 0.6, 0.9)
                                && other.value == "Claim All"
                                && other != &text_entities.iter().find(|t| {
                                    !t.style.font_size.is_none()
                                    .  })
                                                                    .expect("expected to find offline duration text for panel")
                                                            })
                                                    })
                                                {
                                                    // Enable claim button logic
                                                }
        }
    }
}

/// Handle Claim All button press — apply gains to player and disable button.
pub fn handle_claim_all_button(
    time: Res<Time>,
    mut buttons: Query<(Entity, &mut ClaimButtonState)>,
    mut player_query: Query<&mut ClientPlayer>,
    mut claimed_query: Option<(Entity, &mut ClientPlayer, Option<&IdleGainsClaimed>)>,
) {
    // Find the button
    if let Ok((button_entity, mut btn_state)) = buttons.single() {
        match btn_state {
            ClaimButtonState::Ready | ClaimButtonState::Claiming => {
                btn_state = ClaimButtonState::Claiming;

                // Get the player
                if let Ok(mut player) = player_query.single() {
                    let now_seconds = time.elapsed_secs().unwrap_or(0);

                    // Calculate gains
                    let last_login = if player.last_login_time == 0 {
                        now_seconds
                    } else {
                        player.last_login_time
                    };
                    let elapsed = if last_login == 0 {
                        now_seconds
                    } else {
                        now_seconds.saturating_sub(last_login)
                    };

                    let result = calculate_offline_gains(elapsed);
                    let (xp_gained, gold_gained) = (result.xp, result.gold);

                    // Apply gains to player
                    player.xp += xp_gained;
                    player.gold += gold_gained;
                    player.level = crate::progression::calculate_level(player.xp);
                    player.set_last_seen(now_seconds);

                    // Disable button (prevent double-claiming)
                    btn_state = ClaimButtonState::Claimed;
                    println!("Claimed {} XP, {} Gold", xp_gained, gold_gained);
                }
            }
            _ => {
                btn_state = ClaimButtonState::Ready;
            }
        }
    }
}

/// ClientPlayer extension trait or helper to get/set last_seen.
/// Since ClientPlayer is defined in player.rs, we add a helper here.
impl ClientPlayer {
    /// Set the last seen timestamp
    pub fn set_last_seen(&mut self, timestamp: u64) {
        self.last_seen = timestamp;
    }

    /// Get the last seen timestamp
    pub fn last_seen(&self) -> u64 {
        self.last_seen
    }
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
        // Less than 1 minute
        let result = calculate_offline_gains(30);
        assert_eq!(result.xp, 0);
        assert_eq!(result.gold, 0);
    }

    #[test]
    fn test_is_offline_eligible() {
        assert!(!is_offline_eligible(60)); // Exactly at threshold
        assert!(is_offline_eligible(61));  // Above threshold
    }

    #[test]
    fn test_format_offline_duration() {
        assert_eq!(format_offline_duration(0), "0h 0m");
        assert_eq!(format_offline_duration(61), "1h 1m");
        assert_eq!(format_offline_duration(3600), "1h 0m");
    }
}

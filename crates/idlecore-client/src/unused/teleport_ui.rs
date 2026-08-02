//! Client-side teleport UI state and interaction handling.
//!
//! Shows teleport targets on the minimap, handles hex selection,
//! displays cooldown timer and gold cost.

use bevy::prelude::*;
use idlecore_core::teleport::{TeleportTarget, teleport_cost};
use crate::hex::HexCoord;

// ---------------------------------------------------------------------------
// Client Teleport State
// ---------------------------------------------------------------------------

/// UI state for the teleport system.
/// Each frame the Bevy system updates this based on player position and input.
#[derive(Component, Debug)]
pub struct TeleportComponent {
    /// Currently selected hex for teleport destination.
    pub selected_hex: Option<HexCoord>,
    /// Cooldown timer remaining in seconds.
    pub cooldown_timer: f32,
    /// Whether the teleport button is enabled (cooldown expired + enough gold).
    pub teleport_available: bool,
    /// Cost to teleport at current level.
    pub teleport_cost: u64,
    /// Beacon target for player transport.
    pub beacon_hex: Option<HexCoord>,
}

impl TeleportComponent {
    pub fn new() -> Self {
        Self {
            selected_hex: None,
            cooldown_timer: 0.0,
            teleport_available: true,
            teleport_cost: 100,
            beacon_hex: None,
        }
    }

    /// Check if we can teleport right now.
    pub fn can_teleport(&self) -> bool {
        self.teleport_available && self.cooldown_timer <= 0.0 && self.teleport_cost <= 100
    }

    /// Update cooldown timer based on time elapsed.
    pub fn tick_cooldown(&mut self, dt: f32) {
        if self.cooldown_timer > 0.0 {
            self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
        }
        self.teleport_available = self.cooldown_timer <= 0.0;
    }

    /// Start cooldown for 60 seconds.
    pub fn start_cooldown(&mut self) {
        self.cooldown_timer = 60.0;
        self.teleport_available = false;
        self.selected_hex = None;
    }

    /// Select a hex for teleport.
    pub fn select_hex(&mut self, hex: HexCoord) {
        self.selected_hex = Some(hex);
        self.teleport_available = false;
    }

    /// Deselect current teleport target.
    pub fn clear_selection(&mut self) {
        self.selected_hex = None;
    }

    /// Update beacon target (player transport).
    pub fn set_beacon(&mut self, hex: HexCoord) {
        self.beacon_hex = Some(hex);
    }

    /// Clear beacon target.
    pub fn clear_beacon(&mut self) {
        self.beacon_hex = None;
    }
}

impl Default for TeleportComponent {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Client Teleport UI System
// ---------------------------------------------------------------------------

/// Update the teleport UI based on player position and game state.
pub fn update_teleport_ui(
    time: Res<Time>,
    mut teleport_query: Query<&mut TeleportComponent>,
    mut player_query: Query<(&Transform, &mut TeleportComponent), Without<TeleportComponent>>,
) {
    let Ok((mut teleport, _player)) = teleport_query.single_mut() else {
        return;
    };

    // Tick cooldown
    teleport.tick_cooldown(time.delta_secs());

    // Update teleport cost based on level
    teleport.teleport_cost = teleport_cost(1);
}

/// Handle mouse clicks on hex tiles to select teleport destination.
pub fn handle_hex_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut teleport_query: Query<&mut TeleportComponent>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // TODO: Raycast to find clicked hex
    // For now, this is a placeholder
}

/// Confirm teleport when button is pressed.
pub fn confirm_teleport(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut teleport_query: Query<&mut TeleportComponent>,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }

    let mut teleport = teleport_query.single_mut();
    if teleport.can_teleport() {
        if let Some(hex) = teleport.selected_hex {
            // Execute teleport
            teleport.start_cooldown();
            // TODO: Send teleport command to server
        }
    }
}

// ---------------------------------------------------------------------------
// Beacon system for player transport
// ---------------------------------------------------------------------------

/// System to manage beacon targets for player transport.
pub fn update_beacon_system(
    time: Res<Time>,
    mut teleport_query: Query<&mut TeleportComponent>,
) {
    let Ok(mut teleport) = teleport_query.single_mut() else {
        return;
    };

    // Auto-expire beacon after 5 minutes if not used
    if let Some(hex) = teleport.beacon_hex {
        let now = time.elapsed_secs();
        // TODO: Track beacon timestamp, expire after 5 minutes
    }
}

//! Input handling — WASD movement and interaction system
//!
//! Maps keyboard input to movement commands and E key interaction.

use bevy::prelude::*;
use std::time::SystemTime;

use crate::player::{ClientPlayer, CurrentHex};
use crate::world_pos_to_hex;

/// Interaction marker component for hex entities with plants/pollution
#[derive(Component, Debug, Clone)]
pub struct InteractionTarget {
    pub hex_id: u64,
    pub has_plant: bool,
    pub has_pollution: bool,
    /// Plant type name (e.g. "Wheat", "Tree")
    pub plant_type: Option<String>,
}

/// Interaction handler — triggered by E key press
#[derive(Component)]
pub struct InteractionHandler {
    /// Currently selected action (default: Plant)
    pub action: InteractionAction,
}

/// Supported interaction actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum InteractionAction {
    Plant,
    Harvest,
    Clean,
}

impl std::fmt::Display for InteractionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractionAction::Plant => write!(f, "plant"),
            InteractionAction::Harvest => write!(f, "harvest"),
            InteractionAction::Clean => write!(f, "clean"),
        }
    }
}

/// Update input system — handles WASD movement and E key interaction
pub fn handle_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut ClientPlayer, &mut Transform)>,
) {
    let mut iter = player_query.iter_mut();
    let Some((mut player, mut transform)) = iter.next() else {
        return;
    };
    // Consume the iterator to ensure we have exclusive access
    drop(iter);

    let mut direction = Vec2::ZERO;

    // WASD movement
    if keyboard.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction.length() > 0.0 {
        direction = direction.normalize();
    }

    let speed = 10.0 * time.delta_secs();
    let movement = Vec2::new(direction.x * speed, direction.y * speed);

    transform.translation.x += movement.x as f32;
    transform.translation.z += movement.y as f32;
    player.position = transform.translation;

    // Update hex tracking
    let hex_radius = 10.0f32;
    player.current_hex = Some(CurrentHex {
        q: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).0,
        r: world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).1,
    });

    // Reset position with R key
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }

    // Interaction with E key — trigger hex interaction
    if keyboard.just_pressed(KeyCode::KeyE) {
        println!("[INPUT] Interaction triggered (E key). Action: {:?}", "plant");
    }
}

/// Get current Unix timestamp
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

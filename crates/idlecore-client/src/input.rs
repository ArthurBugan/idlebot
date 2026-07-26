//! Input handling — WASD movement system
//!
//! Maps keyboard input to movement commands for the player.

use bevy::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::player::{ClientPlayer, CurrentHex};

/// Update input system — handles WASD and special keys
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
        q: crate::world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).0,
        r: crate::world_pos_to_hex(transform.translation.x, transform.translation.z, hex_radius).1,
    });

    // Reset position with R key
    if keyboard.just_pressed(KeyCode::KeyR) {
        transform.translation = Vec3::ZERO;
        player.position = Vec3::ZERO;
        player.current_hex = Some(CurrentHex { q: 0, r: 0 });
        player.velocity = Vec2::ZERO;
    }
}

/// Get current Unix timestamp
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

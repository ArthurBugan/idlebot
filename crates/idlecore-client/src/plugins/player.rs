//! Player system plugin
//! Handles player movement and position tracking

use bevy::prelude::*;
use crate::player::PlayerTransform;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, player_movement);
    }
}

/// Player movement system — WASD input
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut crate::player::ClientPlayer)>,
    mut player_transform: ResMut<PlayerTransform>,
) {
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
    };

    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) { input.y -= 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { input.y += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { input.x -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { input.x += 1.0; }

    let speed = 100.0;
    let dt = time.delta_secs();
    let delta = input * speed * dt;
    transform.translation.x += delta.x;
    transform.translation.z += delta.y;
    player.position = transform.translation;

    // Update resource
    player_transform.translation = transform.translation;
}

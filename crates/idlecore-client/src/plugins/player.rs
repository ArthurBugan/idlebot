//! Player system plugin
//! Handles player movement and position tracking

use bevy::prelude::*;
use crate::player::{PlayerOrientation, PlayerTransform};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, player_movement);
        app.add_systems(Startup, register_player_orientation);
    }
}

fn register_player_orientation(mut commands: Commands) {
    commands.insert_resource(PlayerOrientation::default());
}

/// Player movement system — WASD input
fn player_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut crate::player::ClientPlayer)>,
    mut player_transform: ResMut<PlayerTransform>,
    mut orientation: ResMut<PlayerOrientation>,
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

    player_transform.translation = transform.translation;

    if input.length() > 0.0 {
        orientation.facing_angle = input.y.atan2(input.x);
    }
}

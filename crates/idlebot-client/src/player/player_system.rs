//! Controles do Jogador — WASD + Câmera

use crate::assets::procedural::Vehicle;
use bevy::prelude::*;

/// Componente de transform do player
#[derive(Component)]
pub struct PlayerTransform {
    pub position: Vec2,
    pub speed: f32,
    pub vehicle: Vehicle,
}

/// Camera que segue o jogador com zoom
#[derive(Component)]
pub struct GameCamera;

/// Sistema de câmera que segue o jogador
pub fn update_camera(
    player_query: Query<&Transform, With<PlayerTransform>>,
    mut camera_query: Query<&mut Transform, With<GameCamera>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut zoom: ResMut<Zoom>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let mut camera_transform = camera_query.get_single_mut().unwrap();

    let target_x = player_transform.translation.x;
    let target_y = player_transform.translation.y;

    camera_transform.translation.x = lerp(camera_transform.translation.x, target_x, 0.1);
    camera_transform.translation.y = lerp(camera_transform.translation.y, target_y, 0.1);

    if keys.pressed(KeyCode::Digit1) {
        zoom.value = (zoom.value - 0.1).max(0.5);
    }
    if keys.pressed(KeyCode::Digit2) {
        zoom.value = (zoom.value + 0.1).min(3.0);
    }
}

#[derive(Resource)]
pub struct Zoom {
    pub value: f32,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Sistema de movimento WASD
pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(&mut Transform, &mut PlayerTransform)>,
) {
    let Ok((mut transform, mut player)) = player_query.get_single_mut() else {
        return;
    };

    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction.length() > 0.0 {
        direction = direction.normalize();
    }

    let speed = 100.0 * player.vehicle.speed_multiplier();
    let delta = direction * speed * time.delta_secs();

    transform.translation.x += delta.x;
    transform.translation.y += delta.y;

    player.position = Vec2::new(transform.translation.x, transform.translation.y);
}

/// Sistema de colisão com bordas do mapa
pub fn clamp_to_map(mut player_query: Query<&mut Transform, With<PlayerTransform>>) {
    const MAP_BOUNDARY: f32 = 600.0;

    let Ok(mut transform) = player_query.get_single_mut() else {
        return;
    };

    transform.translation.x = transform.translation.x.clamp(-MAP_BOUNDARY, MAP_BOUNDARY);
    transform.translation.y = transform.translation.y.clamp(-MAP_BOUNDARY, MAP_BOUNDARY);
}

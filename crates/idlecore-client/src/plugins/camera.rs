//! Camera system plugin
//! Handles camera following the player

use bevy::prelude::*;
use crate::player::PlayerTransform;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, follow_camera);
    }
}

/// Camera follow system
fn follow_camera(
    player_transform: Res<PlayerTransform>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };

    let offset = Vec3::new(0.0, 500.0, 500.0);
    camera_transform.translation = player_transform.translation + offset;
    camera_transform.look_at(player_transform.translation, Vec3::Y);
}

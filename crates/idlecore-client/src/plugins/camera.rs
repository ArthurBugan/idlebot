//! Camera system plugin
//! Handles camera following the player + keyboard zoom

use bevy::prelude::*;
use crate::player::PlayerTransform;

/// Camera zoom level (controlled by keyboard)
#[derive(Resource)]
pub struct CameraZoom {
    pub distance: f32,
    pub height: f32,
}

impl Default for CameraZoom {
    fn default() -> Self {
        Self {
            distance: 150.0,  // Starting distance (farther)
            height: 150.0,
        }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (follow_camera, handle_zoom));
    }
}

/// Camera follow system
fn follow_camera(
    player_transform: Res<PlayerTransform>,
    zoom: Res<CameraZoom>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };

    let offset = Vec3::new(0.0, zoom.height, zoom.distance);
    camera_transform.translation = player_transform.translation + offset;
    camera_transform.look_at(player_transform.translation, Vec3::Y);
}

/// Handle keyboard zoom (+ to zoom in, - to zoom out)
fn handle_zoom(
    keys: Res<ButtonInput<KeyCode>>,
    mut zoom: ResMut<CameraZoom>,
) {
    let delta = 10.0;
    
    // Zoom in with + key
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        zoom.distance = (zoom.distance - delta).clamp(30.0, 300.0);
        zoom.height = (zoom.height - delta).clamp(30.0, 300.0);
    }
    
    // Zoom out with - key
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        zoom.distance = (zoom.distance + delta).clamp(30.0, 300.0);
        zoom.height = (zoom.height + delta).clamp(30.0, 300.0);
    }
}

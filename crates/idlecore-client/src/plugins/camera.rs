//! Camera system plugin
//! Handles camera following the player + keyboard zoom

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use crate::player::PlayerTransform;
use crate::minimap::MinimapState;

/// Minimap corner padding (must match `minimap::MINIMAP_PADDING`).
const MINIMAP_PADDING: f32 = 10.0;

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

/// Is the cursor currently over the minimap (top-right corner)?
fn cursor_over_minimap(
    windows: &Query<&Window>,
    minimap_state: &MinimapState,
) -> bool {
    let Ok(window) = windows.single() else { return false };
    let Some(cursor) = window.cursor_position() else { return false };
    let mm_size = minimap_state.mm_size();
    let mm_left = window.width() - MINIMAP_PADDING - mm_size;
    let mm_top = MINIMAP_PADDING;
    cursor.x >= mm_left
        && cursor.x < mm_left + mm_size
        && cursor.y >= mm_top
        && cursor.y < mm_top + mm_size
}

/// Handle camera zoom: mouse wheel (over the map, not the minimap) and +/- keys.
fn handle_zoom(
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: MessageReader<MouseWheel>,
    windows: Query<&Window>,
    minimap_state: Res<MinimapState>,
    mut zoom: ResMut<CameraZoom>,
) {
    let delta = 10.0;

    for event in scroll.read() {
        if event.y == 0.0 || cursor_over_minimap(&windows, &minimap_state) {
            continue;
        }
        // Wheel up zooms in (closer), down zooms out.
        zoom.distance = (zoom.distance - event.y * delta).clamp(30.0, 300.0);
        zoom.height = (zoom.height - event.y * delta).clamp(30.0, 300.0);
    }

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
